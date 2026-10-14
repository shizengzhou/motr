mod auto_launch;
mod commands;
mod config_manager;
mod context;
mod engine;
mod logging;
mod menu;
mod protocol;
mod proxy;
mod run_mode;
mod tray;
mod upnp;
mod window;

use std::sync::Mutex;

use tauri::{Manager, RunEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

use crate::engine::EngineChild;
use commands::{
    apply_auto_launch, apply_protocols, apply_run_mode, apply_upnp, factory_reset, get_app_config,
    protocol_ready, rebuild_menus, relaunch, reset_session, reset_tray_icon, set_tray_speedometer,
    trash_item,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        // 自动更新：检查 → 下载 → 安装 → 重启（配置见 tauri.conf.json 的 plugins.updater）
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // 应用自身的 HTTP 请求（Tracker 列表抓取等）：请求由 Rust 发出，
        // 不受 webview 的 CORS 限制，且能按请求指定代理
        .plugin(tauri_plugin_http::init())
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
        // UPnP：已建好的映射 + 缓存的网关（见 upnp.rs）
        .manage(Mutex::new(upnp::UpnpState::default()))
        // 下载协议：前端就绪前攒下的协议链接（见 protocol.rs）
        .manage(Mutex::new(protocol::ProtocolState::default()))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            // 再启动一次时把已有窗口显示出来（原项目 second-instance -> showPage）。
            // 参数里若带 `magnet:...`，single-instance 插件（开了 deep-link feature）
            // 会先转给 deep-link 插件，再走到 protocol.rs 的 handle_urls；
            // 剩下的交给 handle_open_args —— Windows/Linux 的文件关联就是
            // 「`Motr xxx.torrent`」这种命令行参数（macOS 走 RunEvent::Opened）
            window::show_main_window(app);
            protocol::handle_open_args(app, args, &cwd);
        }))
        // 「下载协议」：magnet: / thunder: 的声明在 tauri.conf.json 的
        // plugins.deep-link 里（macOS 打包时写进 Info.plist），
        // 运行时注册见 protocol.rs
        .plugin(tauri_plugin_deep_link::init())
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
            apply_auto_launch,
            apply_upnp,
            apply_protocols,
            protocol_ready,
            reset_session,
            factory_reset,
            relaunch
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
            //
            // 例外：用户主动重启（设置里改端口要重启、装完更新要重启）时窗口必须出现 ——
            // 上面两条判断都不该拦住它。标记由 `commands::relaunch` 在重启前写下。
            //
            // 注意这里只负责"可见"：聚焦要等应用真正跑起来（见事件循环里的
            // `RunEvent::Ready`）—— `setup` 本身就是 Tauri 在 `RunEvent::Ready`
            // 内部同步调的，那一刻 `set_focus()` 常常不算数
            let focus_after_restart = window::take_focus_after_restart(&handle);

            if focus_after_restart {
                // 用 warn 级：release 构建里 log 的编译期等级会把 info 整条编掉
                log::warn!("[Motr] 检测到主动重启标记：本次启动会显示窗口并在就绪后聚焦");
                window::show_main_window(&handle);
            } else if let Some(window) = handle.get_webview_window("main") {
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

            // 「下载协议」：按 user.json 的 protocols 注册 / 取消系统默认客户端，
            // 并接上系统递链接的两条路（见 protocol.rs）。
            // 冷启动就带链接时前端还没挂载，会先攒着，等 protocol_ready 再补发
            protocol::apply(&handle);
            protocol::init(&handle);

            // 首次启动就带文件：Windows / Linux 的文件关联走命令行参数
            //（macOS 走 RunEvent::Opened，见下面的事件循环）
            #[cfg(any(target_os = "windows", target_os = "linux"))]
            protocol::handle_open_args(
                &handle,
                std::env::args().skip(1),
                &std::env::current_dir()
                    .map(|dir| dir.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );

            // 「启用 UPnP」：给 BT / DHT 的监听端口在路由器上开洞，
            // 之后按租约定期续租（见 upnp.rs）。异步做：SSDP 发现最长要几秒，
            // 不能卡住启动；与引擎起没起来无关（映射先建好，等引擎来监听）
            upnp::apply(&handle);
            upnp::spawn_renewal_task(&handle);

            let mut config = get_app_config(handle.clone())?;

            // 代理：把 user.json 里的 proxy 映射进 system.json 的 all-proxy/no-proxy
            //（原项目 ConfigManager.fixSystemConfig），必须在起引擎之前
            proxy::apply(&handle, &mut config);

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
            // 应用就绪：这里才是"重启后补聚焦"的正确时机。
            //
            // 为什么不在 setup 里一次做完：`setup` 是 Tauri 在 `RunEvent::Ready`
            // 内部同步调的，那一刻窗口刚创建、应用还没激活，`set_focus()` 不算数；
            // 而且 macOS 14+ 起 `activateIgnoringOtherApps:` 被系统忽略（协作式激活），
            // 自我重启的新进程需要主动调用现代 API 才能到前台 —— 具体见 window.rs 的
            // `activate_app` / `focus_main_window_until_focused`（后者按 400ms 重试，
            // 直到窗口真的拿到焦点为止，拿到就收手）
            RunEvent::Ready => {
                if window::focus_after_restart_pending() {
                    window::focus_main_window_until_focused(app);
                }
            }
            // 进入退出流程：之后的关闭请求要放行（原项目 before-quit 里的 willQuit）
            RunEvent::ExitRequested { .. } => {
                window::mark_quitting();

                // 这里先于 `Exit` 触发，正好赶在 store 插件把**缓存**写盘之前刷新它，
                // 否则前端（JS LazyStore）本次会话写的内容会被启动时的旧缓存覆盖
                //（典型表现：last-check-update-time 推不动 ⇒ 每次启动都检查更新）
                config_manager::refresh_stores(app);
            }
            RunEvent::Exit => {
                engine::stop(app);
                // 退出前尽力撤销路由器上的端口映射（同步跑完，见 upnp.rs）
                upnp::shutdown(app);
            }
            // 点 Dock 图标：把窗口显示出来（原项目 app.on('activate') -> showPage）
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                window::show_main_window(app);
            }
            // 系统把文件交给我们：双击 `.torrent`（或 `open -a Motr xxx.torrent`）。
            // macOS/iOS/Android 走这个事件，urls 是 file:// 形式，交给 protocol
            // 按"和菜单打开种子文件一样"的方式处理。
            // 注意：Windows / Linux 的关联是走命令行参数（首次启动在 std::env::args，
            // 再次启动由 single-instance 插件回调收到），这条还没接。
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            RunEvent::Opened { urls } => {
                protocol::handle_urls(app, urls);
            }
            _ => {}
        });
}
