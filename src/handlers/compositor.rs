use crate::{state::ClientState, AeroWM};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler, delegate_compositor, delegate_shm, desktop::layer_map_for_output, reexports::{wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge, wayland_server::{
        Client,
        protocol::{wl_buffer, wl_surface::WlSurface},
    }}, wayland::{
        buffer::BufferHandler,
        compositor::{
            CompositorClientState, CompositorHandler, CompositorState, get_parent, is_sync_subsurface
        },
        shm::{ShmHandler, ShmState},
    }, xwayland::XWaylandClientData
};

use super::xdg_shell;

impl CompositorHandler for AeroWM {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        if let Some(state) = client.get_data::<ClientState>() {
            return &state.compositor_state;
        }
        &client.get_data::<XWaylandClientData>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);
        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            let mut position_changed = false;
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            if let Some(window) = self
                .space
                .elements()
                .find(|w| w.toplevel().map_or(false, |t| t.wl_surface() == &root))
            {
                window.on_commit();
            }
            if let Some(cw) = self
                .windows
                .iter_mut()
                .find(|w| {
                    w.window.toplevel().map(|t| t.wl_surface() == surface).unwrap_or(false)
                        || w.window.x11_surface().and_then(|s| s.wl_surface()).map_or(false, |s| &s == surface)
                }) 
            {
                if cw.needs_center {
                    cw.canvas_x -= cw.window.geometry().size.w as f64 / 2.0;
                    cw.canvas_y -= cw.window.geometry().size.h as f64 / 2.0;
                    cw.needs_center = false;
                    position_changed = true;
                }
                if cw.resize_edge != ResizeEdge::None {
                    let current_w = cw.window.geometry().size.w;
                    let current_h = cw.window.geometry().size.h;

                    match cw.resize_edge {
                        ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft => {
                            let expected_x = cw.resize_initial_x + (cw.resize_initial_w - current_w) as f64;
                            if cw.canvas_x != expected_x {
                                cw.canvas_x = expected_x;
                                cw.target_x = expected_x;
                                position_changed = true;
                            }
                        }
                        _ => {}
                    }
                    match cw.resize_edge {
                        ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight => {
                            let expected_y = cw.resize_initial_y + (cw.resize_initial_h - current_h) as f64;
                            if cw.canvas_y != expected_y {
                                cw.canvas_y = expected_y;
                                cw.target_y = expected_y;
                                position_changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            if self.layer_surfaces.iter().any(|s| s.wl_surface() == surface) {
                let output = self.space.outputs().next().unwrap();
                layer_map_for_output(&output).arrange();
            }
            if position_changed { self.sync_window_positions(); }
        }

        xdg_shell::handle_commit(&mut self.popups, &self.space, surface);
    }
}

impl BufferHandler for AeroWM {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for AeroWM {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

delegate_compositor!(AeroWM);
delegate_shm!(AeroWM);
