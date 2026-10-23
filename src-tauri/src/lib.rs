mod commands;
mod config_manager;
mod energy;
mod ipc;
mod logging;
mod protocol;
mod upnp;
mod window;

use std::sync::Mutex;

use tauri::RunEvent;

use commands::{apply_upnp, protocol_ready, shutdown_upnp, trash_item};
use energy::{start_power_save_blocker, stop_power_save_blocker};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        // 自动更新：检查 → 下载 → 安装 → 重启（配置见 tauri.conf.json 的 plugins.updater）
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // 应用自身的 HTTP 请求（Tracker 列表抓取等）：请求由 Rust 发出，
        // 不受 webview 的 CORS 限制，且能按请求指定代理
        .plugin(tauri_plugin_http::init())
        .plugin(
            tauri_plugin_window_state::Builder::new()
                // 只保存尺寸和位置（恢复由前端按「恢复窗口状态」开关调用
                // restoreStateCurrent；插件默认的无条件自动恢复要关掉）
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        .union(tauri_plugin_window_state::StateFlags::POSITION),
                )
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
        // UPnP：已建好的映射 + 缓存的网关（见 upnp.rs）
        .manage(Mutex::new(upnp::UpnpState::default()))
        // 下载协议：前端就绪前攒下的协议链接（见 protocol.rs）
        .manage(Mutex::new(protocol::ProtocolState::default()))
        // 「有下载时不休眠」持有的电源断言（见 energy.rs）
        .manage(energy::EnergyState::default())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            // 参数里若带 `magnet:...`，single-instance 插件（开了 deep-link feature）
            // 会先转给 deep-link 插件，由前端 ProtocolManager 的 onOpenUrl 接住；
            // 剩下的交给 handle_open_args —— Windows/Linux 的文件关联就是
            // 「`Motr xxx.torrent`」这种命令行参数（macOS 走 RunEvent::Opened）。
            // 窗口显示由前端负责（main.js 启动即 showWindow），这里不用管
            protocol::handle_open_args(app, args, &cwd);
        }))
        // 「下载协议」：scheme 的声明在 tauri.conf.json 的 plugins.deep-link
        //（macOS 打包时写进 Info.plist）；运行时注册与监听由前端
        // ProtocolManager 通过 deep-link 插件完成
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        // App 菜单与托盘菜单都由前端构建（MenuManager / TrayManager），
        // 点击动作也由前端分发，Rust 不再注册全局菜单事件处理
        .invoke_handler(tauri::generate_handler![
            trash_item,
            apply_upnp,
            protocol_ready,
            shutdown_upnp,
            start_power_save_blocker,
            stop_power_save_blocker
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            // 日志：等级取自 user.json 的 log-level（见 logging.rs）
            logging::init(&handle)?;

            // 运行模式（托盘、Dock 图标、窗口显隐）由前端 Application 按 user.json 应用

            // 小屏幕适配由前端 WindowManager.fitWindowToDisplay 处理
            //（开了「恢复窗口状态」时以上次保存的 bounds 为准，不再缩放）

            // 「恢复上次退出时的窗口大小和位置」由前端 WindowManager.restoreWindowState
            // 按开关决定（要在显示窗口之前，免得先出现默认尺寸再跳一下）

            // 「隐藏菜单栏」→ 窗口装饰（原生标题栏）由前端 WindowManager.applyHideAppMenu
            // 在窗口显示之前应用，避免闪一下原生标题栏

            // 窗口显隐 / 聚焦由前端负责：main.js 在应用初始化完成后无条件 showWindow()
            //（含 setFocus），重启后同样走这条路，不需要 Rust 侧的标记文件

            // Deep Link（magnet: / mo: 等）的注册与监听由前端 ProtocolManager
            // 负责；Rust 只处理器命令行 / RunEvent::Opened 递进来的 .torrent 文件。
            // 冷启动就带文件时前端还没挂载，会先攒着，等 protocol_ready 再补发

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

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::Exit => {
                // 退出前尽力撤销路由器上的端口映射（同步跑完，见 upnp.rs）
                upnp::shutdown(app);
                // 释放电源断言（原项目 quit 前也会 stopPowerSaveBlocker）
                energy::shutdown(app);
            }
            // 点 Dock 图标：把窗口显示出来（原项目 app.on('activate') -> showPage）
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => {
                window::show_main_window(app);
            }
            // 系统把文件交给我们：双击 `.torrent`（或 `open -a Motr xxx.torrent`）。
            // macOS/iOS/Android 走这个事件，urls 是 file:// 形式，交给 protocol
            // 按"和菜单打开种子文件一样"的方式处理。
            // 注意：Windows / Linux 的关联走命令行参数（首次启动在 std::env::args，
            // 再次启动由 single-instance 插件回调收到）
            #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
            RunEvent::Opened { urls } => {
                protocol::handle_file_urls(app, urls);
            }
            _ => {}
        });
}
