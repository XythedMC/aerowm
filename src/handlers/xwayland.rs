use smithay::{
    desktop::Window, 
    input::pointer::{Focus, GrabStartData as PointerGrabStartData}, 
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge, 
    utils::{Logical, Point, Rectangle, SERIAL_COUNTER}, 
    wayland::xwayland_shell::XWaylandShellHandler, 
    xwayland::{X11Surface, X11Wm, XwmHandler, xwm::{Reorder, X11Window, XwmId}}
};
use crate::{AeroWM, grabs::ResizeSurfaceGrab, state::{CanvasWindow, ViewMode}};

impl XwmHandler for AeroWM {
    fn xwm_state(&mut self, _xwm: XwmId) -> &mut X11Wm {
        self.xwm.as_mut().unwrap()
    }

    fn new_window(&mut self, _xwm: XwmId, _window: X11Surface) {
        {}
    }

    fn new_override_redirect_window(&mut self, _xwm: XwmId, _window: X11Surface) {
        // nothing to do - handled in mapped_override_redirect_window
    }

    fn map_window_request(&mut self, _xwm: XwmId, window: X11Surface) {
        window.set_mapped(true).unwrap();
        let window = Window::new_x11_window(window);

        let id = self.alloc_id();
        tracing::info!("new_toplevel: id={id}");

        // Parent = currently focused window; None means new tree root.
        let parent_id = self.focused_window_id;

        let (initial_w, initial_h) = {
            let size = window.geometry().size;
            (size.w, size.h)
        };

        let initial_w = if initial_w < 10 { 800 } else { initial_w };
        let initial_h = if initial_h < 10 { 600 } else { initial_h };

        if let Some(x11) = window.x11_surface() {
            x11.configure(Rectangle {
                loc: (0, 0).into(),
                size: (initial_w, initial_h).into(),
            }).ok();
        }

        let canvas_x = self.cursor_position.x / self.zoom + self.viewport_x;
        let canvas_y = self.cursor_position.y / self.zoom + self.viewport_y;

        let screen_x = self.cursor_position.x as i32;
        let screen_y = self.cursor_position.y as i32;
        self.space.map_element(window.clone(), (screen_x, screen_y), true);
        self.space.raise_element(&window, true);

        // Register this window as a child of its parent.
        if let Some(pid) = parent_id {
            if let Some(parent) = self.windows.iter_mut().find(|cw| cw.id == pid) {
                parent.children.push(id);
            }
        }

        let z_index = self.z_counter;
        self.z_counter += 1;
        self.windows.push(CanvasWindow {
            id,
            window,
            canvas_x,
            canvas_y,
            target_x: canvas_x,
            target_y: canvas_y,
            anim_start_x: canvas_x,
            anim_start_y: canvas_y,
            parent_id,
            children: Vec::new(),
            tree_x: None,
            tree_y: None,
            tree_width: initial_w,
            tree_height: initial_h,
            base_width: initial_w,
            base_height: initial_h,
            resize_edge: ResizeEdge::None,
            resize_initial_x: 0.0,
            resize_initial_y: 0.0,
            resize_initial_w: 0,
            resize_initial_h: 0,
            is_fullscreen: false,
            pre_fullscreen_x: 0.0,
            pre_fullscreen_y: 0.0,
            pre_fullscreen_width: 0,
            pre_fullscreen_height: 0,
            z_index,
        });

        self.emit_event(crate::ipc::IpcEvent::WindowOpened {
            id: id.to_string(),
            parent: parent_id.map(|pid| pid.to_string()),
        });

        self.focus_by_id(id);
        self.print_tree();
        // In tree view, other windows are free-form — don't reposition them for a new window.
        // The new window is already placed at viewport center above.
        if self.view_mode == ViewMode::Tiling {
            self.apply_layout();
        }
    }

    fn mapped_override_redirect_window(&mut self, _xwm: XwmId, window: X11Surface) {
        window.set_mapped(true).unwrap();
        let loc = window.geometry().loc;
        let w = Window::new_x11_window(window);
        self.space.map_element(w.clone(), (loc.x, loc.y), false);
        self.x11_override_redirect.push(w);
    }

    fn unmapped_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(pos) = self.x11_override_redirect.iter().position(|w| {
            w.x11_surface().map_or(false, |s| s == &window)
        }) {
            let w = self.x11_override_redirect.remove(pos);
            self.space.unmap_elem(&w);
            return;
        }

        let Some(pos) = self.windows.iter().position(|cw| {
            cw.window.x11_surface().map_or(false, |s| s == &window)
        }) else { return; };

        let dead_id = self.windows[pos].id;
        let dead_parent_id = self.windows[pos].parent_id;
        let children: Vec<u32> = self.windows[pos].children.clone();

        // Re-parent orphans: they inherit the dead window's parent (or become roots).
        for &child_id in &children {
            if let Some(child) = self.windows.iter_mut().find(|cw| cw.id == child_id) {
                child.parent_id = dead_parent_id;
            }
        }

        // Update grandparent's children list: swap dead for its orphans.
        if let Some(pid) = dead_parent_id {
            if let Some(parent) = self.windows.iter_mut().find(|cw| cw.id == pid) {
                let insert_pos = parent.children.iter().position(|&id| id == dead_id);
                parent.children.retain(|&id| id != dead_id);
                if let Some(pos) = insert_pos {
                    for (i, &child_id) in children.iter().enumerate() {
                        parent.children.insert(pos + i, child_id);
                    }
                } else {
                    parent.children.extend_from_slice(&children);
                }
            }
        }
        // If dead window was a root, its orphans already have parent_id = None → they are roots.

        let dead_window = self.windows[pos].window.clone();
        self.space.unmap_elem(&dead_window);
        self.windows.remove(pos);
        self.emit_event(crate::ipc::IpcEvent::WindowClosed { id: dead_id.to_string() });

        // Update focus.
        if self.focused_window_id == Some(dead_id) {
            let new_focus = dead_parent_id
                .filter(|&pid| self.windows.iter().any(|cw| cw.id == pid))
                .or_else(|| self.windows.last().map(|cw| cw.id));

            match new_focus {
                Some(fid) => self.focus_by_id(fid),
                None => self.focus_clear(),
            }
        }

        if self.tiling_root_id == Some(dead_id) {
            self.tiling_root_id = self.focused_window_id;
        }

        self.print_tree();
        if self.view_mode == ViewMode::Tiling {
            self.apply_layout();
        }
    }

    fn destroyed_window(&mut self, _xwm: XwmId, window: X11Surface) {
        if let Some(pos) = self.x11_override_redirect.iter().position(|w| {
            w.x11_surface().map_or(false, |s| s == &window)
        }) {
            let w = self.x11_override_redirect.remove(pos);
            self.space.unmap_elem(&w);
            return;
        }

        let Some(pos) = self.windows.iter().position(|cw| {
            cw.window.x11_surface().map_or(false, |s| s == &window)
        }) else { return; };

        let dead_id = self.windows[pos].id;
        let dead_parent_id = self.windows[pos].parent_id;
        let children: Vec<u32> = self.windows[pos].children.clone();

        // Re-parent orphans: they inherit the dead window's parent (or become roots).
        for &child_id in &children {
            if let Some(child) = self.windows.iter_mut().find(|cw| cw.id == child_id) {
                child.parent_id = dead_parent_id;
            }
        }

        // Update grandparent's children list: swap dead for its orphans.
        if let Some(pid) = dead_parent_id {
            if let Some(parent) = self.windows.iter_mut().find(|cw| cw.id == pid) {
                let insert_pos = parent.children.iter().position(|&id| id == dead_id);
                parent.children.retain(|&id| id != dead_id);
                if let Some(pos) = insert_pos {
                    for (i, &child_id) in children.iter().enumerate() {
                        parent.children.insert(pos + i, child_id);
                    }
                } else {
                    parent.children.extend_from_slice(&children);
                }
            }
        }
        // If dead window was a root, its orphans already have parent_id = None → they are roots.

        let dead_window = self.windows[pos].window.clone();
        self.space.unmap_elem(&dead_window);
        self.windows.remove(pos);
        self.emit_event(crate::ipc::IpcEvent::WindowClosed { id: dead_id.to_string() });

        // Update focus.
        if self.focused_window_id == Some(dead_id) {
            let new_focus = dead_parent_id
                .filter(|&pid| self.windows.iter().any(|cw| cw.id == pid))
                .or_else(|| self.windows.last().map(|cw| cw.id));

            match new_focus {
                Some(fid) => self.focus_by_id(fid),
                None => self.focus_clear(),
            }
        }

        if self.tiling_root_id == Some(dead_id) {
            self.tiling_root_id = self.focused_window_id;
        }

        self.print_tree();
        if self.view_mode == ViewMode::Tiling {
            self.apply_layout();
        }
    }

    fn configure_request(
        &mut self,
        _xwm: XwmId,
        window: X11Surface,
        x: Option<i32>,
        y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        let geo = window.geometry();
        let new_x = x.unwrap_or(geo.loc.x);
        let new_y = y.unwrap_or(geo.loc.y);
        let new_w = w.map(|v| v as i32).unwrap_or(geo.size.w);
        let new_h = h.map(|v| v as i32).unwrap_or(geo.size.h);
        window.configure(Rectangle {
            loc: (new_x, new_y).into(),
            size: (new_w, new_h).into(),
        }).ok();
    }

    fn configure_notify(
        &mut self,
        _xwm: XwmId,
        _window: X11Surface,
        _geometry: Rectangle<i32, Logical>,
        _above: Option<X11Window>,
    ) {
        // X11 window notified us of its geometry — we don't need to act on this
        // because we drive positions through the canvas/space system ourselves.
    }

    fn resize_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32, resize_edge: smithay::xwayland::xwm::ResizeEdge) {
        let edge = match resize_edge {
            smithay::xwayland::xwm::ResizeEdge::Top         => ResizeEdge::Top,
            smithay::xwayland::xwm::ResizeEdge::Bottom      => ResizeEdge::Bottom,
            smithay::xwayland::xwm::ResizeEdge::Left        => ResizeEdge::Left,
            smithay::xwayland::xwm::ResizeEdge::Right       => ResizeEdge::Right,
            smithay::xwayland::xwm::ResizeEdge::TopLeft     => ResizeEdge::TopLeft,
            smithay::xwayland::xwm::ResizeEdge::TopRight    => ResizeEdge::TopRight,
            smithay::xwayland::xwm::ResizeEdge::BottomLeft  => ResizeEdge::BottomLeft,
            smithay::xwayland::xwm::ResizeEdge::BottomRight => ResizeEdge::BottomRight,
        };

        let pointer = self.seat.get_pointer().unwrap();
        let serial = SERIAL_COUNTER.next_serial();

        if let Some(cw) = self.windows.iter_mut().find(|cw| {
            cw.window.x11_surface().map_or(false, |s| s == &window)
        }) {
            cw.resize_edge = edge;
            cw.resize_initial_x = cw.canvas_x;
            cw.resize_initial_y = cw.canvas_y;
            cw.resize_initial_w = cw.base_width;
            cw.resize_initial_h = cw.base_height;
            let cw_id = cw.id;
            let initial_width = cw.base_width;
            let initial_height = cw.base_height;

            let grab = ResizeSurfaceGrab {
                start_data: PointerGrabStartData {
                    focus: None,
                    button: 0x110,
                    location: pointer.current_location(),
                },
                window_id: cw_id,
                initial_width,
                initial_height,
                grabbed_edge: edge,
                last_update: std::time::Instant::now(),
            };
            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn move_request(&mut self, _xwm: XwmId, window: X11Surface, _button: u32) {
        let pointer = self.seat.get_pointer().expect("No pointer/mouse connected or found");
        if let Some(cw) = self.windows.iter().find(|cw| {
            cw.window.x11_surface().map_or(false, |s| s == &window)
        }) {
            let offset = Point::new(
                pointer.current_location().x - cw.canvas_x,
                pointer.current_location().y - cw.canvas_y,
            );
            self.active_drag = true;
            self.dragged_window = Some((cw.id, offset));
        }
    }
}

impl XWaylandShellHandler for AeroWM {
    fn xwayland_shell_state(&mut self) -> &mut smithay::wayland::xwayland_shell::XWaylandShellState {
        &mut self.xwayland_shell_state
    }
}