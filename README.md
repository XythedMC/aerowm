# AeroWM

A Wayland compositor built in Rust on top of [Smithay](https://github.com/Smithay/smithay).

The idea is simple: your windows live on an infinite canvas in a tree. Day-to-day you work in normal tiling mode. When things get complex, you zoom out and see everything at once - the whole tree, every window, laid out in front of you. Pinch to zoom, drag to pan, then dive back in.

## What it does

- **BSP tiling** - windows split the space automatically as you open them
- **Tree view** - zoom out to the infinite canvas and see your whole window tree
- **Rounded corners** - window content clipped to a rounded rectangle, not just the border
- **Scratchpad** - a floating window outside the tree you can summon and dismiss anytime
- **Areas** - mark regions of the canvas and jump between them instantly
- **Backgrounds** - solid colour, wallpaper image, or a custom GLSL shader
- **XWayland** - X11 apps work fine
- **Lua config** - one file, hot-reloaded, no restart needed
- **Trackpad gestures** - pinch to zoom, swipe to pan
- **Everything else** - layer shell, screencopy, clipboard, IME, idle protocols, multi-monitor

## Requirements

- A Linux system with DRM/KMS support
- `libseat` for session management
- `libinput` for input handling
- A GPU with EGL/GBM support

## Building

```sh
cargo build --release
```

## Running

```sh
cargo run --release
```

Or after building:

```sh
./target/release/aerowm
```

To launch with an app:

```sh
./target/release/aerowm -e kitty
```

## Configuration

The config file lives at `~/.config/aerowm/aerowm.lua`. If it doesn't exist, AeroWM will create a default one on first run.

### Example config

```lua
config = {
    main_modifier = "Super",
    gap = 80.0,
    focused_border_color = '#4090c2',
    unfocused_border_color = '#000000',
    background_type = "color",
    background_color = '#1a1a1a',
    corner_rounding = 32.0,
    tile_distance = 8,
    border_width = 2.0,
    animation_ease = 0.3,
    hover_to_focus = true,
    client_side_decorations = false,
    cursor_size = {32, 32},

    area_colors = {
        '#5e81ac', '#88c0d0', '#8fbcbb', '#a3be8c', '#ebcb8b',
        '#d08770', '#b48ead', '#81a1c1', '#4c566a', '#76c0a0',
    },
    area_border_thickness = 3,
    always_show_areas = false,

    apps = {
        terminal = "kitty",
        browser  = "firefox",
    },

    launch_rules = {},
}

input = {
    layouts = {"us"},
    repeat_rate = 25,
    repeat_delay = 600,
    pointer_speed = 1.0,
}

local mainMod = "Super"

-- Keybinds
bind(mainMod .. "+Return", "exec", "terminal")
bind(mainMod .. "+W",      "exec", "browser")
bind(mainMod .. "+Q",      "close")
bind(mainMod .. "+F",      "fullscreen")
bind(mainMod .. "+Space",  "switch_view")

bind(mainMod .. "+P",      "parent")
bind(mainMod .. "+N",      "sibling")
bind(mainMod .. "+C",      "child")
bind(mainMod .. "+Z",      "focus_zoom")
bind(mainMod .. "+Home",   "reset_view")

bind(mainMod .. "+Left",   "pan", "-100 0")
bind(mainMod .. "+Right",  "pan", "100 0")
bind(mainMod .. "+Up",     "pan", "0 -100")
bind(mainMod .. "+Down",   "pan", "0 100")

bind(mainMod .. "+S",         "send_to_scratchpad")
bind(mainMod .. "+Shift+S",   "toggle_scratchpad")

bind("Alt+Shift",              "switch_layout")

bind("Super+Alt+BackSpace", "quit")
```

### Monitor configuration

```lua
monitor("DP-1", "0x0", 165.0, 1.0)
monitor("HDMI-A-1", "2560x0", 75.0, 1.0)
```

Arguments: output name, position as `"WxH"`, refresh rate in Hz, scale factor.

### Input configuration

```lua
input = {
    layouts = {"us", "il"},   -- XKB layout names, cycled with switch_layout
    repeat_rate = 25,         -- key repeats per second
    repeat_delay = 600,       -- ms before repeat starts
    pointer_speed = 1.0,      -- flat speed multiplier

    -- optional: custom acceleration function, receives movement speed, returns multiplier
    pointer_acceleration = function(speed)
        return 1.0 + speed * 0.1
    end,
}
```

### Keybind syntax

```lua
bind("Mod+Key", "action")
bind("Mod+Key", "action", "argument")
```

Modifiers: `Ctrl`, `Alt`, `Shift`, `Super`. Mouse buttons: `left`, `right`, `middle`. Modifier-only binds (e.g. `"Alt+Shift"`) are also supported.

### Actions

| Action | Argument | Description |
|--------|----------|-------------|
| `exec` | command or app alias | Launch an application |
| `close` | | Close the focused window |
| `quit` | | Exit AeroWM |
| `fullscreen` | | Toggle fullscreen for the focused window |
| `switch_view` | | Toggle between tiling and tree view |
| `parent` | | Focus the parent window in the tree |
| `child` | | Focus the first child window in the tree |
| `sibling` | | Focus the next sibling window in the tree |
| `focus_zoom` | | Zoom the viewport to the focused window (tree view) |
| `reset_view` | | Reset viewport and zoom |
| `pan` | `"dx dy"` | Pan the canvas by dx, dy pixels |
| `switch_vt` | vt number | Switch to a virtual terminal |
| `mark_area` | | Start marking a new area by dragging on the canvas |
| `goto_area` | area number | Animate the viewport to fill the given area |
| `remove_area` | | Remove the currently active area |
| `show_areas` | | Hold to show area outlines on the canvas |
| `send_to_scratchpad` | | Move the focused window to the scratchpad |
| `toggle_scratchpad` | | Show or hide the scratchpad window |
| `switch_layout` | | Cycle to the next XKB keyboard layout |

## 🗺️ Roadmap

- ✅ XWayland support
- ✅ Multi-output / multi-monitor support
- ✅ Background images and shaders
- ✅ Trackpad gestures
- ✅ Scratchpad
- ✅ IPC - full external control over Unix socket
- ✅ Screencopy - screenshot and screen recording support
- ✅ Clipboard manager - wlr-data-control protocol
- ✅ Idle notify and idle inhibit
- ✅ Config hot-reload
- ✅ IME / multi-language input
- ✅ Layer shell - bars, docks, notifications
- ⬜ Session lock - ext-session-lock protocol
- ⬜ Animations - window open/close transitions
- ⬜ Window rules - auto-assign apps to positions or parents on launch

## License

MIT
