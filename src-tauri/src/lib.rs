mod auto_launch;
mod commands;
mod logging;
mod config_manager;
mod context;
mod engine;
mod menu;
mod run_mode;
mod tray;
mod window;

use tauri::{Manager, RunEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::engine::EngineChild;
use commands::{
    apply_auto_launch, apply_run_mode, get_app_config, rebuild_menus, reset_tray_icon,
    set_tray_speedometer, trash_item,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                // 只保存/恢复尺寸、位置、最大化、全屏（见 window.rs 的 STATE_FLAGS）
                .with_state_flags(window::STATE_FLAGS)
                // 关掉「窗口一就绪就自动恢复」，改成 setup 里按 user.json 的
                // 「恢复窗口状态」开关决定（window::restore_state）
                .skip_initial_state("main")
                .build(),
        )
        .plugin(
            tauri_plugin_autostart::Builder::new()
                // 开机自启拉起时带上的参数（与原项目 LOGIN_SETTING_OPTIONS 一致），
                // 用来区分"开机自启"和"手动启动"
                .args(["--opened-at-login=1"])
                .build(),
        )
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(EngineChild::default())
        // 菜单项 id -> 点击动作（由 menu::apply 在构建菜单时填充）
        .manage(menu::MenuActions::default())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            // 再启动一次时把已有窗口显示出来（原项目 second-instance -> showPage）
            window::show_main_window(app);
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
            rebuild_menus,
            set_tray_speedometer,
            reset_tray_icon,
            apply_run_mode,
            apply_auto_launch
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // 日志：等级取自 user.json 的 log-level（见 logging.rs）
            logging::init(&handle)?;

            // 托盘 + Dock 图标：按 user.json 里的运行模式（Standard / Tray / HideTray）
            // 决定托盘是否创建、Dock 图标是否显示（见 run_mode.rs）
            let run_mode = run_mode::init(&handle);

            // 「恢复上次退出时的窗口大小和位置」：开了才用上次的 bounds，
            // 要在显示窗口之前恢复，免得先出现默认尺寸再跳一下（见 window.rs）
            window::restore_state(&handle);

            // 窗口在 tauri.conf.json 里设的是 visible: false，这里决定要不要显示：
            // 托盘应用模式不显示；本次是被「开机自启」拉起时也不显示
            // （后者对应原项目 showPage 里的 `hidden: openedAtLogin || autoHideWindow`）
            if let Some(window) = handle.get_webview_window("main") {
                let visible = run_mode::window_visible_at_startup(run_mode)
                    && !auto_launch::opened_at_login();

                let _ = if visible {
                    window.show()
                } else {
                    window.hide()
                };
            }

            // App 菜单 + 托盘菜单：按当前平台和 user.json 里的语言构建
            // （菜单读不出来只记日志，不让 App 起不来）
            menu::init(&handle);

            // 窗口行为：开了「自动隐藏窗口」就失焦即隐藏
            window::init(&handle);

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
            // 进入退出流程：之后的关闭请求要放行（原项目 before-quit 里的 willQuit）
            RunEvent::ExitRequested { .. } => {
                window::mark_quitting();
            }
            RunEvent::Exit => {
                engine::stop(app);
            }
            // 点 Dock 图标：把窗口显示出来（原项目 app.on('activate') -> showPage）
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                window::show_main_window(app);
            }
            _ => {}
        });
}
