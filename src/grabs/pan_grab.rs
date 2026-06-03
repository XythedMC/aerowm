use smithay::{
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
        GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
        GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
        GrabStartData as PointerGrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
        RelativeMotionEvent,
    },
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

use crate::AeroWM;

pub struct PanCanvasGrab {
    pub start_data: PointerGrabStartData<AeroWM>,
    pub initial_viewport_x: f64,
    pub initial_viewport_y: f64,
}

impl PointerGrab<AeroWM> for PanCanvasGrab {
    fn motion(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        handle.motion(data, None, event);

        let (min_x, min_y, max_x, max_y) = data.space.outputs()
            .filter_map(|o| data.space.output_geometry(o))
            .fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |(x0, y0, x1, y1), geo| {
                (x0.min(geo.loc.x), y0.min(geo.loc.y),
                    x1.max(geo.loc.x + geo.size.w), y1.max(geo.loc.y + geo.size.h))
        });
        data.cursor_position.x = event.location.x.clamp(min_x as f64, max_x as f64);
        data.cursor_position.y = event.location.y.clamp(min_y as f64, max_y as f64);

        // Divide screen-pixel delta by zoom so 1 mouse px = 1 screen px of canvas movement.
        let delta = event.location - self.start_data.location;
        data.viewport_x = self.initial_viewport_x - delta.x / data.current_viewport().2;
        data.viewport_y = self.initial_viewport_y - delta.y / data.current_viewport().2;
        if let Some(output) = data.output_under_cursor().cloned() {
            if let Some(vs) = data.per_output_state.get_mut(&output) {
                vs.viewport_x = data.viewport_x;
                vs.viewport_y = data.viewport_y;
            }
        }
        // Keep target/anim_start in sync so no animation fights the pan.
        data.viewport_target_x = data.viewport_x;
        data.viewport_target_y = data.viewport_y;
        data.viewport_anim_start_x = data.viewport_x;
        data.viewport_anim_start_y = data.viewport_y;
        data.sync_window_positions();
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
        const BTN_MIDDLE: u32 = 0x112;
        if !handle.current_pressed().contains(&BTN_MIDDLE) {
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

    fn gesture_swipe_begin(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut AeroWM,
        handle: &mut PointerInnerHandle<'_, AeroWM>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &PointerGrabStartData<AeroWM> {
        &self.start_data
    }

    fn unset(&mut self, _data: &mut AeroWM) {}
}
