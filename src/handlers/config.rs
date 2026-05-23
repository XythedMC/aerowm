
use mlua::{
    Error, Lua, Table
};
use anyhow::anyhow;
use std::{cell::RefCell, collections::HashMap, fs::{create_dir_all, read_to_string, write}, rc::Rc};
use dirs::config_dir;

use crate::{keybind::{Action, ParsedKeybind, parse_action, parse_keybind}};

#[derive(Debug, Clone)]
pub struct AeroWMConfig {
    pub main_modifier: String,
    pub gap: f64,
    pub focused_border_color: [u8; 3],
    pub unfocused_border_color: [u8; 3],
    pub background_type: String,
    pub background_color: [u8; 3],
    pub background_image: Option<String>,
    pub background_shader: Option<String>,
    pub corner_rounding: f32,
    pub tile_distance: i32,
    pub border_width: f32,
    pub hover_to_focus: bool,
    pub client_side_decorations: bool,
    pub cursor_size: [i32; 2],
    pub launch_rules: HashMap<String, LaunchRule>,
    pub default_apps: HashMap<String, String>,
    pub keybinds: Vec<(ParsedKeybind, Action)>,
}

#[derive(Debug, Clone)]
pub struct LaunchRule {
    pub args: Option<String>,
    pub env: Option<HashMap<String, String>>
}


pub fn read_config() -> Result<AeroWMConfig, Error> {
    let config_path = config_dir()
        .ok_or_else(|| Error::runtime("Config directory ($HOME/.config) doesn't exist"))?
        .join("aerowm")
        .join("aerowm.lua");
    eprintln!("config path: {:?}", config_path);
    let contents = read_to_string(config_path)?;

    let lua = Lua::new();

    let keybinds: Vec<(ParsedKeybind, Action)> = Vec::new();
    let keybinds = Rc::new(RefCell::new(keybinds));
    let keybinds_clone = keybinds.clone();

    lua.globals().set("bind", lua.create_function(move |_, (keybind_str, action_str, arg): (String, String, Option<String>)| {
        let keybind = parse_keybind(&keybind_str).map_err(|e| Error::runtime(e.to_string()))?;
        let action = parse_action(&action_str, arg).map_err(|e| Error::runtime(e.to_string()))?;
        keybinds_clone.borrow_mut().push((keybind, action));
        Ok(())
    })?)?;
    
    lua.load(&contents).exec().map_err(|e| Error::runtime(e.to_string()))?;
    let keybinds = keybinds.borrow().clone();

    let table = lua.globals().get::<Table>("config").map_err(|e| Error::runtime(e.to_string()))?;
    
    let main_modifier = table.get::<String>("main_modifier").map_err(|e| Error::runtime(e.to_string()))?;
    let gap = table.get::<f64>("gap").map_err(|e| Error::runtime(e.to_string()))?;
    
    let focused_arr: Table = table.get("focused_border_color").map_err(|e| Error::runtime(e.to_string()))?;
    let focused_border_color = [
        focused_arr.get::<u8>(1).map_err(|e| Error::runtime(e.to_string()))?,
        focused_arr.get::<u8>(2).map_err(|e| Error::runtime(e.to_string()))?,
        focused_arr.get::<u8>(3).map_err(|e| Error::runtime(e.to_string()))?,
    ];

    let unfocused_arr: Table = table.get("unfocused_border_color").map_err(|e| Error::runtime(e.to_string()))?;
    let unfocused_border_color = [
        unfocused_arr.get::<u8>(1).map_err(|e| Error::runtime(e.to_string()))?,
        unfocused_arr.get::<u8>(2).map_err(|e| Error::runtime(e.to_string()))?,
        unfocused_arr.get::<u8>(3).map_err(|e| Error::runtime(e.to_string()))?,
    ];
    let background_type = table.get::<String>("background_type").map_err(|e| Error::runtime(e.to_string()))?;

    let background_color_arr: Table = table.get("background_color").map_err(|e| Error::runtime(e.to_string()))?;
    let background_color = [
        background_color_arr.get::<u8>(1).map_err(|e| Error::runtime(e.to_string()))?,
        background_color_arr.get::<u8>(2).map_err(|e| Error::runtime(e.to_string()))?,
        background_color_arr.get::<u8>(3).map_err(|e| Error::runtime(e.to_string()))?,
    ];
    let background_image = table.get::<Option<String>>("background_image").map_err(|e| Error::runtime(e.to_string()))?;
    let background_shader = table.get::<Option<String>>("background_shader").map_err(|e| Error::runtime(e.to_string()))?;
    let corner_rounding = table.get::<f32>("corner_rounding").map_err(|e| Error::runtime(e.to_string()))?;
    let tile_distance = table.get::<i32>("tile_distance").map_err(|e| Error::runtime(e.to_string()))?;
    let border_width = table.get::<f32>("border_width").map_err(|e| Error::runtime(e.to_string()))?;
    let hover_to_focus = table.get::<bool>("hover_to_focus").map_err(|e| Error::runtime(e.to_string()))?;
    let client_side_decorations = table.get::<bool>("client_side_decorations").map_err(|e| Error::runtime(e.to_string()))?;
    let cursor_size_arr: Table = table.get("cursor_size").map_err(|e| Error::runtime(e.to_string()))?;
    let cursor_size = [
        cursor_size_arr.get::<i32>(1).map_err(|e| Error::runtime(e.to_string()))?,
        cursor_size_arr.get::<i32>(2).map_err(|e| Error::runtime(e.to_string()))?,
    ];

    let rules_table: Table = table.get("launch_rules").map_err(|e| Error::runtime(e.to_string()))?;
    let mut launch_rules = HashMap::new();
    for pair in rules_table.pairs::<String, Table>() {
        let (app_name, rule_table) = pair.map_err(|e| Error::runtime(e.to_string()))?;
        let args: Option<String> = rule_table.get("args").map_err(|e| Error::runtime(e.to_string()))?;

        let env_table: Option<Table> = rule_table.get("env").map_err(|e| Error::runtime(e.to_string()))?;
        let env: Option<HashMap<String, String>> = env_table.map(|et| {
            et.pairs::<String, String>()
                .map(|p| p.map_err(|e| Error::runtime(e.to_string())))
                .collect::<Result<HashMap<_,_>, _>>()
        }).transpose()?;

        launch_rules.insert(app_name, LaunchRule { args, env });
    }

    let default_apps_table: Table = table.get("apps").map_err(|e| Error::runtime(e.to_string()))?;
    let mut default_apps = HashMap::new();
    for pair in default_apps_table.pairs::<String, String>() {
        let (app_type, app_name) = pair.map_err(|e| Error::runtime(e.to_string()))?;
        default_apps.insert(app_type, app_name);
    }


    Ok(AeroWMConfig {
        main_modifier,
        gap,
        focused_border_color,
        unfocused_border_color,
        background_type,
        background_color,
        background_image,
        background_shader,
        corner_rounding,
        tile_distance,
        border_width,
        hover_to_focus,
        client_side_decorations,
        cursor_size,
        launch_rules,
        default_apps,
        keybinds,
    })
}

pub fn create_config() -> anyhow::Result<()>  {
    let result_path = config_dir()
        .ok_or_else(|| anyhow!("config path couldn't be found"))?
        .join("aerowm")
        .join("aerowm.lua");
    eprintln!("config path: {:?}", result_path);
    let default_config = r#"config = {
    main_modifier = "Ctrl",
    gap = 80.0,
    focused_border_color = {64, 144, 194},
    unfocused_border_color = {0, 0, 0},
    background_type = "color",
    background_color = {26, 26, 26},
    background_image = "",
    background_shader = "",
    corner_rounding = 32.0,
    tile_distance = 8,
    border_width = 2.0,
    hover_to_focus = true,
    client_side_decorations = false,
    cursor_size = {32, 32},

    apps = {
        terminal = "kitty",
        browser  = "zen-browser",
        files = "nautilus",
    },

    launch_rules = {
        ["zen-browser"]= { args = "--no-remote", env = { ELECTRON_OZONE_PLATFORM_HINT = "auto", MOZ_ENABLE_WAYLAND="1" } },
        ["discord"] = { env = { ELECTRON_OZONE_PLATFORM_HINT = "wayland" } },
    },
}

-- Window management
bind("Ctrl+Q",           "close")
bind("Ctrl+F",           "fullscreen")
bind("Ctrl+Space",       "switch_view")

-- Apps
bind("Ctrl+Return",      "exec", "kitty")
bind("Ctrl+W",           "exec", "zen-browser")
bind("Ctrl+E",           "exec", "nautilus")

-- Tree navigation
bind("Ctrl+P",           "parent")
bind("Ctrl+N",           "sibling")
bind("Ctrl+C",           "child")

-- Tree view
bind("Ctrl+Z",           "focus_zoom")
bind("Ctrl+Home",        "reset_view")

-- Viewport panning
bind("Ctrl+Left",        "pan", "-100 0")
bind("Ctrl+Right",       "pan", "100 0")
bind("Ctrl+Up",          "pan", "0 -100")
bind("Ctrl+Down",        "pan", "0 100")

-- Quit
bind("Ctrl+Alt+BackSpace", "quit")
"#;

    create_dir_all(result_path.parent().ok_or_else(|| anyhow!("Parent path couldn't be found"))?)?;
    write(result_path, default_config)?;
    Ok(())
}
