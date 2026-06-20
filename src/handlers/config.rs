
use mlua::{
    Error, Lua, Table
};
use anyhow::anyhow;
use std::{cell::RefCell, collections::HashMap, fs::{create_dir_all, read_to_string, write}, rc::Rc};
use dirs::{config_dir, home_dir};
use hex_color::HexColor;

use crate::{keybind::{Action, ParsedKeybind, parse_action, parse_keybind}};

#[derive(Debug, Clone)]
pub struct AeroWMConfig {
    pub main_modifier: String,
    pub gap: f64,

    pub focused_border_color: [u8; 4],
    pub unfocused_border_color: [u8; 4],

    pub background_type: String,
    pub background_color: [u8; 4],
    pub background_image: Option<String>,
    pub background_shader: Option<String>,

    pub corner_rounding: f32,
    pub tile_distance: i32,
    pub border_width: f32,
    pub hover_to_focus: bool,
    pub center_on_launch: bool,

    pub client_side_decorations: bool,

    pub animation_ease: f64,

    pub cursor_size: [i32; 2],

    pub launch_rules: HashMap<String, LaunchRule>,
    pub default_apps: HashMap<String, String>,
    pub keybinds: Vec<(ParsedKeybind, Action)>,

    pub area_border_thickness: i32,
    pub area_colors: Vec<[u8; 4]>,
    pub always_show_areas: bool,

    pub monitors: Vec<MonitorConfig>
}

#[derive(Debug, Clone)]
pub struct LaunchRule {
    pub args: Option<String>,
    pub env: Option<HashMap<String, String>>
}

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub refresh_rate: f64,
    pub scale: f64,
}

pub fn get_colors_rgba(key: &str) -> [u8; 4] {
    let color = HexColor::parse(key).expect(format!("Failed to convert color {} to rgba", key).as_str());
    [color.r, color.g, color.b, color.a]
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

    let monitors: Vec<MonitorConfig> = Vec::new();
    let monitors = Rc::new(RefCell::new(monitors));
    let monitors_clone = monitors.clone();

    lua.globals().set("monitor", lua.create_function(move |_, (name, position, refresh_rate, scale): (String, String, f64, f64)| {
        let parts: Vec<&str> = position.split("x").collect();
        let x = parts[0].parse::<i32>().unwrap_or(0);
        let y = parts[1].parse::<i32>().unwrap_or(0);
        monitors_clone.borrow_mut().push(MonitorConfig { name, x, y, refresh_rate, scale });
        Ok(())
    })?)?;
    
    lua.load(&contents).exec().map_err(|e| Error::runtime(e.to_string()))?;
    let keybinds = keybinds.borrow().clone();
    let monitors = monitors.borrow().clone();

    let table = lua.globals().get::<Table>("config").map_err(|e| Error::runtime(e.to_string()))?;
    
    let main_modifier = table.get::<String>("main_modifier").map_err(|e| Error::runtime(e.to_string()))?;
    let gap = table.get::<f64>("gap").map_err(|e| Error::runtime(e.to_string()))?;
    
    let focused_border_color = get_colors_rgba(table.get::<String>("focused_border_color").map_err(|e| Error::runtime(e.to_string()))?.as_str());

    let unfocused_border_color = get_colors_rgba(table.get::<String>("unfocused_border_color").map_err(|e| Error::runtime(e.to_string()))?.as_str());
    let background_type = table.get::<String>("background_type").map_err(|e| Error::runtime(e.to_string()))?;

    let background_color= get_colors_rgba(table.get::<String>("background_color").map_err(|e| Error::runtime(e.to_string()))?.as_str());
    let background_image = table.get::<Option<String>>("background_image").map_err(|e| Error::runtime(e.to_string()))?;
    let background_shader = table.get::<Option<String>>("background_shader").map_err(|e| Error::runtime(e.to_string()))?;

    let corner_rounding = table.get::<f32>("corner_rounding").map_err(|e| Error::runtime(e.to_string()))?;
    let tile_distance = table.get::<i32>("tile_distance").map_err(|e| Error::runtime(e.to_string()))?;
    let border_width = table.get::<f32>("border_width").map_err(|e| Error::runtime(e.to_string()))?;
    let hover_to_focus = table.get::<bool>("hover_to_focus").map_err(|e| Error::runtime(e.to_string()))?;
    let center_on_launch = table.get::<bool>("center_on_launch").map_err(|e| Error::runtime(e.to_string()))?;

    let client_side_decorations = table.get::<bool>("client_side_decorations").map_err(|e| Error::runtime(e.to_string()))?;
    
    let animation_ease = table.get::<f64>("animation_ease").map_err(|e| Error::runtime(e.to_string()))?;

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

    let area_colors_table: Table = table.get("area_colors").map_err(|e| Error::runtime(e.to_string()))?;
    let mut area_colors = Vec::new();
    for i in 1..=area_colors_table.len().map_err(|_| Error::runtime("Empty area_colors, fill in at least one color value"))? {
        area_colors.push(get_colors_rgba(area_colors_table.get::<String>(i).map_err(|e| Error::runtime(e.to_string()))?.as_str()));
    }
    let area_border_thickness = table.get::<i32>("area_border_thickness").map_err(|e| Error::runtime(e.to_string()))?;
    let always_show_areas = table.get::<bool>("always_show_areas").map_err(|e| Error::runtime(e.to_string()))?;

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
        center_on_launch,
        client_side_decorations,
        animation_ease,
        cursor_size,
        launch_rules,
        default_apps,
        keybinds,
        area_colors,
        area_border_thickness,
        always_show_areas,
        monitors,
    })
}

pub fn create_config() -> anyhow::Result<()>  {
    let result_path = config_dir()
        .ok_or_else(|| anyhow!("config path couldn't be found"))?
        .join("aerowm")
        .join("aerowm.lua");
    eprintln!("config path: {:?}", result_path);
    let default_config = r#"config = {
    main_modifier = "Super",
    gap = 80.0,
    focused_border_color = '#4090c2',
    unfocused_border_color = '#000000',
    background_type = "shader",
    background_color = '#1a1a1a',
    background_image = "$HOME/current_wallpaper.png",
    background_shader = "$HOME/.config/aerowm/shaders/sunset.frag",
    corner_rounding = 32.0,
    tile_distance = 8,
    border_width = 2.0,
    launch_at_center = true,
    animation_ease = 0.3,
    hover_to_focus = true,
    center_on_launch = true,
    client_side_decorations = false,
    cursor_size = {32, 32},

    apps = {
        terminal = "kitty",
        browser  = "zen-browser",
        files = "nautilus",
        launcher = "rofi -show drun"
    },

    launch_rules = {
        ["zen-browser"]= { args = "--no-remote", env = { ELECTRON_OZONE_PLATFORM_HINT = "auto", MOZ_ENABLE_WAYLAND="1" } },
        ["discord"] = { env = { ELECTRON_OZONE_PLATFORM_HINT = "wayland" } },
    },

    area_colors = {
        '#5e81ac',
        '#88c0d0',
        '#8fbcbb',
        '#a3be8c',
        '#ebcb8b',
        '#d08770',
        '#b48ead',
        '#81a1c1',
        '#4c566a',
        '#76c0a0',
    },
    area_border_thickness = 3,
    always_show_areas = true,
}

local mainMod = "Super"

-- Window management
bind(mainMod .. "+Q",           "close")
bind(mainMod .. "+F",           "fullscreen")
bind(mainMod .. "+Space",       "switch_view")
bind(mainMod .. "+G",           "move_to_next_output")

-- Areas
bind(mainMod .. "+M", "mark_area")
bind(mainMod .. "+A", "show_areas")
bind(mainMod .. "+R", "remove_area")
bind(mainMod .. "+1", "goto_area", "1")
bind(mainMod .. "+2", "goto_area", "2")
bind(mainMod .. "+3", "goto_area", "3")
bind(mainMod .. "+4", "goto_area", "4")
bind(mainMod .. "+5", "goto_area", "5")
bind(mainMod .. "+6", "goto_area", "6")
bind(mainMod .. "+7", "goto_area", "7")
bind(mainMod .. "+8", "goto_area", "8")
bind(mainMod .. "+9", "goto_area", "9")

-- Apps
bind(mainMod .. "+Return",      "exec", "kitty")
bind(mainMod .. "+W",           "exec", "zen-browser")
bind(mainMod .. "+E",           "exec", "nautilus")
bind(mainMod .. "+Tab", "exec", "launcher")

-- Tree navigation
bind(mainMod .. "+P",           "parent")
bind(mainMod .. "+N",           "sibling")
bind(mainMod .. "+C",           "child")

-- Tree view
bind(mainMod .. "+Z",           "focus_zoom")
bind(mainMod .. "+Home",        "reset_view")

-- Viewport panning
bind(mainMod .. "+Left",        "pan", "-100 0")
bind(mainMod .. "+Right",       "pan", "100 0")
bind(mainMod .. "+Up",          "pan", "0 -100")
bind(mainMod .. "+Down",        "pan", "0 100")

-- Scratchpad
bind(mainMod .. "+S",             "send_to_scratchpad")
bind(mainMod .. "+Shift+S",       "toggle_scratchpad")

-- Quit
bind(mainMod .. "+Alt+BackSpace", "quit")

-- Monitors
monitor("HDMI-A-1", "2560x0", 74.78, 1.0)
monitor("DP-1", "0x0", 165.08, 1.0)
"#.replace("$HOME", home_dir().expect("home dir not found, how are you reading this?").to_str().unwrap());

    create_dir_all(result_path.parent().ok_or_else(|| anyhow!("Parent path couldn't be found"))?)?;
    write(result_path, default_config)?;
    Ok(())
}
