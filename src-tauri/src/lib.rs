mod commands;
mod config_manager;
mod context;
mod engine;

use tauri::{Manager, RunEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::engine::EngineChild;
use commands::{get_app_config, trash_item};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(EngineChild::default())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app
                .get_webview_window("main")
                .expect("no main window")
                .set_focus();
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![trash_item, get_app_config])
        .setup(|app| {
            let handle = app.handle().clone();

            handle.plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    // 单个日志文件最大 1M（1_000_000 字节），超出后轮转
                    .max_file_size(1_000_000)
                    .build(),
            )?;

            let config = get_app_config(handle.clone())?;
            if let Err(e) = engine::start(&handle, &config) {
                handle
                    .dialog()
                    .message(format!("Application startup failed: {e}"))
                    .kind(MessageDialogKind::Error)
                    .title("System Error")
                    .show(move |result| match result {
                        true => handle.exit(0),
                        false => {}
                    });
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::Exit => {
                engine::stop(app);
            }
            _ => {}
        });
}
