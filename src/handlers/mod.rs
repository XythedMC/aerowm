mod compositor;
mod xdg_shell;
pub mod config;
pub mod cursor_shape;
pub mod tablet;
pub mod layer_shell;
mod xwayland;
use crate::AeroWM;

use smithay::{
    backend::allocator::dmabuf::Dmabuf, delegate_data_control, delegate_data_device, delegate_dmabuf, delegate_fractional_scale, delegate_idle_inhibit, delegate_idle_notify, delegate_image_capture_source, delegate_image_copy_capture, delegate_input_method_manager, delegate_output, delegate_output_capture_source, delegate_primary_selection, delegate_seat, delegate_text_input_manager, delegate_viewporter, delegate_xdg_activation, delegate_xdg_decoration, delegate_xwayland_shell, desktop::{PopupKind, PopupManager}, input::{
        Seat, SeatHandler, SeatState,
        dnd::{DnDGrab, DndGrabHandler, GrabType, Source},
        pointer::Focus,
    }, output::WeakOutput, reexports::{
        wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1,
        wayland_server::{Resource, protocol::{wl_shm::Format, wl_surface::WlSurface}},
    }, utils::{Buffer, Logical, Rectangle, Serial, Size}, wayland::{
        dmabuf::{DmabufHandler, DmabufState, ImportNotifier}, fractional_scale::FractionalScaleHandler, idle_inhibit::IdleInhibitHandler, idle_notify::{IdleNotifierHandler, IdleNotifierState}, image_capture_source::{ImageCaptureSource, ImageCaptureSourceHandler, OutputCaptureSourceHandler, OutputCaptureSourceState}, image_copy_capture::{BufferConstraints, Frame, ImageCopyCaptureHandler, ImageCopyCaptureState, Session, SessionRef}, input_method::{InputMethodHandler, PopupSurface}, output::OutputHandler, seat::WaylandFocus, selection::{
            SelectionHandler,
            data_device::{DataDeviceHandler, DataDeviceState, WaylandDndGrabHandler, set_data_device_focus},
            primary_selection::{PrimarySelectionHandler, PrimarySelectionState, set_primary_focus},
            wlr_data_control::{DataControlHandler, DataControlState},
        }, shell::xdg::{ToplevelSurface, decoration::XdgDecorationHandler}, xdg_activation::{XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData}
    }
};

impl SeatHandler for AeroWM {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<AeroWM> {
        &mut self.seat_state
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        image: smithay::input::pointer::CursorImageStatus,
    ) {
        self.cursor_icon = image;
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client.clone());
        set_primary_focus(dh, seat, client);
    }
}

delegate_xwayland_shell!(AeroWM);
delegate_seat!(AeroWM);

impl SelectionHandler for AeroWM {
    type SelectionUserData = ();
}

impl DataDeviceHandler for AeroWM {
    fn data_device_state(&mut self) -> &mut DataDeviceState {
        &mut self.data_device_state
    }
}

impl DndGrabHandler for AeroWM {}

impl WaylandDndGrabHandler for AeroWM {
    fn dnd_requested<S: Source>(
        &mut self,
        source: S,
        _icon: Option<WlSurface>,
        seat: Seat<Self>,
        serial: Serial,
        type_: GrabType,
    ) {
        match type_ {
            GrabType::Pointer => {
                let ptr = seat.get_pointer().unwrap();
                let start_data = ptr.grab_start_data().unwrap();
                let grab =
                    DnDGrab::new_pointer(&self.display_handle, start_data, source, seat);
                ptr.set_grab(self, grab, serial, Focus::Keep);
            }
            GrabType::Touch => {
                source.cancel();
            }
        }
    }
}

delegate_data_device!(AeroWM);

impl OutputHandler for AeroWM {}
delegate_output!(AeroWM);

impl PrimarySelectionHandler for AeroWM {
    fn primary_selection_state(&mut self) -> &mut PrimarySelectionState {
        &mut self.primary_selection_state
    }
}
delegate_primary_selection!(AeroWM);

impl XdgDecorationHandler for AeroWM {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            if self.config.client_side_decorations {
                state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ClientSide);
            } else {
                state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            }
        });
        toplevel.send_pending_configure();
    }
    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: zxdg_toplevel_decoration_v1::Mode) {
        toplevel.with_pending_state(|state| {
            if self.config.client_side_decorations {
                state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ClientSide);
            } else {
                state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            }
        });
        toplevel.send_pending_configure();
    }
    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            if self.config.client_side_decorations {
                state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ClientSide);
            } else {
                state.decoration_mode = Some(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            }
        });
        toplevel.send_pending_configure();
    }
}
delegate_xdg_decoration!(AeroWM);

impl XdgActivationHandler for AeroWM {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.activation_state
    }
    fn request_activation(&mut self, _token: XdgActivationToken, _data: XdgActivationTokenData, _surface: WlSurface) {}
}
delegate_xdg_activation!(AeroWM);

delegate_viewporter!(AeroWM);

impl FractionalScaleHandler for AeroWM {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        smithay::wayland::compositor::with_states(&surface, |states| {
            smithay::wayland::fractional_scale::with_fractional_scale(states, |fs| {
                fs.set_preferred_scale(1.0);
            });
        });
    }
}
delegate_fractional_scale!(AeroWM);

impl DmabufHandler for AeroWM {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }
    fn dmabuf_imported(&mut self, _global: &smithay::wayland::dmabuf::DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier) {
        // Queue for import on the next render frame where the renderer is available.
        self.pending_dmabufs.push((dmabuf, notifier));
    }
}
delegate_dmabuf!(AeroWM);

impl IdleNotifierHandler for AeroWM {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}
delegate_idle_notify!(AeroWM);

impl IdleInhibitHandler for AeroWM {
    fn inhibit(&mut self, _surface: WlSurface) {
        self.idle_notifier_state.set_is_inhibited(true);
    }

    fn uninhibit(&mut self, _surface: WlSurface) {
        self.idle_notifier_state.set_is_inhibited(false);
    }
}
delegate_idle_inhibit!(AeroWM);

impl DataControlHandler for AeroWM {
    fn data_control_state(&mut self) -> &mut DataControlState {
        &mut self.data_control_state
    }
}
delegate_data_control!(AeroWM);

impl ImageCaptureSourceHandler for AeroWM { }
delegate_image_capture_source!(AeroWM);

impl OutputCaptureSourceHandler for AeroWM {
    fn output_capture_source_state(&mut self) -> &mut OutputCaptureSourceState {
        &mut self.output_capture_source_state
    }

    fn output_source_created(&mut self, source: ImageCaptureSource, output: &smithay::output::Output) {
        source.user_data().insert_if_missing(|| output.downgrade());
    }
}
delegate_output_capture_source!(AeroWM);

impl ImageCopyCaptureHandler for AeroWM {
    fn image_copy_capture_state(&mut self) -> &mut ImageCopyCaptureState {
        &mut self.image_copy_capture_state
    }

    fn capture_constraints(&mut self, source: &ImageCaptureSource) -> Option<BufferConstraints> {
        let Some(output) = source.user_data().get::<WeakOutput>().and_then(|w| w.upgrade()) else { return None; };
        let mode = output.current_mode()?;
        let size: Size<i32, Buffer> = Size::from((mode.size.w, mode.size.h));
        let formats = vec![Format::Xrgb8888, Format::Argb8888];
        Some(BufferConstraints { 
            size: size, 
            shm: formats, 
            dma: None,
        })
    }

    fn new_session(&mut self, session: Session) {
        // AeroWM doesnt track sessions so empty body
        self.screencopy_sessions.push(session);
    }

    fn frame(&mut self, session: &SessionRef, frame: Frame) {
        eprintln!("framing frame to frame framers");
        let Some(output) = session.source().user_data().get::<WeakOutput>().and_then(|w| w.upgrade()) else { return; };
        self.pending_screencopy_frames.push((output, frame));
    }
}
delegate_image_copy_capture!(AeroWM);

impl InputMethodHandler for AeroWM {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface)) {
            eprintln!("Failed to track popup: {}", err);
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|parent| parent.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) { }

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.space
            .elements()
            .find_map(|w| (w.wl_surface().as_deref() == Some(parent)).then(|| w.geometry()))
            .unwrap_or_default()
    }
}
delegate_input_method_manager!(AeroWM);
delegate_text_input_manager!(AeroWM);