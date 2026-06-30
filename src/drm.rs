use std::{collections::{HashMap, HashSet}, path::Path};

use smithay::{
    backend::{
        allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
        drm::{DrmDevice, DrmDeviceFd, compositor::FrameFlags, exporter::gbm::{GbmFramebufferExporter, NodeFilter}},
        egl::{context::EGLContext, display::EGLDisplay},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{Color32F, ImportDma, element::surface::WaylandSurfaceRenderElement, gles::GlesRenderer},
        session::{Event as SessionEvent, Session, libseat::LibSeatSession},
        udev::{UdevBackend, UdevEvent},
    },
    desktop::space::SpaceRenderElements,
    output::{Mode as WlMode, Output, OutputModeSource, PhysicalProperties, Scale},
    reexports::{
        calloop::{EventLoop, LoopHandle},
        drm::{buffer::DrmFourcc, control::{Device, connector::State, crtc::Handle, property::Value}},
        input::Libinput,
    },
    utils::Size,
};
use rustix::fs::OFlags;
use libdisplay_info::info::Info;
use crate::{AeroWM, rendering, state::{GbmDrmCompositor, GpuData, ViewportState}, state::ViewMode};

fn open_gpu(
    device_id: u64,
    path: &Path,
    state: &mut AeroWM,
    handle: &LoopHandle<'static, AeroWM>,
) {
    eprintln!("opening GPU: {:?}", path);

    let fd = DrmDeviceFd::new(
        state.session.as_mut().unwrap()
            .open(path, OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY)
            .expect("Failed to open GPU")
            .into(),
    );

    let (mut drm, drm_notifier) = DrmDevice::new(fd.clone(), true).expect("Invalid DRM node");
    let gbm = GbmDevice::new(fd.clone()).expect("Failed to init GBM");

    handle.insert_source(drm_notifier, move |event, _, state| {
        state.process_drm_event(device_id, event);
    }).unwrap();

    let egl_display = unsafe { EGLDisplay::new(gbm.clone()).expect("Failed to create EGL display") };
    let egl_ctx = EGLContext::new(&egl_display).expect("Failed to create EGL context");
    let mut renderer = unsafe { GlesRenderer::new(egl_ctx).expect("Failed to create GLES renderer") };

    let line_prog = rendering::compile_line(&mut renderer);
    let solid_prog = rendering::compile_solid(&mut renderer);
    let border_prog = rendering::compile_border(&mut renderer);
    let clip_prog = rendering::compile_clip(&mut renderer);

    let resources = drm.resource_handles().expect("Failed to get DRM resources");
    let mut compositors: HashMap<Handle, GbmDrmCompositor> = HashMap::new();

    let mut crtc_to_output: HashMap<Handle, Output> = HashMap::new();
    let mut used_crtcs: HashSet<Handle> = HashSet::new();
    let mut x_offset = 0;

    for &connector_handle in resources.connectors() {
        let info = drm.get_connector(connector_handle, true).unwrap();
        if info.state() != State::Connected {
            continue;
        }
        let properties = drm.get_properties(connector_handle).expect("handle doesnt have properties");
        let mut make: Option<String> = None;
        let mut model: Option<String> = None;
        let mut serial_number: Option<String> = None;
        for (handle, value) in properties {
            let property = drm.get_property(handle).unwrap();
            let name = property.name().to_str().unwrap();
            if name == "EDID" {
                let value_type = property.value_type();
                let value = value_type.convert_value(value);
                if let Value::Blob(blob_handle) = value {
                    let blob = drm.get_property_blob(blob_handle).unwrap();
                    if let Ok(info_property) = Info::parse_edid(&blob) {
                        make = info_property.make();
                        model = info_property.model();
                        serial_number = info_property.serial();
                    }
                }
            }
        }

        let output_name = format!("{}-{}", info.interface().as_str(), info.interface_id());
        let monitor_cfg = state.config.monitors.iter().find(|m| m.name == output_name);

        let mode = monitor_cfg
            .and_then(|cfg| info.modes().iter().min_by_key(|m| (m.vrefresh() as f64 - cfg.refresh_rate).abs() as i64))
            .or_else(|| info.modes().first())
            .copied()
            .unwrap();

        let (mw, mh) = mode.size();

        let output = Output::new(
            output_name,
            PhysicalProperties {
                size: info.size()
                    .map(|(w, h)| (w as i32, h as i32).into())
                    .unwrap_or_default(),
                subpixel: info.subpixel().into(),
                make: if make.is_some() { make.unwrap() } else { "Unknown".to_string() },
                model: if model.is_some() { model.unwrap() } else { "Unknown".to_string() },
                serial_number: if serial_number.is_some() { serial_number.unwrap() } else { "Unknown".to_string() },
            },
        );
        let wl_mode = WlMode {
            size: (mw as i32, mh as i32).into(),
            refresh: mode.vrefresh() as i32 * 1000,
        };

        let (pos_x, pos_y) = monitor_cfg.map(|cfg| (cfg.x, cfg.y)).unwrap_or((x_offset, 0));
        let scale = monitor_cfg.map(|cfg| cfg.scale).unwrap_or(1.0);

        output.change_current_state(
            Some(wl_mode), 
            None, 
            Some(Scale::Fractional(scale)), 
            Some((pos_x, pos_y).into())
        );
        output.set_preferred(wl_mode);
        output.create_global::<AeroWM>(&state.display_handle);

        state.space.map_output(&output, (pos_x, pos_y));
        state.per_output_state.insert(output.clone(), ViewportState {
            viewport_x: pos_x as f64,
            viewport_y: pos_y as f64,
            zoom: 1.0,
            saved_tree_zoom: 1.0,
            saved_tree_viewport_x: 0.0,
            saved_tree_viewport_y: 0.0,
            view_mode: ViewMode::Tiling,
            tiling_visible_ids: Vec::new(),
        });

        if monitor_cfg.is_none() {
            x_offset += mw as i32;
        }

        for &encoder in info.encoders() {
            let encoder_info = drm.get_encoder(encoder).unwrap();
            let crtcs = resources.filter_crtcs(encoder_info.possible_crtcs());
            let Some(&crtc) = crtcs.iter().find(|c| !used_crtcs.contains(c)) else {
                continue;
            };
            let surface = drm.create_surface(crtc, mode, &[connector_handle])
                .expect("Failed to create DRM surface");

            let Ok(mut compositor) = GbmDrmCompositor::new(
                OutputModeSource::from(&output),
                surface,
                None,
                GbmAllocator::new(gbm.clone(), GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT),
                GbmFramebufferExporter::new(gbm.clone(), NodeFilter::None),
                [DrmFourcc::Xrgb8888],
                renderer.dmabuf_formats(),
                Size::from((state.config.cursor_size[0] as u32, state.config.cursor_size[1] as  u32)),
                Some(gbm.clone()),
            ) else {
                eprintln!("Skipping ctrc {:?}", crtc);
                continue;
            };

            compositor.render_frame(
                &mut renderer,
                &[] as &[SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>],
                Color32F::from([0.1, 0.1, 0.1, 1.0]),
                FrameFlags::empty(),
            ).expect("Initial render failed");
            compositor.queue_frame(()).expect("Initial queue failed");

            eprintln!("GPU ready, first frame queued for crtc {:?}", crtc);
            compositors.insert(crtc, compositor);
            crtc_to_output.insert(crtc, output.clone());
            used_crtcs.insert(crtc);
            break; // one compositor per connector
        }
    }

    state.gpu.insert(device_id, GpuData {
        fd: fd.device_fd(),
        drm,
        gbm,
        renderer,
        compositors,
        crtc_to_output,
        line_prog,
        solid_prog,
        border_prog,
        clip_prog,
    });
}

pub fn init_drm(event_loop: &mut EventLoop<'static, AeroWM>, state: &mut AeroWM) -> anyhow::Result<()> {
    eprintln!("Starting DRM init");
    let (session, notifier) = LibSeatSession::new()
        .map_err(|e| { eprintln!("Libseat error: {:?}", e); e })
        .expect("Failed to create libseat session");
    eprintln!("Session created");

    let handle = event_loop.handle();
    state.session = Some(session);
    handle.insert_source(notifier, |event, _, state| {
        match event {
            SessionEvent::PauseSession => {
                eprintln!("PauseSession fired");
                for gpu in state.gpu.values_mut() {
                    gpu.drm.pause();
                }
            }
            SessionEvent::ActivateSession => {
                eprintln!("ActivateSession fired");
                for gpu in state.gpu.values_mut() {
                    gpu.drm.activate(true).unwrap();
                    for compositor in gpu.compositors.values_mut() {
                        compositor.reset_buffers();
                        if state.zoom != 1.0 || state.zoom_animating {
                            compositor.reset_buffer_ages();
                        }
                        if let Err(e) = compositor.render_frame(
                            &mut gpu.renderer,
                            &[] as &[SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>],
                            Color32F::from([0.1, 0.1, 0.1, 1.0]),
                            FrameFlags::empty(),
                        ) { eprintln!("activate render_frame error: {:?}", e); }

                        if let Err(e) = compositor.queue_frame(()) {
                            eprintln!("activate queue_frame error: {:?}", e);
                        }

                    }
                }
            }
        }
    }).unwrap();

    let seat_name = state.session.as_ref().unwrap().seat();
    let udev_backend = UdevBackend::new(&seat_name).expect("Failed to initialize udev backend");
    eprintln!("Udev backend created");

    let existing: Vec<_> = udev_backend.device_list()
        .map(|(id, path)| (id, path.to_path_buf()))
        .collect();
    for (device_id, path) in existing {
        eprintln!("device_list entry: {:?}", path);
        if path.to_str().map(|s| s.starts_with("/dev/dri/card")).unwrap_or(false) {
            open_gpu(device_id, &path, state, &handle);
        }
    }

    let handle_inner = handle.clone();
    handle.insert_source(udev_backend, move |event, _, state| {
        if let UdevEvent::Added { device_id, path } = event {
            if path.to_str().map(|s| s.starts_with("/dev/dri/card")).unwrap_or(false) {
                open_gpu(device_id, &path, state, &handle_inner);
            }
        }
    }).unwrap();

    let interface = LibinputSessionInterface::from(state.session.as_ref().unwrap().clone());
    let mut context = Libinput::new_with_udev(interface);
    context.udev_assign_seat(&seat_name).unwrap();
    let libinput_backend = LibinputInputBackend::new(context.clone());

    eprintln!("calling input backend");
    handle.insert_source(libinput_backend, |event, _, state| {
        state.process_input_event(event);
    }).unwrap();
    eprintln!("input backend called");

    Ok(())
}
