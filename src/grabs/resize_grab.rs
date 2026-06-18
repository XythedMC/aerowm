use smithay::{
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
        GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
        GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
        GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
        RelativeMotionEvent,
    },
    reexports::{wayland_server::protocol::wl_surface::WlSurface,
                wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge},
    utils::{Logical, Point, Rectangle},
};
use crate::AeroWM;

pub struct ResizeSurfaceGrab {
    pub start_data: PointerGrabStartData<AeroWM>,
    pub window_id: u32,
    pub initial_width: i32,
    pub initial_height: i32,
    pub grabbed_edge: ResizeEdge,
    pub last_update: std::time::Instant,
}

impl PointerGrab<AeroWM> for ResizeSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        _focus: Option<(<AeroWM as smithay::input::SeatHandler>::PointerFocus, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);
        eprintln!("resizing window");

        let zoom = data.current_viewport().2;

        let raw_delta = event.location - self.start_data.location;
        let dx = (raw_delta.x / zoom) as i32;
        let dy = (raw_delta.y / zoom) as i32;

        let mut new_width = self.initial_width;
        let mut new_height = self.initial_height;

        match self.grabbed_edge {
            ResizeEdge::Bottom      => { new_height = (self.initial_height + dy).max(128); }
            ResizeEdge::Top         => { new_height = (self.initial_height - dy).max(128); }
            ResizeEdge::Right       => { new_width  = (self.initial_width  + dx).max(128); }
            ResizeEdge::Left        => { new_width  = (self.initial_width  - dx).max(128); }
            ResizeEdge::BottomRight => { new_width  = (self.initial_width  + dx).max(128); new_height = (self.initial_height + dy).max(128); }
            ResizeEdge::BottomLeft  => { new_width  = (self.initial_width  - dx).max(128); new_height = (self.initial_height + dy).max(128); }
            ResizeEdge::TopRight    => { new_width  = (self.initial_width  + dx).max(128); new_height = (self.initial_height - dy).max(128); }
            ResizeEdge::TopLeft     => { new_width  = (self.initial_width  - dx).max(128); new_height = (self.initial_height - dy).max(128); }
            _ => {}
        };

        let now = std::time::Instant::now();
        let should_update = now.duration_since(self.last_update).as_millis() >= 16;

        let shifts_left = matches!(self.grabbed_edge, ResizeEdge::Left | ResizeEdge::BottomLeft | ResizeEdge::TopLeft);
        let shifts_top = matches!(self.grabbed_edge, ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight);

        if let Some(cw) = data.windows.iter_mut().find(|cw| cw.id == self.window_id) {
            if cw.base_width != new_width || cw.base_height != new_height {
                cw.base_width  = new_width;
                cw.base_height = new_height;
                cw.tree_width  = new_width;
                cw.tree_height = new_height;

                if shifts_left {
                    cw.canvas_x = cw.resize_initial_x + (cw.resize_initial_w - new_width) as f64;
                    cw.target_x = cw.canvas_x;
                    cw.anim_start_x = cw.canvas_x;
                }
                if shifts_top {
                    cw.canvas_y = cw.resize_initial_y + (cw.resize_initial_h - new_height) as f64;
                    cw.target_y = cw.canvas_y;
                    cw.anim_start_y = cw.canvas_y;
                }
                if should_update {
                    if let Some(tl) = cw.window.toplevel() {
                        tl.with_pending_state(|s| { s.size = Some((new_width, new_height).into()); });
                        tl.send_pending_configure();
                    } else if let Some(x11) = cw.window.x11_surface() {
                        let _ = x11.configure(Some(Rectangle::new((0, 0).into(), (new_width, new_height).into())));
                    }
                }
            }
        }

        if should_update {
            self.last_update = now;
        }

        let _ = data.display_handle.flush_clients();
    }

    fn relative_motion(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        if !handle.current_pressed().contains(&self.start_data.button) {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GestureSwipeBeginEvent) { handle.gesture_swipe_begin(data, event); }
    fn gesture_swipe_update(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GestureSwipeUpdateEvent) { handle.gesture_swipe_update(data, event); }
    fn gesture_swipe_end(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GestureSwipeEndEvent) { handle.gesture_swipe_end(data, event); }
    fn gesture_pinch_begin(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GesturePinchBeginEvent) { handle.gesture_pinch_begin(data, event); }
    fn gesture_pinch_update(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GesturePinchUpdateEvent) { handle.gesture_pinch_update(data, event); }
    fn gesture_pinch_end(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GesturePinchEndEvent) { handle.gesture_pinch_end(data, event); }
    fn gesture_hold_begin(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GestureHoldBeginEvent) { handle.gesture_hold_begin(data, event); }
    fn gesture_hold_end(&mut self, data: &mut AeroWM, handle: &mut PointerInnerHandle<'_, AeroWM>, event: &GestureHoldEndEvent) { handle.gesture_hold_end(data, event); }

    fn unset(&mut self, data: &mut AeroWM) {
        if let Some(cw) = data.windows.iter_mut().find(|cw| cw.id == self.window_id) {
            cw.resize_edge = ResizeEdge::None;
        }
    }

    fn start_data(&self) -> &PointerGrabStartData<AeroWM> {
        &self.start_data
    }
}
