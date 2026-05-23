# AeroWM

A Wayland compositor and window manager written in Rust, built on [Smithay](https://github.com/Smithay/smithay).

AeroWM organises windows in a tree structure. You can work in a focused **tiling** mode or zoom out to a **tree view** that shows the full window hierarchy on an infinite canvas.

## Features

- **BSP tiling layout** - windows are split recursively into the available space
- **Tree view** - zoom out to see and navigate the full window tree
- **Infinite canvas** - pan and zoom freely in tree view
- **Tree navigation** - move focus to parent, child, or sibling windows with keyboard shortcuts
- **Edge resizing** - drag any window edge or corner to resize
- **Drag to move** - hold the main modifier and drag to reposition windows freely
- **Configurable keybinds** - bind keyboard shortcuts and mouse buttons to actions via Lua
- **Lua config** - all settings and keybinds are defined in a single Lua file
- **Borders with corner rounding** - customisable focused/unfocused border colours and rounded corners
- **Hover to focus** - optional focus-follows-mouse
- **Background** - solid colour, image, or custom GLSL shader
- **XWayland support** - run X11 applications alongside native Wayland clients

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
    main_modifier = "Ctrl",
    gap = 80.0,
    focused_border_color = {64, 144, 194},
    unfocused_border_color = {0, 0, 0},
    background_type = "color",
    background_color = {26, 26, 26},
    corner_rounding = 32.0,
    tile_distance = 8,
    border_width = 2.0,
    hover_to_focus = true,
    client_side_decorations = false,
    cursor_size = {32, 32},

    apps = {
        terminal = "kitty",
        browser  = "firefox",
    },

    launch_rules = {},
}

-- Keybinds
bind("Ctrl+Return", "exec", "terminal")
bind("Ctrl+W",      "exec", "browser")
bind("Ctrl+Q",      "close")
bind("Ctrl+F",      "fullscreen")
bind("Ctrl+Space",  "switch_view")

bind("Ctrl+P",      "parent")
bind("Ctrl+N",      "sibling")
bind("Ctrl+C",      "child")
bind("Ctrl+Z",      "focus_zoom")
bind("Ctrl+Home",   "reset_view")

bind("Ctrl+Left",   "pan", "-100 0")
bind("Ctrl+Right",  "pan", "100 0")
bind("Ctrl+Up",     "pan", "0 -100")
bind("Ctrl+Down",   "pan", "0 100")

bind("Ctrl+Alt+BackSpace", "quit")
```

### Keybind syntax

```lua
bind("Mod+Key", "action")
bind("Mod+Key", "action", "argument")
```

Modifiers: `Ctrl`, `Alt`, `Shift`, `Super`. Mouse buttons: `left`, `right`, `middle`.

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

## Planned / In Progress

- ~~**XWayland support** - compatibility with X11 applications~~
- **Multi-output support** - multiple monitors
- **wlr protocols** - layer shell (bars, docks, notifications), screencopy, data control, idle notify, session lock
- ~~**Background images and shaders** - image backgrounds and custom GLSL shaders~~
- **Trackpad support** - gestures for panning, zooming, and switching views
- **Animations** - window open/close transitions
- **Scratchpad** - floating windows outside the tree
- **Window rules** - auto-assign apps to positions or parents on launch
- **IPC** - full external control protocol over the Unix socket
- **Screenshot/screencopy** - wlr-screencopy protocol support
- **Clipboard manager support** - wlr-data-control protocol
- **Idle/lock screen** - ext-idle-notify and ext-session-lock support
- **Named workspaces** - map subtrees to named workspaces with fast switching
- **Config hot-reload** - reload the Lua config without restarting
- **Multi-language input** - IME support for non-Latin scripts

## License

MIT
