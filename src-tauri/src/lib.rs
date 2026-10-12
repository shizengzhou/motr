mod commands;
mod config_manager;
mod context;
mod engine;
mod menu;
mod tray;

use tauri::{Manager, RunEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::engine::EngineChild;
use commands::{get_app_config, trash_item};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(EngineChild::default())
        // 菜单项 id -> 点击动作（由 menu::apply 在构建菜单时填充）
        .manage(menu::MenuActions::default())
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
        // 菜单结构与各语言文案都在 Rust 侧（src-tauri/resources/menus/），
        // 切换语言时前端调用 rebuild_menus 重建；
        // 菜单点击按 JSON 里的 command-before / command / command-after 分发
        .on_menu_event(|app, event| menu::on_menu_event(app, event.id().as_ref()))
        .invoke_handler(tauri::generate_handler![
            trash_item,
            get_app_config,
            menu::rebuild_menus,
            tray::set_tray_speedometer,
            tray::reset_tray_icon
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            handle.plugin(
                tauri_plugin_log::Builder::default()
                    .level(log::LevelFilter::Info)
                    // 单个日志文件最大 1M（1_000_000 字节），超出后轮转
                    .max_file_size(1_000_000)
                    .build(),
            )?;

            // 托盘：左键聚焦窗口，右键弹菜单（菜单项功能待接）
            tray::init(&handle)?;

            // App 菜单 + 托盘菜单：按当前平台和 user.json 里的语言构建
            // （菜单读不出来只记日志，不让 App 起不来）
            menu::init(&handle);

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
