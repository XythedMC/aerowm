mod grabs;
mod handlers;
mod input;
mod ipc;
mod state;
mod winit;
mod drm;
mod rendering;
mod keybind;

use dirs::config_dir;
use notify::{RecursiveMode, Watcher};
pub use state::AeroWM;

use std::{env::set_var, path::PathBuf, process::Command, thread};

use smithay::reexports::{
    calloop::{EventLoop, channel::{Event, channel}}, 
    wayland_server::Display
};
use tokio::{runtime::Runtime, sync::broadcast};
use tracing_subscriber::EnvFilter;
use crate::handlers::config::{create_config, read_config};

fn main() -> anyhow::Result<()>{
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .init();

    // Collect args: AeroWM [-e cmd [args...]]
    let args: Vec<String> = std::env::args().collect();
    let startup_cmd: Option<Vec<String>> = {
        let mut iter = args.iter().skip(1);
        if iter.next().map(|s| s == "-e").unwrap_or(false) {
            let rest: Vec<String> = args.iter().skip(2).cloned().collect();
            if rest.is_empty() { None } else { Some(rest) }
        } else {
            None
        }
    };

    let config = match read_config(){
        Ok(aerowm_config) => aerowm_config,
        Err(_) => {
            create_config()?;
            read_config().expect("Failed to read config after initial creation for some weird reason")
        }
    };

    let mut event_loop: EventLoop<'static, AeroWM> = EventLoop::try_new().expect("Failed to create event loop");
    let display: Display<AeroWM> = Display::new().unwrap();

    let mut state = AeroWM::new(&mut event_loop, display, config);

    let (cmd_tx, cmd_rx) = channel::<ipc::InternalCommand>();
    let (config_tx, config_rx) = channel::<()>();
    let (event_tx, event_rx) = broadcast::channel(16);

    state.event_tx = Some(event_tx);

    let socket_path = PathBuf::from("/tmp/AeroWM.sock");
    thread::spawn(move || {
        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            ipc::run_ipc_server(cmd_tx, event_rx, &socket_path).await;
        });
    });

    let mut watcher = notify::recommended_watcher( move |res: notify::Result<notify::Event>| {
        if let Ok(event) = res {
            if event.kind.is_modify() {
                config_tx.send(()).ok();
            }
        }
    }).unwrap();
    let config_path = config_dir()
        .expect("Config directory ($HOME/.config) doesn't exist")
        .join("aerowm")
        .join("aerowm.lua");
    watcher.watch(&config_path.to_path_buf(), RecursiveMode::NonRecursive).unwrap();
    std::mem::forget(watcher);

    event_loop.handle().insert_source(config_rx, |_event, _, state| {
        state.reload_config().expect("Failed to reload config, there is a problem with the config file");
    }).unwrap();

    event_loop.handle().insert_source(cmd_rx, |event, _, state| {
        if let Event::Msg(cmd) = event {
            state.handle_ipc_cmd(cmd);
        }
    }).unwrap();


    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        winit::init_winit(&mut event_loop, &mut state).expect("Failed to initialize winit backend");
    } else {
        drm::init_drm(&mut event_loop, &mut state).expect("Failed to initialize DRM backend");
    }

    let socket_str = state.socket_name.to_string_lossy().into_owned();

    // Propagate into our own environment so child processes inherit the right display.
    set_var("WAYLAND_DISPLAY",            &state.socket_name);
    set_var("MOZ_ENABLE_WAYLAND",         "1");  // Firefox / Zen
    set_var("GDK_BACKEND",                "wayland");
    set_var("QT_QPA_PLATFORM",            "wayland");
    set_var("ELECTRON_OZONE_PLATFORM_HINT", "wayland"); // Electron (Spotify, Claude …)
    set_var("CLUTTER_BACKEND",            "wayland");
    set_var("SDL_VIDEODRIVER",            "wayland");

    eprintln!("AeroWM: WAYLAND_DISPLAY={socket_str}");

    // Spawn the startup command (e.g. a terminal) with all Wayland env vars baked in.
    if let Some(cmd) = startup_cmd {
        let (prog, argv) = cmd.split_first().unwrap();
        match Command::new(prog).args(argv).spawn()
        {
            Ok(_)  => eprintln!("AeroWM: spawned: {}", cmd.join(" ")),
            Err(e) => eprintln!("AeroWM: failed to spawn '{}': {e}", cmd.join(" ")),
        }
    }

    event_loop
        .run(None, &mut state, |_| {})
        .expect("Event loop failed");
    Ok(())
}
