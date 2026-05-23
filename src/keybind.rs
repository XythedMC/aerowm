use std::fs::{remove_file};

use mlua::{ Error };
use smithay::{backend::session::Session, input::keyboard::{Keysym, ModifiersState, xkb::{KEYSYM_CASE_INSENSITIVE, keysym_from_name}}};

use crate::{AeroWM, state::{ModifierKey, ViewMode}};

#[derive(Debug, Clone)]
pub struct ParsedKeybind {
    pub mods: Vec<ModifierKey>,
    pub trigger: Trigger,
}

#[derive(Debug, Clone)]
pub enum Trigger {
    Key(Keysym),
    Button(MouseButtons)
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
}
pub fn parse_keybind(s: &str) -> Result<ParsedKeybind, Error> {
    let parts: Vec<String> = s.split("+").map(|string| string.trim().to_lowercase()).collect();
    if !parts.contains(&String::from("ctrl")) && !parts.contains(&String::from("alt")) && !parts.contains(&String::from("shift")) && !parts.contains(&String::from("super")) {
        return Err(Error::runtime(format!("modifiers for keybind {} dont exist", s)))
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
                    return Err(Error::runtime("more than one key in keybind"))
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
    let trigger = trigger.ok_or_else(|| Error::runtime("no key or button in keybind"))?;
    Ok(ParsedKeybind { mods, trigger })
}

pub fn parse_action(action: &str, args: Option<String>) -> Result<Action, Error> {
    match action.to_lowercase().as_str() {
        "close" => Ok(Action::Close),
        "exec" => {
            if args.is_some() { Ok(Action::Exec(args.unwrap()))}
            else { Err(Error::runtime("need arguments for this type of action"))}
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
            let arg = args.ok_or_else(|| Error::runtime("pan requires an argument"))?;
            let mut parts = arg.splitn(2, ' ');
            let x = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| Error::runtime("invalid pan x"))?;
            let y = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| Error::runtime("invalid pan y"))?;
            Ok(Action::Pan(x, y))
        },
        "resize" => {
            let arg = args.ok_or_else(|| Error::runtime("resize requires an argument"))?;
            let mut parts = arg.splitn(2, ' ');
            let x = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| Error::runtime("invalid resize x"))?;
            let y = parts.next().and_then(|s| s.parse().ok()).ok_or_else(|| Error::runtime("invalid resize y"))?;
            Ok(Action::Resize(x, y))
        },
        "switch_vt" => {
            let n = args.ok_or_else(|| Error::runtime("switch_vt needs a number"))?
                .parse::<u8>().map_err(|e| Error::runtime(e.to_string()))?;
            Ok(Action::SwitchVT(n))
        },
        "mark_area" => Ok(Action::MarkArea),
        "goto_area" => {
            let n = args.ok_or_else(|| Error::runtime("goto_area needs a number"))?
                .parse::<u32>().map_err(|_| Error::runtime("invalid area number"))?;
            Ok(Action::GoToArea(n))
        },
        _ => Err(Error::runtime("action type not supported"))
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
            Action::Fullscreen => self.toggle_fullscreen(),
            Action::Parent => self.focus_parent(),
            Action::Child => self.focus_child(),
            Action::Sibling => self.focus_sibling(),
            Action::FocusZoom => if self.view_mode == ViewMode::TreeView { self.focus_zoom(); } else {},
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
            Action::GoToArea(n) => self.goto_area(*n),
        }
    }

    fn reset_view(&mut self) {
        if self.view_mode == ViewMode::Tiling {
            self.viewport_target_x = 0.0;
            self.viewport_target_y = 0.0;
            self.viewport_x = 0.0;
            self.viewport_y = 0.0;
            self.viewport_anim_start_x = 0.0;
            self.viewport_anim_start_y = 0.0;
            self.apply_layout();
        } else {
            self.snap_to_roots();
        }
    }
    fn close(&self) {
        self.windows
            .iter()
            .find(|cw| cw.id == self.focused_window_id.expect("No focused window to close"))
            .and_then(|cw| cw.window.toplevel()
            .map(|t| t.send_close()));
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
        let (aw, ah) = (area.size.w, area.size.h);
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
    }

    fn switch_view(&mut self) {
        self.view_mode = match self.view_mode {
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
            }
        };
        self.apply_layout();
        let mode_str = match self.view_mode {
            ViewMode::Tiling => "tiling".to_string(),
            ViewMode::TreeView => "tree".to_string(),
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
            match self.view_mode {
                ViewMode::Tiling => self.apply_layout(),
                ViewMode::TreeView => self.center_viewport_on_focused(),
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
            match self.view_mode {
                ViewMode::Tiling => self.apply_layout(),
                ViewMode::TreeView => self.center_viewport_on_focused(),
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
            match self.view_mode {
                ViewMode::Tiling => self.apply_layout(),
                ViewMode::TreeView => self.center_viewport_on_focused(),
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