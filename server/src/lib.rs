use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::RwLock;
use std::thread;

use engine::Engine;
use tauri::Manager as _;

mod attack;
mod chess;
mod common;
mod config;
mod engine;
mod listen;
mod logger;
mod practice;
mod worker;
mod yolo;

// 全局共享状态，用Arc和Mutex包装以实现线程安全共享
struct SharedState {
    config: Arc<RwLock<config::Config>>,
    engine: Arc<Mutex<Engine>>,
    listen_thread: Mutex<Option<thread::JoinHandle<()>>>,
}

static SHARED_STATE: OnceLock<SharedState> = OnceLock::new();

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            logger::init_tracer(tracing::Level::DEBUG, &app.path().app_data_dir().unwrap());

            let _ = SHARED_STATE.get_or_init(|| {
                let config = config::Config::load(&app.path().config_dir().unwrap());
                let lib_path = app.path().resolve("../libs/pikafish", tauri::path::BaseDirectory::Resource).unwrap();
                let (child, stdin, stop_stdin) = engine::command::new(&lib_path);
                let mut engine = engine::Engine::from_child(child, stdin, &lib_path);
                engine.set_show_wdl(config.engine.show_wdl);
                engine.set_hash(config.engine.hash);
                engine.set_threads(config.engine.threads);
                practice::set_stop_writer(stop_stdin);

                SharedState {
                    config: Arc::new(RwLock::new(config)),
                    engine: Arc::new(Mutex::new(engine)),
                    listen_thread: Mutex::new(None),
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            reload_engine,
            listen::list_windows,
            worker::start_listen,
            worker::stop_listen,
            config::get_engine_config,
            config::set_engine_depth,
            config::set_engine_time,
            config::set_engine_threads,
            config::set_engine_hash,
            config::set_engine_show_wdl,
            config::set_engine_multipv,
            config::set_engine_alt_score_gap,
            config::set_chessdb,
            practice::practice_start,
            practice::practice_moves,
            practice::practice_do_move,
            practice::practice_undo,
            practice::practice_reset,
            practice::practice_analyze,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn reload_engine(app: tauri::AppHandle) {
    let lib_path = app.path().resolve("../libs/pikafish", tauri::path::BaseDirectory::Resource).unwrap();
    let state = SHARED_STATE.get().unwrap();
    let engine_config = state.config.read().unwrap().engine;
    let mut engine = state.engine.lock().unwrap();
    let stop_stdin = engine.reload(&lib_path, &engine_config);
    practice::set_stop_writer(stop_stdin);
}
