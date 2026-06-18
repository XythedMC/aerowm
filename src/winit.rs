use std::time::Duration;

use smithay::{
    backend::{
        renderer::{
            ImportDma, ImportEgl, damage::OutputDamageTracker,
        }, winit::{self, WinitEvent}
    }, desktop::{Window, layer_map_for_output}, input::pointer::{CursorIcon, CursorImageStatus}, output::{Mode, Output, PhysicalProperties, Subpixel}, reexports::calloop::EventLoop, utils::{Rectangle, Transform}
};

use crate::{AeroWM, rendering, state::{AeroWMElement, BackgroundType, ViewportState}};

// ── Shader sources ─────────────────────────────────────────────────────────────
// compile_custom_pixel_shader prepends "#version 100\n" — do NOT include it here.

/// Straight-line connector with anti-aliased edges and endpoint dots.
/// Uses highp + premultiplied alpha output (Smithay blends with GL_ONE, GL_ONE_MINUS_SRC_ALPHA).

// ── Winit backend init ─────────────────────────────────────────────────────────

pub fn init_winit(
    event_loop: &mut EventLoop<AeroWM>,
    state: &mut AeroWM,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut backend, winit) = winit::init()?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (800, 600).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
            serial_number: "Unknown".into(),
        },
    );
    state.scale = output.current_scale().fractional_scale();
    let _global = output.create_global::<AeroWM>(&state.display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));
    state.per_output_state.insert(output.clone(), ViewportState::default());

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    let line_prog  = rendering::compile_line(backend.renderer());
    let solid_prog = rendering::compile_solid(backend.renderer());
    let border_prog = rendering::compile_border(backend.renderer());

    {
        let formats = backend.renderer().dmabuf_formats();
        let _ = state.dmabuf_state.create_global::<AeroWM>(&state.display_handle, formats);
    }

    match backend.renderer().bind_wl_display(&state.display_handle) {
        Ok(_)  => tracing::info!("EGL bound to Wayland display (wl_drm/EGLStream enabled)"),
        Err(e) => tracing::warn!("bind_wl_display failed (ok on non-EGL paths): {e}"),
    }

    backend.window().request_redraw();

    event_loop
        .handle()
        .insert_source(winit, move |event, _, state| {
            match event {
                WinitEvent::Resized { size, .. } => {
                    output.change_current_state(
                        Some(Mode { size, refresh: 60_000 }),
                        None,
                        None,
                        None,
                    );
                }
                WinitEvent::Input(event) => state.process_input_event(event),
                WinitEvent::Redraw => {
                    state.tick_animation();
                    match &state.cursor_icon {
                        CursorImageStatus::Hidden => backend.window().set_cursor_visible(false),
                        CursorImageStatus::Named(cursor_icon) => backend.window().set_cursor(*cursor_icon),
                        CursorImageStatus::Surface(_surface) => backend.window().set_cursor(CursorIcon::Default),
                    }
                    output.change_current_state(None, None, Some(smithay::output::Scale::Fractional(state.zoom)), None);

                    let pending = std::mem::take(&mut state.pending_dmabufs);
                    for (dmabuf, notifier) in pending {
                        match backend.renderer().import_dmabuf(&dmabuf, None) {
                            Ok(_) => { let _ = notifier.successful::<AeroWM>(); }
                            Err(e) => {
                                tracing::warn!("dmabuf import failed: {e}");
                                notifier.failed();
                            }
                        }
                    }

                    let size   = backend.window_size();
                    let damage = Rectangle::from_size(size);
                    {
                        let (renderer, mut framebuffer) = match backend.bind() {
                            Ok(v)  => v,
                            Err(e) => { eprintln!("AeroWM: bind error: {e}"); return; }
                        };

                        let background_color = state.config.background_color;
                        let color = background_color.map(|x| x as f32 / 255.0);
                        let Some(vs) = state.per_output_state.get(&output) else { return; };
                        let overlays = rendering::build_render_elements(
                            &state.windows,
                            &state.x11_override_redirect,
                            &state.space,
                            vs.view_mode,
                            &state.tiling_visible_ids,
                            state.scale,
                            state.zoom,
                            state.viewport_x,
                            state.viewport_y,
                            &state.config,
                            &state.show_areas,
                            &state.areas,
                            state.cursor_position,
                            &None,
                            &state.background_texture,
                            &state.background_shader_prog,
                            &state.background_image_size,
                            state.start_time.elapsed().as_secs_f32(),
                            renderer, 
                            &line_prog, 
                            &solid_prog, 
                            &border_prog
                        );

                        if let Err(e) = smithay::desktop::space::render_output::<
                            _,
                            AeroWMElement,
                            Window,
                            _,
                        >(
                            &output,
                            renderer,
                            &mut framebuffer,
                            1.0,
                            0,
                            [],
                            &overlays,
                            &mut damage_tracker,
                            if state.background_type != BackgroundType::Color { [0.1, 0.1, 0.1, 1.0] } else { [color[0], color[1], color[2], 1.0] },
                        ) {
                            eprintln!("AeroWM: render error: {e}");
                            return;
                        }
                    }

                    if let Err(e) = backend.submit(Some(&[damage])) {
                        eprintln!("AeroWM: submit error: {e}");
                        return;
                    }

                    state.space.elements().for_each(|window| {
                        window.send_frame(
                            &output,
                            state.start_time.elapsed(),
                            Some(Duration::ZERO),
                            |_, _| Some(output.clone()),
                        )
                    });

                    // Send frames to layer surfaces and refresh the layer map.
                    {
                        let layer_map = layer_map_for_output(&output);
                        for layer in layer_map.layers() {
                            layer.send_frame(
                                &output,
                                state.start_time.elapsed(),
                                Some(Duration::ZERO),
                                |_, _| Some(output.clone()),
                            );
                        }
                    }

                    state.space.refresh();
                    layer_map_for_output(&output).cleanup();
                    state.popups.cleanup();
                    let _ = state.display_handle.flush_clients();

                    backend.window().request_redraw();
                }
                WinitEvent::CloseRequested => {
                    state.loop_signal.stop();
                }
                _ => (),
            };
        })?;

    Ok(())
}
