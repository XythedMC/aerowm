use std::fs::{remove_file};

use anyhow::{Error, anyhow};
use smithay::{backend::session::Session, input::keyboard::{Keysym, ModifiersState, xkb::{KEYSYM_CASE_INSENSITIVE, keysym_from_name}}, utils::Point};

use crate::{AeroWM, state::{CanvasWindow, ModifierKey, ViewMode}};

#[derive(Debug, Clone)]
pub struct ParsedKeybind {
    pub mods: Vec<ModifierKey>,
    pub trigger: Trigger,
}

#[derive(Debug, Clone)]
pub enum Trigger {
    Key(Keysym),
    Button(MouseButtons),
    Modifiers,
}
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum MouseButtons {
    BtnLeft = 0x110u32,
    BtnMiddle = 0x112u32,
    BtnRight = 0x111u32,
}

#[derive(Debug, Clone)]
pub enum Action {
    Close,
    Exec(String),
    Quit,
    SwitchView,
    Fullscreen,
    Parent,
    Child,
    Sibling,
    FocusZoom,
    ResetView,
    Pan(f64, f64),
    Resize(i32, i32),
    SwitchVT(u8),
    MarkArea,
    GoToArea(u32),
    RemoveArea,
    ShowAreas,
    MoveToNextOutput,
    SendToScratchpad,
    ToggleScratchpad,
    SwitchLayout,
}

pub fn parse_keybind(s: &str) -> Result<ParsedKeybind, Error> {
    let parts: Vec<String> = s.split("+").map(|string| string.trim().to_lowercase()).collect();
    if !parts.contains(&String::from("ctrl")) && !parts.contains(&String::from("alt")) && !parts.contains(&String::from("shift")) && !parts.contains(&String::from("super")) {
        return Err(anyhow!(format!("modifiers for keybind {} dont exist", s)))
    } 
    let mut mods: Vec<ModifierKey> = Vec::new();
    let mut trigger: Option<Trigger> = None;
    for part in parts {
        match part.as_str() {
            "ctrl" => mods.push(ModifierKey::Ctrl),
            "alt" => mods.push(ModifierKey::Alt),
            "shift" => mods.push(ModifierKey::Shift),
            "super" => mods.push(ModifierKey::Super),
            name => {
                if trigger.is_some() {
                    return Err(anyhow!("more than one key in keybind"))
                }

                trigger = Some(match name {
                    "leftclick" | "lmb" => Trigger::Button(MouseButtons::BtnLeft),
                    "middleclick" | "mmb" => Trigger::Button(MouseButtons::BtnMiddle),
                    "rightclick" | "rmb" => Trigger::Button(MouseButtons::BtnRight),
                    key => Trigger::Key(keysym_from_name(key, KEYSYM_CASE_INSENSITIVE))
                });
            }
        }
    }
    let has_shift = mods.contains(&ModifierKey::Shift);
    let trigger = trigger.unwrap_or(Trigger::Modifiers);
    let trigger = if has_shift {
        if let Trigger::Key(sym) = trigger {
            let raw = sym.raw();
            if raw >= 0x61 && raw <= 0x7a {
                Trigger::Key(Keysym::new(raw - 0x20))
            } else {
                Trigger::Key(sym)
            }
        } else {
            trigger
        }
    } else {
        trigger
    };
    Ok(ParsedKeybind { mods, trigger })
}

pub fn parse_action(action: &str, args: Option<String>) -> Result<Action, Error> {
    match action.to_lowercase().as_str() {
        "close" => Ok(Action::Close),
        "exec" => {
            if args.is_some() { Ok(Action::Exec(args.unwrap()))}
            else { Err(anyhow!("need arguments for this type of action"))}
        },
        "quit" => Ok(Action::Quit),
        "switch_view" => Ok(Action::SwitchView),
        "fullscreen" => Ok(Action::Fullscreen),
        "parent" => Ok(Action::Parent),
        "child" => Ok(Action::Child),
        "sibling" => Ok(Action::Sibling),
        "focus_zoom" => Ok(Action::FocusZoom),
        "reset_view" => Ok(Action::ResetView),
        "pan" => {
            let arg = args.ok_or_else(|| anyhow!("pan requires an argument"))?;
            let mut parts = arg.splitn(2, ' ');
            let x = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| anyhow!("invalid pan x"))?;
            let y = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| anyhow!("invalid pan y"))?;
            Ok(Action::Pan(x, y))
        },
        "resize" => {
            let arg = args.ok_or_else(|| anyhow!("resize requires an argument"))?;
            let mut parts = arg.splitn(2, ' ');
            let x = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| anyhow!("invalid resize x"))?;
            let y = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| anyhow!("invalid resize y"))?;
            Ok(Action::Resize(x, y))
        },
        "switch_vt" => {
            let n = args.ok_or_else(|| anyhow!("switch_vt needs a number"))?
                .parse::<u8>().map_err(|e| anyhow!(e.to_string()))?;
            Ok(Action::SwitchVT(n))
        },
        "mark_area" => Ok(Action::MarkArea),
        "goto_area" => {
            let n = args.ok_or_else(|| anyhow!("goto_area needs a number"))?
                .parse::<u32>().map_err(|_| anyhow!("invalid area number"))?;
            Ok(Action::GoToArea(n))
        },
        "remove_area" => Ok(Action::RemoveArea),
        "show_areas" => Ok(Action::ShowAreas),
        "move_to_next_output" => Ok(Action::MoveToNextOutput),
        "send_to_scratchpad" => Ok(Action::SendToScratchpad),
        "toggle_scratchpad" => Ok(Action::ToggleScratchpad),
        "switch_layout" => Ok(Action::SwitchLayout),
        _ => Err(anyhow!("action type not supported"))
    }
}

impl AeroWM {
    pub fn dispatch_action(&mut self, action: &Action) {
        match action {
            Action::Close => self.close(),
            Action::Exec(name) => {
                let cmd = self.config.default_apps.get(name.as_str()).cloned().unwrap_or_else(|| name.clone());
                self.launch_app(&cmd);
            }
            Action::Quit => {
                remove_file("/tmp/AeroWM.sock").expect("failed to remove socket file while exiting");
                self.pending_screenshot = true;
            },
            Action::SwitchView => self.switch_view(),
            Action::Fullscreen => self.fullscreen(),
            Action::Parent => self.focus_parent(),
            Action::Child => self.focus_child(),
            Action::Sibling => self.focus_sibling(),
            Action::FocusZoom => if self.current_view_mode() == ViewMode::TreeView { self.focus_zoom(); } else {},
            Action::ResetView => self.reset_view(),
            Action::Pan(x, y) => self.pan(*x, *y),
            Action::Resize(x, y) => self.resize_focused(*x, *y),
            Action::SwitchVT(n) => self.session.as_mut().unwrap().change_vt(*n as i32).unwrap(),
            Action::MarkArea => {
                let next = self.areas.keys().max().map(|n| n + 1).unwrap_or(1);
                self.marking_area = Some(next);
                self.marking_area_start = None;
                eprintln!("marking_area {} - drag to define", next);
            },
            Action::RemoveArea => self.remove_current_area(),
            Action::ShowAreas => {},
            Action::GoToArea(n) => self.goto_area(*n),
            Action::MoveToNextOutput => self.move_to_next_output(),
            Action::SendToScratchpad => self.send_to_scratchpad(),
            Action::ToggleScratchpad => self.toggle_scratchpad(),
            Action::SwitchLayout => self.switch_layout(),
        }
    }

    fn reset_view(&mut self) {
        if self.current_view_mode() == ViewMode::Tiling {
            self.viewport_target_x = 0.0;
            self.viewport_target_y = 0.0;
            self.viewport_x = 0.0;
            self.viewport_y = 0.0;
            self.zoom = 1.0;
            self.zoom_target = 1.0;
            self.viewport_anim_start_x = 0.0;
            self.viewport_anim_start_y = 0.0;
            if let Some(output) = self.output_under_cursor().cloned() {
                if let Some(vs) = self.per_output_state.get_mut(&output) {
                    vs.viewport_x = self.viewport_x;
                    vs.viewport_y = self.viewport_y;
                    vs.zoom = self.zoom;
                }
            }
            self.apply_layout();
        } else {
            self.snap_to_roots();
        }
    }

    fn close(&self) {
        if self.focused_window_id.is_none() { return; }
        self.windows
            .iter()
            .find(|cw| cw.id == self.focused_window_id.unwrap())
            .and_then(|cw| cw.window.toplevel()
            .map(|t| t.send_close()));
    }

    fn remove_current_area(&mut self) {
        if let Some(n) = self.current_area.take() {
            self.areas.remove(&n);
        }
    }

    fn resize_focused(&mut self, x: i32, y: i32) {
        if let Some(fid) = self.focused_window_id {
            if let Some(cw) = self.windows.iter_mut().find(|cw| cw.id == fid) {
                cw.base_height = y;
                cw.base_width = x;
                self.apply_layout();
            }
        }
    }

    fn goto_area(&mut self, n: u32) {
        if self.areas.contains_key(&n) == false { return; }
        let area = self.areas[&n];
        let (_aw, ah) = (area.size.w, area.size.h);
        let (sw, sh) = {
            let o = self.space.outputs().next().unwrap();
            let g = self.space.output_geometry(o).unwrap(); 
            (g.size.w, g.size.h)
        };
        let zoom_target = sh as f64 / ah;
        let cx = area.loc.x + area.size.w / 2.0;
        let cy = area.loc.y + area.size.h / 2.0;
        self.zoom_target = zoom_target;
        self.zoom_anim_start = self.zoom;
        self.viewport_target_x = cx - (sw / 2) as f64 / zoom_target;
        self.viewport_target_y = cy - (sh / 2) as f64 / zoom_target;
        self.begin_animation();
        self.current_area = Some(n);
    }

    fn switch_layout(&mut self) {
        let keyboard = self.seat.get_keyboard().expect("Keyboard not found while trying to add it");
        keyboard.with_xkb_state(self, |mut state| state.cycle_next_layout());
    }

    fn send_to_scratchpad(&mut self) {
            let Some(focused_window_id) = self.focused_window_id else { return; };
        let parent_id;
        let children;
        {
            let window = self.windows.iter().find(|cw| cw.id == focused_window_id).unwrap();
            
            if window.is_scratchpad { return; }
            parent_id = window.parent_id.clone();
            children = window.children.clone();
        }

        // detach children from focused window and attach to their grandparents
        let children_windows: Vec<&mut CanvasWindow> = self.windows.iter_mut().filter(|cw| children.contains(&cw.id)).collect();
        for window in children_windows {
            window.parent_id = parent_id;
        }
        {
            if let Some(parent_id) = parent_id {
                if let Some(parent) = self.windows.iter_mut().find(|cw| cw.id == parent_id) {
                    let insert_pos = parent.children.iter().position(|&id| id == focused_window_id);
                    parent.children.retain(|w| w != &focused_window_id);
                    if let Some(pos) = insert_pos {
                        for (i, &child_id) in children.iter().enumerate() {
                            parent.children.insert(pos + i, child_id);
                        }
                    } else {
                        parent.children.extend_from_slice(&children);
                    }
                }
            }
        }
        let window = self.windows.iter_mut().find(|cw| cw.id == focused_window_id).unwrap();

        window.is_scratchpad = true;
        window.scratchpad_visible = false;
        window.children = Vec::new();
        window.parent_id = None;

        let local = window.window.clone();
        self.space.unmap_elem(&local);

        if let Some(parent_id) = parent_id { 
            self.focus_by_id(parent_id);
        } else { 
            let non_scratchpad = self.windows.iter().find(|cw| !cw.is_scratchpad);
            if let Some(window) = non_scratchpad { 
                self.focus_by_id(window.id);
            } else {
                self.focus_clear();
            }
        }; 

        self.apply_layout();
    }

    fn toggle_scratchpad(&mut self) {
        if let Some(focused_window_id) = self.focused_window_id {
            let is_visible_scratchpad;
            let win_clone;
            {
                let window = self.windows.iter_mut().find(|cw| cw.id == focused_window_id).unwrap();
                is_visible_scratchpad = window.is_scratchpad && window.scratchpad_visible;
                win_clone = window.window.clone();
                if is_visible_scratchpad { window.scratchpad_visible = false; }
            }
            if is_visible_scratchpad {
                self.space.unmap_elem(&win_clone);
                let non_scratchpad = self.windows.iter().find(|cw| !cw.is_scratchpad).map(|cw| cw.id);
                match non_scratchpad {
                    Some(id) => self.focus_by_id(id),
                    None => self.focus_clear(),
                }
                return;
            }
        }

        let (screen_width, screen_height) = self.output_size();
        let (viewport_x, viewport_y, zoom) = self.current_viewport();
        let Some(window) = self.windows.iter_mut().find(|cw| cw.is_scratchpad && !cw.scratchpad_visible) else { return; };
        let id = window.id;

        let cx = viewport_x + screen_width / (2.0 * zoom) - window.base_width as f64 / 2.0;
        let cy = viewport_y + screen_height / (2.0 * zoom) - window.base_height as f64 / 2.0;
        window.scratchpad_visible = true;
        window.canvas_x = cx;
        window.canvas_y = cy;
        window.target_x = cx;
        window.target_y = cy;
        window.anim_start_x = cx;
        window.anim_start_y = cy;
        let win = window.window.clone();
        self.space.map_element(win, Point::new(cx as i32, cy as i32), true);
        self.focus_by_id(id);
    }

    fn fullscreen(&mut self) {
        self.layout_fullscreen();
        let mode_str = match self.current_view_mode() {
            ViewMode::Tiling => "tiling".to_string(),
            ViewMode::TreeView => "tree".to_string(),
            ViewMode::Fullscreen => "fullscreen".to_string(),
        };
        self.emit_event(crate::ipc::IpcEvent::ModeChanged { mode: mode_str });
    }

    fn switch_view(&mut self) {
        let current_view_mode = self.current_view_mode();
        if let Some(output) = self.output_under_cursor().cloned() {
            if let Some(vs) = self.per_output_state.get_mut(&output) {
                vs.view_mode = match current_view_mode {
                    ViewMode::Tiling => {
                        self.zoom = 1.0;
                        self.zoom_target = 1.0;
                        self.zoom_anim_start = 1.0;
                        ViewMode::TreeView
                    },
                    ViewMode::TreeView => {
                        for cw in &mut self.windows {
                            cw.tree_x = Some(cw.canvas_x);
                            cw.tree_y = Some(cw.canvas_y);
                        }
                        self.zoom = 1.0;
                        self.zoom_target = 1.0;
                        self.zoom_anim_start = 1.0;
                        ViewMode::Tiling
                    },
                    ViewMode::Fullscreen => { self.pre_fullscreen_viewport.as_ref().unwrap().view_mode },
                }
            }
        }
        self.apply_layout();
        let mode_str = match self.current_view_mode() {
            ViewMode::Tiling => "tiling".to_string(),
            ViewMode::TreeView => "tree".to_string(),
            ViewMode::Fullscreen => "fullscreen".to_string(),
        };
        self.emit_event(crate::ipc::IpcEvent::ModeChanged { mode: mode_str });
    }

    fn focus_parent(&mut self) {  
        let pending_tree_focus = self
            .focused_window_id
            .and_then(|fid| {
                self.windows.iter().find(|cw| cw.id == fid)
            })
            .and_then(|cw| cw.parent_id);
        if let Some(target_id) = pending_tree_focus {
            self.focus_by_id(target_id);
            self.tiling_root_id = Some(target_id);
            match self.current_view_mode() {
                ViewMode::Tiling => self.apply_layout(),
                ViewMode::TreeView => self.center_viewport_on_focused(),
                ViewMode::Fullscreen => {},
            }
        }
    }
    
    fn focus_child(&mut self) {  
        let pending_tree_focus = self
            .focused_window_id
            .and_then(|fid| {
                self.windows.iter().find(|cw| cw.id == fid)
            })
            .and_then(|cw| cw.children.first().copied());
        if let Some(target_id) = pending_tree_focus {
            self.focus_by_id(target_id);
            self.tiling_root_id = Some(target_id);
            match self.current_view_mode() {
                ViewMode::Tiling => self.apply_layout(),
                ViewMode::TreeView => self.center_viewport_on_focused(),
                ViewMode::Fullscreen => {},
            }
        }
    }
    
    fn focus_sibling(&mut self) {  
        let mut pending_tree_focus: Option<u32> = None;
        if let Some(fid) = self.focused_window_id {
            let siblings = self.siblings_of(fid);
            if let Some(pos) =
                siblings.iter().position(|&id| id == fid)
            {
                let next = siblings[(pos + 1) % siblings.len()];
                if next != fid {
                    pending_tree_focus = Some(next);
                }
            }
        }
        if let Some(target_id) = pending_tree_focus {
            self.focus_by_id(target_id);
            self.tiling_root_id = Some(target_id);
            match self.current_view_mode() {
                ViewMode::Tiling => self.apply_layout(),
                ViewMode::TreeView => self.center_viewport_on_focused(),
                ViewMode::Fullscreen => {},
            }
        }
    }

    pub fn mods_match(&self, required: &[ModifierKey], held: &ModifiersState) -> bool {
        required.iter().all(|m| match m {
            ModifierKey::Alt => held.alt,
            ModifierKey::Ctrl => held.ctrl,
            ModifierKey::Shift => held.shift,
            ModifierKey::Super => held.logo,
        })
    }
}