use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, GesturePinchUpdateEvent, GestureSwipeUpdateEvent, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent
    }, desktop::WindowSurfaceType, input::{
        keyboard::{FilterResult, Keysym},
        pointer::{AxisFrame, ButtonEvent, CursorIcon, CursorImageStatus, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    }, reexports::{wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge, wayland_server::protocol::wl_surface::WlSurface}, utils::{Logical, Point, Rectangle, SERIAL_COUNTER}, wayland::{compositor::get_parent, input_method::InputMethodSeat, shell::wlr_layer::{KeyboardInteractivity, Layer}},
};

use crate::{AeroWM, grabs::{PanCanvasGrab, ResizeSurfaceGrab}, ipc::IpcEvent, keybind::{Action, Trigger}, state::{CanvasWindow, ModifierKey, ViewMode}};
impl AeroWM {
    fn window_edge_at(
        &self,
        cw: &CanvasWindow,
        px: i32, py: i32,
    ) -> ResizeEdge {
        if self.focused_window_id.is_none() { return ResizeEdge::None }
        if cw.id != self.focused_window_id.unwrap() { return ResizeEdge::None }

        let (viewport_x, viewport_y, zoom) = self.current_viewport();
        let output_pos = self.output_under_cursor()
            .and_then(|o| self.space.output_geometry(o))
            .map(|g| g.loc).unwrap_or_default();

        let wx = ((cw.canvas_x - viewport_x) * zoom) as i32 + output_pos.x;
        let wy = ((cw.canvas_y - viewport_y) * zoom) as i32 + output_pos.y;
        let ww = (cw.base_width as f64 * zoom) as i32;
        let wh = (cw.base_height as f64 * zoom) as i32;
        
        let margin = 8;
        let in_right  = px >= wx + ww  && px < wx + ww + margin && py >= wy - margin && py <= wy + wh + margin;
        let in_left   = px >= wx - 4*margin       && px < wx      && py >= wy - margin && py <= wy + wh + margin;
        let in_bottom = py >= wy + wh  && py < wy + wh + margin && px >= wx - margin && px <= wx + ww + margin;
        let in_top    = py >= wy - 4*margin       && py < wy      && px >= wx - margin && px <= wx + ww + margin;


        if      in_right && in_bottom { return ResizeEdge::BottomRight; }
        else if in_right && in_top { return ResizeEdge::TopRight; }
        else if in_left && in_top { return ResizeEdge::TopLeft; }
        else if in_left && in_bottom { return ResizeEdge::BottomLeft; }
        else if in_right { return ResizeEdge::Right; }
        else if in_left { return ResizeEdge::Left; }
        else if in_bottom { return ResizeEdge::Bottom; }
        else if in_top { return ResizeEdge::Top; }
        return ResizeEdge::None;
    } 

    fn cursor_icon_for(
        &self,
        px: i32, py: i32,
    ) -> CursorImageStatus {
        if self.focused_window_id.is_none() { return CursorImageStatus::default_named() }
        for window in self.windows.iter().rev() {
            if window.id != self.focused_window_id.unwrap() {
                let (vx, vy, zoom) = self.current_viewport();
                let output_pos = self.output_under_cursor()
                    .and_then(|o| self.space.output_geometry(o))
                    .map(|g| g.loc).unwrap_or_default();
                let wx = ((window.canvas_x - vx) * zoom) as i32 + output_pos.x;
                let wy = ((window.canvas_y - vy) * zoom) as i32 + output_pos.y;
                let ww = (window.base_width as f64 * zoom) as i32;
                let wh = (window.base_height as f64 * zoom) as i32;
                if px >= wx && px < wx + ww && py >= wy && py < wy + wh {
                    return CursorImageStatus::default_named();
                }
                continue;
            }
            match self.window_edge_at(window, px, py) {
                ResizeEdge::None => {
                    let (vx, vy, zoom) = self.current_viewport();
                    let output_pos = self.output_under_cursor()
                        .and_then(|o| self.space.output_geometry(o))
                        .map(|g| g.loc).unwrap_or_default();
                    let wx = ((window.canvas_x - vx) * zoom) as i32 + output_pos.x;
                    let wy = ((window.canvas_y - vy) * zoom) as i32 + output_pos.y;
                    let ww = (window.base_width as f64 * zoom) as i32;
                    let wh = (window.base_height as f64 * zoom) as i32;
                    if px >= wx && px < wx + ww && py >= wy && py < wy + wh {
                        break;
                    }
                },
                ResizeEdge::Top         => { return CursorImageStatus::Named(CursorIcon::NResize); }
                ResizeEdge::Bottom      => { return CursorImageStatus::Named(CursorIcon::SResize);  }
                ResizeEdge::Left        => { return CursorImageStatus::Named(CursorIcon::WResize);  }
                ResizeEdge::TopLeft     => { return CursorImageStatus::Named(CursorIcon::NwResize); }
                ResizeEdge::BottomLeft  => { return CursorImageStatus::Named(CursorIcon::SwResize); }
                ResizeEdge::Right       => { return CursorImageStatus::Named(CursorIcon::EResize);  }
                ResizeEdge::TopRight    => { return CursorImageStatus::Named(CursorIcon::NeResize); }
                ResizeEdge::BottomRight => { return CursorImageStatus::Named(CursorIcon::SeResize); }
                _ => {}
            }
        }
        CursorImageStatus::default_named()
    }

    fn is_modifier_keysym(&self, sym: Keysym) -> bool {
        matches!(sym, Keysym::Shift_L | Keysym::Shift_R | Keysym::Alt_L | Keysym::Alt_R | 
                Keysym::Control_L | Keysym::Control_R | Keysym::Super_L | Keysym::Super_R)
    }

    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) { 
        self.idle_notifier_state.notify_activity(&self.seat);
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let key_state = event.state();
                
                let mut pending_action: Option<Action> = None;

                let keyboard = self.seat.get_keyboard().expect("Keyboard not found while trying to add it");
                let keyboard_grabbed = self.seat.input_method().keyboard_grabbed();

                if !keyboard_grabbed {
                    keyboard.input::<(), _>(
                        self,
                        event.key_code(),
                        key_state,
                        serial,
                        time,
                        |data, modifiers, handle| {
                            let sym = handle.modified_sym();
                            let raw_syms = handle.raw_syms();
                            let is_mod = data.is_modifier_keysym(sym);

                            if key_state == KeyState::Pressed {
                                for (keybind, action) in &data.config.keybinds {
                                    if let Trigger::Key(keysym) = keybind.trigger {
                                        if raw_syms.contains(&keysym) && data.mods_match(&keybind.mods, modifiers) {
                                            if let Action::ShowAreas = action {
                                                data.show_areas = true;
                                                data.modifier_combo_used = true;
                                                return FilterResult::Intercept(());
                                            }
                                            pending_action = Some(action.clone());
                                            data.modifier_combo_used = true;
                                            return FilterResult::Intercept(());
                                        }
                                    }
                                    if let Trigger::Modifiers = keybind.trigger {
                                        if is_mod && data.mods_match(&keybind.mods, modifiers) {
                                            data.modifier_action_armed = true;
                                            return FilterResult::Intercept(());
                                        }
                                    }
                                }
                                if !is_mod {
                                    data.modifier_combo_used = true;
                                }
                            } else {
                                for (keybind, action) in &data.config.keybinds {
                                    if let (Trigger::Key(keysym), Action::ShowAreas) = (&keybind.trigger, action) {
                                        if raw_syms.contains(keysym) {
                                            data.show_areas = false;
                                            break;
                                        }
                                    }
                                }
                                if is_mod && data.modifier_action_armed && !data.modifier_combo_used {
                                    let mut held = *modifiers;
                                    if matches!(sym, Keysym::Super_L | Keysym::Super_R) { held.logo  = true; }
                                    if matches!(sym, Keysym::Alt_L   | Keysym::Alt_R  ) { held.alt   = true; }
                                    if matches!(sym, Keysym::Shift_L | Keysym::Shift_R) { held.shift = true; }
                                    if matches!(sym, Keysym::Control_L | Keysym::Control_R) { held.ctrl = true; }
                                    for (keybind, action) in &data.config.keybinds {
                                        if let Trigger::Modifiers = keybind.trigger {
                                            if data.mods_match(&keybind.mods, &held) {
                                                pending_action = Some(action.clone());
                                                data.modifier_action_armed = false;
                                                data.modifier_combo_used   = false;
                                                return FilterResult::Intercept(());
                                            }
                                        }
                                    }
                                }
                                if is_mod {
                                    data.modifier_action_armed = false;
                                    data.modifier_combo_used = false;
                                }
                            }

                            FilterResult::Forward
                        },
                    );
                } else {
                    keyboard.input::<(), _>(
                        self, 
                        event.key_code(), 
                        key_state, 
                        serial, 
                        time, 
                        |_, _, _| 
                        {
                            return FilterResult::Forward;
                        }
                    );
                }

                if let Some(action) = pending_action {
                    self.dispatch_action(&action);
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let keyboard = self.seat.get_keyboard().expect("Keyboard not found - this is a bug");
                let pointer = self.seat.get_pointer().expect("No pointer/mouse connected or found");

                let speed = self.config.input_config.pointer_speed;
                let mut delta = event.delta();
                if let Some(acceleration) = &self.config.input_config.pointer_acceleration {
                    let accel = acceleration.call::<f64>(delta.x.hypot(delta.y)).expect("Couldn't call acceleration function");
                    delta.x *= accel;
                    delta.y *= accel;
                }
                self.cursor_position.x += delta.x * speed;
                self.cursor_position.y += delta.y * speed;

                let (_, _, zoom) = self.current_viewport();

                let (min_x, min_y, max_x, max_y) = self.space.outputs()
                    .filter_map(|o| self.space.output_geometry(o))
                    .fold((i32::MAX, i32::MAX, i32::MIN, i32::MIN), |(x0, y0, x1, y1), geo| {
                        (x0.min(geo.loc.x), y0.min(geo.loc.y),
                         x1.max(geo.loc.x + geo.size.w), y1.max(geo.loc.y + geo.size.h))
                    });
                self.cursor_position.x = self.cursor_position.x.clamp(min_x as f64, max_x as f64);
                self.cursor_position.y = self.cursor_position.y.clamp(min_y as f64, max_y as f64);
                
                if self.active_drag {
                    let (id, _) = self.dragged_window.unwrap();
                    self.windows.iter_mut().find(|cw| cw.id == id)
                        .map(|cw| {
                            cw.canvas_x += delta.x * speed / zoom;
                            cw.canvas_y += delta.y * speed / zoom;
                            cw.target_x = cw.canvas_x;
                            cw.target_y = cw.canvas_y;
                            cw.anim_start_x = cw.canvas_x;
                            cw.anim_start_y = cw.canvas_y;
                    });
                    self.sync_window_positions();
                    pointer.motion(self, None, &MotionEvent {
                        location: self.cursor_position,
                        serial,
                        time: event.time_msec(),
                    });
                    pointer.frame(self);
                    return;
                }
                if pointer.is_grabbed() {
                    pointer.motion(self, None, &MotionEvent {
                        location: self.cursor_position,
                        serial,
                        time: event.time_msec(),
                    });
                    pointer.frame(self);
                    return;
                }

                let (canvas_cx, canvas_cy) = self.cursor_to_canvas();

                // Overlay and Top shells sit above canvas windows — check them first.
                if let Some((surf, surf_pos)) = self.layer_surface_under(self.cursor_position, &[Layer::Overlay, Layer::Top]) {
                    pointer.motion(
                        self,
                        Some((surf, surf_pos)),
                        &MotionEvent { location: self.cursor_position, serial, time: event.time_msec() },
                    );
                    pointer.frame(self);
                    return;
                }

                self.cursor_icon = self.cursor_icon_for(
                    self.cursor_position.x as i32, 
                    self.cursor_position.y as i32, 
                );

                let cursor_output_name = self.output_under_cursor().map(|o| o.name());

                let Some(window) = self.windows.iter().find(|cw| {
                    (!cw.is_scratchpad || cw.scratchpad_visible) &&
                    (cw.canvas_x..(cw.canvas_x + cw.base_width as f64)).contains(&canvas_cx) &&
                    (cw.canvas_y..(cw.canvas_y + cw.base_height as f64)).contains(&canvas_cy) &&
                    &cw.output_name == &cursor_output_name
                }) else {
                    // No canvas window - fall back to Bottom/Background shells or nothing.
                    let result = self.layer_surface_under(self.cursor_position, &[Layer::Bottom, Layer::Background]);
                    pointer.motion(
                        self,
                        result,
                        &MotionEvent { location: self.cursor_position, serial, time: event.time_msec() },
                    );
                    pointer.frame(self);
                    return;
                };

                let local_x = canvas_cx - window.canvas_x;
                let local_y = canvas_cy - window.canvas_y;

                let local: Point<f64, Logical> = Point::new(local_x, local_y);
                let Some((surf, p)) = window.window.surface_under(local, WindowSurfaceType::ALL) else {
                    pointer.motion(
                        self,
                        None,
                        &MotionEvent {
                            location: self.cursor_position,
                            serial,
                            time: event.time_msec(),
                        }
                    );
                    pointer.frame(self);
                    return;
                };

                let global_pos = Point::new(
                    self.cursor_position.x - local_x + p.x as f64,
                    self.cursor_position.y - local_y + p.y as f64,
                );
                let under = (surf, global_pos);

                pointer.motion(
                    self,
                    Some(under.clone()),
                    &MotionEvent {
                        location: self.cursor_position,
                        serial,
                        time: event.time_msec(),
                    },
                );
                self.sync_window_positions();
                
                pointer.frame(self);
                // to allow focusing a subsurface
                let root = {
                    let mut s = under.0.clone();
                    while let Some(parent) = get_parent(&s) {
                        s = parent;
                    }
                    s
                };
                if let Some(window) = self.windows.iter().find(|cw| {
                    cw.window.toplevel().map_or(false, |t| t.wl_surface() == &root)
                    || cw.window.x11_surface().and_then(|s| s.wl_surface()).map_or(false, |s| s == root)
                }) {
                    let window_id = window.id;
                    if self.layer_surfaces
                        .iter()
                        .find(|surface|
                            surface.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                        ).is_none()
                    {
                        if self.config.hover_to_focus {
                            keyboard.set_focus(self, Some(under.0.clone()), serial);
                            self.focused_window_id = Some(window_id);
                        }
                    }
                }
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output = self.space.outputs().next().expect("No other monitors connected. Either went through all, or none are connected");
                let output_geo = self.space.output_geometry(output).expect("Monitor connected but not fully configured, so geometry couldnt be drawn");

                let pos =
                    event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let serial = SERIAL_COUNTER.next_serial();
                let pointer = self.seat.get_pointer().expect("No pointer/mouse connected or found");
                let under = self.surface_under(pos);
                let keyboard = self.seat.get_keyboard().expect("Keyboard not found - this is a bug");
                pointer.motion(
                    self,
                    under.clone(),
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
                
                self.cursor_icon = self.cursor_icon_for(
                    pointer.current_location().x as i32, 
                    pointer.current_location().y as i32, 
                );
                
                if let Some((wl_surf, _)) = under {    
                    if let Some(window) = self.windows.iter().find(|cw| {
                        cw.window.toplevel().map_or(false, |t| t.wl_surface() == &wl_surf)
                        || cw.window.x11_surface().and_then(|s| s.wl_surface()).map_or(false, |s| s == wl_surf)
                    }) {
                        let window_id = window.id;
                        if self.layer_surfaces
                            .iter()
                            .find(|surface|
                                surface.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                            ).is_none()
                        {
                            if self.config.hover_to_focus {
                                keyboard.set_focus(self, Some(wl_surf.clone()), serial);
                                self.focused_window_id = Some(window_id);
                            }
                        }
                    }
                }
            }
            InputEvent::PointerButton { event, .. } => {
                self.sync_window_positions();
                let pointer = self.seat.get_pointer().expect("No pointer/mouse connected or found");
                let keyboard = self.seat.get_keyboard().expect("Keyboard not found - this is a bug");
                let under = self.surface_under(self.cursor_position);

                let (viewport_x, viewport_y, zoom) = self.current_viewport();
                let (cx, cy) = self.cursor_to_canvas();
                let cursor_output_name = self.output_under_cursor().map(|o| o.name());

                let mods = keyboard.modifier_state();
                let main_mod = match self.main_modifier {
                    ModifierKey::Ctrl => mods.ctrl,
                    ModifierKey::Alt => mods.alt,
                    ModifierKey::Shift => mods.shift,
                    ModifierKey::Super => mods.logo,
                };

                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let button_state = event.state();

                if button_state == ButtonState::Pressed && self.modifier_action_armed {
                    for (keybind, action) in &self.config.keybinds {
                        if let Trigger::Button(mouse_btn) = &keybind.trigger {
                            if button == *mouse_btn as u32 && self.mods_match(&keybind.mods, &mods) {
                                let action = action.clone();
                                self.dispatch_action(&action);
                                return;
                            }
                        }
                    }
                    self.modifier_combo_used = true;
                }

                const BTN_MIDDLE: u32 = 0x112;
                const BTN_LEFT: u32 = 0x110;
                const BTN_RIGHT: u32 = 0x111;

                if ButtonState::Pressed == button_state && !pointer.is_grabbed()
                    && button == BTN_LEFT && self.marking_area.is_some() 
                {
                    let canvas_pos = Point::new(cx, cy);
                    self.marking_area_start = Some(canvas_pos);
                    return;
                }
                if ButtonState::Released == button_state && !pointer.is_grabbed()
                    && button == BTN_LEFT && self.marking_area.is_some() && self.marking_area_start.is_some()
                {
                    let id = self.marking_area.unwrap();
                    let start = self.marking_area_start.unwrap();
                    let end: Point<f64, Logical> = Point::new(cx, cy);
                    let rect: Rectangle<f64, Logical> = Rectangle::from_extremities(
                        (start.x.min(end.x), start.y.min(end.y)), 
                        (start.x.max(end.x), start.y.max(end.y)),
                    );
                    self.areas.insert(id, rect);
                    eprintln!("area {} saved: {:?}", id, rect);
                    self.marking_area = None;
                    self.marking_area_start = None;
                    return;
                }
                eprintln!("button pressed: main_mod: {}, under {:?}", main_mod, under.is_some());
                if ButtonState::Pressed == button_state && !pointer.is_grabbed()
                    && button == BTN_LEFT && main_mod && under.is_some()
                {
                    if let Some(window) = self.windows.iter().find(|cw| {
                        cw.output_name == cursor_output_name &&
                        (cw.canvas_x..(cw.canvas_x + cw.base_width as f64)).contains(&cx) &&
                        (cw.canvas_y..(cw.canvas_y + cw.base_height as f64)).contains(&cy)
                    }) {
                        eprintln!("drag started");
                        self.active_drag = true;
                        let offset: Point<f64, Logical> = Point::new(
                            pointer.current_location().x - window.canvas_x,
                            pointer.current_location().y - window.canvas_y,
                        );
                        self.dragged_window = Some((window.id, offset));
                        return;
                    } else {
                        eprintln!("drag: now window hit");
                    }
                }
                if ButtonState::Released == button_state && self.active_drag {
                    self.active_drag = false;
                    self.dragged_window = None;
                }

                if ButtonState::Pressed == button_state && !pointer.is_grabbed()
                    && button == BTN_RIGHT && main_mod && under.is_some()
                {
                    let px = pointer.current_location().x as i32;
                    let py = pointer.current_location().y as i32;

                    if let Some((ref surface, _pos)) = under {
                        if let Some(cw) = self
                            .windows
                            .iter_mut()
                            .find(|w| {
                                w.window.toplevel().map(|t| t.wl_surface() == surface).unwrap_or(false)
                                    || w.window.x11_surface().and_then(|s| s.wl_surface()).map_or(false, |s| &s == surface)
                            }) 
                        {
                            let wx = ((cw.canvas_x - viewport_x) * zoom) as i32;
                            let wy = ((cw.canvas_y - viewport_y) * zoom) as i32;
                            let ww = (cw.base_width as f64 * zoom) as i32;
                            let wh = (cw.base_height as f64 * zoom) as i32;
                            
                            let top_left_dist = (((px - wx).pow(2) + (py - wy).pow(2)) as f64).sqrt();
                            let top_right_dist = (((px - wx - ww).pow(2) + (py - wy).pow(2)) as f64).sqrt();
                            let bottom_left_dist = (((px - wx).pow(2) + (py - wy - wh).pow(2)) as f64).sqrt();
                            let bottom_right_dist = (((px - wx - ww).pow(2) + (py - wy - wh).pow(2)) as f64).sqrt();

                            let dists: [f64; 4] = [top_left_dist, top_right_dist, bottom_left_dist, bottom_right_dist];
                            let minimum = dists.iter().fold(f64::INFINITY, |a, &b| a.min(b));

                            let corner = if minimum == top_left_dist { ResizeEdge::TopLeft }
                                else if minimum == top_right_dist { ResizeEdge::TopRight }
                                else if minimum == bottom_left_dist { ResizeEdge::BottomLeft }
                                else { ResizeEdge::BottomRight };

                            cw.resize_edge = corner;
                            cw.resize_initial_x = cw.canvas_x;
                            cw.resize_initial_y = cw.canvas_y;
                            cw.resize_initial_w = cw.base_width;
                            cw.resize_initial_h = cw.base_height;

                            let initial_width = cw.base_width;
                            let initial_height = cw.base_height;

                            let grab = ResizeSurfaceGrab {
                                start_data: PointerGrabStartData {
                                    focus: None,
                                    button: BTN_RIGHT,
                                    location: pointer.current_location(),
                                },
                                window_id: cw.id,
                                initial_width,
                                initial_height,
                                grabbed_edge: corner,
                                last_update: std::time::Instant::now(),
                            };
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                        }
                    }
                }
                    
                if ButtonState::Pressed == button_state && !pointer.is_grabbed()
                    && button == BTN_RIGHT && !main_mod
                {
                    let hit = self.windows.iter().any(|cw| {
                        cw.output_name == cursor_output_name &&
                        (!cw.is_scratchpad || cw.scratchpad_visible) &&
                        (cw.canvas_x..(cw.canvas_x + cw.base_width as f64)).contains(&cx) &&
                        (cw.canvas_y..(cw.canvas_y + cw.base_height as f64)).contains(&cy)
                    }) || self.layer_surface_under(self.cursor_position, &[Layer::Overlay, Layer::Top]).is_some();
                    if !hit {
                        let (output_name, local_sx, local_sy) = self
                            .output_under_cursor()
                            .and_then(|o| self.space.output_geometry(o).map(|g| (o.name(), g.loc)))
                            .map(|(name, loc)| {
                                (name,
                                 self.cursor_position.x - loc.x as f64,
                                 self.cursor_position.y - loc.y as f64)
                            })
                            .unwrap_or_else(|| {
                                (String::new(), self.cursor_position.x, self.cursor_position.y)
                            });
                        self.emit_event(IpcEvent::CanvasRightClicked {
                            x: cx, y: cy,
                            sx: local_sx, sy: local_sy,
                            output: output_name,
                        });
                        return;
                    }
                }

                if ButtonState::Pressed == button_state && !pointer.is_grabbed()
                    && button == BTN_MIDDLE
                {
                    if self.current_view_mode() == ViewMode::TreeView {
                        let grab = PanCanvasGrab {
                            start_data: PointerGrabStartData {
                                focus: None,
                                button: BTN_MIDDLE,
                                location: pointer.current_location(),
                            },
                            initial_viewport_x: viewport_x,
                            initial_viewport_y: viewport_y,
                        };
                        pointer.set_grab(self, grab, serial, Focus::Clear);
                    }
                } else if ButtonState::Pressed == button_state && !pointer.is_grabbed()
                    && button == BTN_LEFT && !main_mod
                {
                    let top_layer_hit = self.layer_surface_under(
                        self.cursor_position,
                        &[Layer::Overlay, Layer::Top],
                    );
                    if let Some((surf, _)) = top_layer_hit {
                        keyboard.set_focus(self, Some(surf), serial);
                        self.focused_window_id = None;
                        pointer.button(self, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
                        pointer.frame(self);
                        return;
                    }

                    let px = pointer.current_location().x as i32;
                    let py = pointer.current_location().y as i32;
                    let output_info: Vec<(String, Point<i32, Logical>, f64, f64, f64)> = self.space.outputs()
                        .filter_map(|o| {
                            let loc = self.space.output_geometry(o)?.loc;
                            let vs = self.per_output_state.get(o)?;
                            Some((o.name(), loc, vs.viewport_x, vs.viewport_y, vs.zoom))
                        }).collect();
                    let found = self.windows.iter().rev().filter(|cw| !cw.is_scratchpad || cw.scratchpad_visible).find_map(|cw| {
                        match self.window_edge_at(cw, px, py) {
                            ResizeEdge::None => {
                                let (_, loc, vx, vy, z) = output_info.iter()
                                    .find(|(name, ..)| Some(name) == cw.output_name.as_ref())
                                    .or_else(|| output_info.first())?;
                                let wx = ((cw.canvas_x - vx) * z) as i32 + loc.x;
                                let wy = ((cw.canvas_y - vy) * z) as i32 + loc.y;
                                let ww = (cw.base_width as f64 * z) as i32;
                                let wh = (cw.base_height as f64 * z) as i32;
                                if px >= wx && px < wx + ww && py >= wy && py < wy + wh { Some((cw.id, ResizeEdge::None)) } else { None }
                            },
                            edge => Some((cw.id, edge)),
                        }
                    });
                    
                    if let Some((cw_id, edge)) = found {
                        if edge != ResizeEdge::None {
                            let cw = self.windows.iter_mut().find(|w| w.id == cw_id).unwrap();
                            cw.resize_edge = edge;
                            cw.resize_initial_x = cw.canvas_x;
                            cw.resize_initial_y = cw.canvas_y;
                            cw.resize_initial_w = cw.base_width;
                            cw.resize_initial_h = cw.base_height;

                            let initial_width = cw.base_width;
                            let initial_height = cw.base_height;

                            let grab = ResizeSurfaceGrab {
                                start_data: PointerGrabStartData {
                                    focus: None,
                                    button: BTN_LEFT,
                                    location: pointer.current_location(),
                                },
                                window_id: cw_id,
                                initial_width,
                                initial_height,
                                grabbed_edge: edge,
                                last_update: std::time::Instant::now(),
                            };
                            pointer.set_grab(self, grab, serial, Focus::Clear);
                        } else {
                            if self.layer_surfaces
                                .iter()
                                .find(|surface| 
                                    surface.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                                ).is_none() 
                            {
                                let z = self.z_counter;
                                self.z_counter += 1;
                                let cw = self.windows.iter_mut().find(|w| w.id == cw_id).unwrap();
                                cw.z_index = z;
                                let wl_surf = cw.window.toplevel().map(|t| t.wl_surface().clone())
                                    .or_else(|| cw.window.x11_surface().and_then(|s| s.wl_surface()))
                                    .unwrap();
                                let win_ref = cw.window.clone();
                                self.space.raise_element(&win_ref, true);
                                keyboard.set_focus(self, Some(wl_surf.clone()), serial);
                                self.focused_window_id = Some(cw_id);
                            }
                        }
                    } else {
                        let on_layer = under.as_ref().map(|(s, _)| {
                            self.layer_surfaces.iter().any(|ls| ls.wl_surface() == s)
                        }).unwrap_or(false);

                        if on_layer {
                            let surf = under.as_ref().map(|(s, _)| s.clone());
                            self.space.elements().for_each(|w| {
                                w.set_activated(false);
                                if let Some(t) = w.toplevel() { t.send_pending_configure(); }
                            });
                            self.focused_window_id = None;
                            keyboard.set_focus(self, surf, serial);
                        } else {
                            self.space.elements().for_each(|w| {
                                w.set_activated(false);
                                if let Some(t) = w.toplevel() { t.send_pending_configure(); }
                            });
                            if self.layer_surfaces
                                .iter()
                                .find(|surface|
                                    surface.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                                ).is_none()
                            {
                                keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                                self.focused_window_id = None;
                            }
                        }
                    }            
                } else if ButtonState::Pressed == button_state && !pointer.is_grabbed() && !self.active_drag {

                    if let Some((surf, _)) = self.layer_surface_under(self.cursor_position, &[Layer::Overlay, Layer::Top]) {
                        keyboard.set_focus(self, Some(surf), serial);
                        self.focused_window_id = None;
                        pointer.button(self, &ButtonEvent { button, state: button_state, serial, time: event.time_msec() });
                        pointer.frame(self);
                        return;
                    }

                    let hit = self.windows.iter().find(|cw| {
                        cw.output_name == cursor_output_name &&
                        (!cw.is_scratchpad || cw.scratchpad_visible) &&
                        (cw.canvas_x..(cw.canvas_x + cw.base_width as f64)).contains(&cx) &&
                        (cw.canvas_y..(cw.canvas_y + cw.base_height as f64)).contains(&cy)
                    });

                    if let Some(cw) = hit {
                        let hit_id = cw.id;
                        let z = self.z_counter;
                        self.z_counter += 1;
                        let cw = self.windows.iter_mut().find(|w| w.id == hit_id).unwrap();
                        cw.z_index = z;
                        let win_ref = cw.window.clone();
                        let wl_surf = cw.window.toplevel().map(|t| t.wl_surface().clone())
                            .or_else(|| cw.window.x11_surface().and_then(|s| s.wl_surface()))
                            .unwrap();
                        self.space.raise_element(&win_ref, true);
                        if self.layer_surfaces
                            .iter()
                            .find(|surface| 
                                surface.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                            ).is_none() 
                        {
                            keyboard.set_focus(self, Some(wl_surf.clone()), serial);

                            self.focused_window_id = self
                                .windows
                                .iter()
                                .find(|cw| {
                                    cw.window.toplevel().map_or(false, |t| t.wl_surface() == &wl_surf)
                                    || cw.window.x11_surface().and_then(|s| s.wl_surface()).map_or(false, |s| s == wl_surf)
                                })
                                .map(|cw| cw.id);
                        }
                        match self.current_view_mode() {
                            ViewMode::Tiling => {
                                self.apply_layout();
                                self.space.elements().for_each(|window| {
                                    if let Some(t) = window.toplevel() { t.send_pending_configure(); }
                                });
                            }
                            ViewMode::TreeView => {}
                            ViewMode::Fullscreen => {}
                        }
                    } else if under.clone().is_some() && 
                        self.layer_surfaces.iter().any(|s| s.wl_surface() == &under.as_ref().unwrap().0) 
                    {
                        let (surface, _) = under.clone().unwrap();
                        keyboard.set_focus(self, Some(surface), serial);
                        self.focused_window_id = None;
                    } else {
                        self.space.elements().for_each(|window| {
                            window.set_activated(false);
                            if let Some(t) = window.toplevel() { t.send_pending_configure(); }
                        });
                        if self.layer_surfaces
                            .iter()
                            .find(|surface| 
                                surface.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                            ).is_none() 
                        {
                            keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                            self.focused_window_id = None;
                        }
                        if self.current_view_mode() == ViewMode::Tiling {
                            self.apply_layout();
                        }
                    }
                }
                eprintln!("pointer focus: {:?}", pointer.current_focus());
                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let mods = self.seat.get_keyboard().unwrap().modifier_state();
                let main_mod = match self.main_modifier {
                    ModifierKey::Ctrl => mods.ctrl,
                    ModifierKey::Alt => mods.alt,
                    ModifierKey::Shift => mods.shift,
                    ModifierKey::Super => mods.logo,
                };

                let source = event.source();
                let horizontal_amount = event.amount(Axis::Horizontal).unwrap_or_else(|| {
                    event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.
                });
                let vertical_amount = event.amount(Axis::Vertical).unwrap_or_else(|| {
                    event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.
                });
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                if main_mod && self.current_view_mode() == ViewMode::TreeView && vertical_amount != 0.0 {
                    if let Some(output) = self.output_under_cursor().cloned() {
                        if let Some(vs) = self.per_output_state.get_mut(&output) {
                            let old_zoom = vs.zoom;
                            let zoom_factor = 1.1_f64.powf(-vertical_amount / 15.0);
                            vs.zoom = (vs.zoom * zoom_factor).clamp(0.2, 5.0);
                            self.zoom = vs.zoom;
                            self.zoom_target = vs.zoom;

                            let output_x = self.space.output_geometry(&output).map(|g| g.loc.x as f64).unwrap_or(0.0);
                            let output_y = self.space.output_geometry(&output).map(|g| g.loc.y as f64).unwrap_or(0.0);
                            vs.viewport_x += (self.cursor_position.x - output_x) * (1.0 / old_zoom - 1.0 / vs.zoom);
                            vs.viewport_y += (self.cursor_position.y - output_y) * (1.0 / old_zoom - 1.0 / vs.zoom);

                            self.viewport_target_x = vs.viewport_x;
                            self.viewport_target_y = vs.viewport_y;
                            self.viewport_anim_start_x = vs.viewport_x;
                            self.viewport_anim_start_y = vs.viewport_y;
                        }
                    }

                    self.sync_window_positions();
                    return;
                } else {
                    let mut frame = AxisFrame::new(event.time_msec()).source(source);
                    if horizontal_amount != 0.0 {
                        frame = frame.value(Axis::Horizontal, horizontal_amount);
                        if let Some(discrete) = horizontal_amount_discrete {
                            frame = frame.v120(Axis::Horizontal, discrete as i32);
                        }
                    }
                    if vertical_amount != 0.0 {
                        frame = frame.value(Axis::Vertical, vertical_amount);
                        if let Some(discrete) = vertical_amount_discrete {
                            frame = frame.v120(Axis::Vertical, discrete as i32);
                        }
                    }
                    if source == AxisSource::Finger {
                        if event.amount(Axis::Horizontal) == Some(0.0) {
                            frame = frame.stop(Axis::Horizontal);
                        }
                        if event.amount(Axis::Vertical) == Some(0.0) {
                            frame = frame.stop(Axis::Vertical);
                        }
                    }

                    let pointer = self.seat.get_pointer().expect("No pointer/mouse connected or found");
                    pointer.axis(self, frame);
                    pointer.frame(self);
                }
            }
            InputEvent::GestureSwipeUpdate { event, .. } => {
                self.pan(event.delta_x(), event.delta_y());
                self.sync_window_positions();
            }
            InputEvent::GesturePinchUpdate { event, .. } => {
                if let Some(output) = self.output_under_cursor().cloned() {
                    if let Some(vs) = self.per_output_state.get_mut(&output) {
                        let old_zoom = vs.zoom;
                        let zoom_factor = event.scale() / self.pinch_last_scale;
                        vs.zoom = (vs.zoom * zoom_factor).clamp(0.2, 5.0);
                        self.zoom = vs.zoom;
                        self.zoom_target = vs.zoom;

                        let output_x = self.space.output_geometry(&output).map(|g| g.loc.x as f64).unwrap_or(0.0);
                        let output_y = self.space.output_geometry(&output).map(|g| g.loc.y as f64).unwrap_or(0.0);
                        vs.viewport_x += (self.cursor_position.x - output_x) * (1.0 / old_zoom - 1.0 / vs.zoom);
                        vs.viewport_y += (self.cursor_position.y - output_y) * (1.0 / old_zoom - 1.0 / vs.zoom);

                        self.viewport_target_x = vs.viewport_x;
                        self.viewport_target_y = vs.viewport_y;
                        self.viewport_anim_start_x = vs.viewport_x;
                        self.viewport_anim_start_y = vs.viewport_y;
                    }
                    self.pinch_last_scale = event.scale();
                }
            }
            InputEvent::GesturePinchBegin { event: _, .. } => {
                self.pinch_last_scale = 1.0;
            }
            _ => {}
        }
    }
}
