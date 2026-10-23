use tauri::{AppHandle, Manager};

/// 显示并聚焦主窗口（托盘左键、菜单里的 application:show、再启动一次、点 Dock 图标都用它）
///
/// 启动 / 重启后的显示与聚焦由前端负责：`main.js` 在应用初始化完成后调
/// `windowManager.showWindow()`（内部含 `setFocus`），时机在应用真正跑起来之后，
/// 不像 `setup` 那样受"窗口刚创建、应用未激活"影响
pub(crate) fn show_main_window(app: &AppHandle) {
    // macOS: 应用被 hide 掉时窗口不会出现，需要先 unhide
    #[cfg(target_os = "macos")]
    let _ = app.show();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
