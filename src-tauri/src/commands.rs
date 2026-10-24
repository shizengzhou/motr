use tauri::{AppHandle, Manager};

use crate::{protocol, upnp};

#[tauri::command]
pub fn protocol_ready(app: AppHandle) -> Result<(), String> {
    protocol::ready(&app);
    Ok(())
}

/// 切换主窗口开发者工具（菜单 `help.toggle-dev-tools`）。
///
/// Tauri 的 PredefinedMenuItem 没有 Electron 的 `toggledevtools` role，
/// 所以要显式调 WebView 的 devtools API；Cargo.toml 里已开 `devtools` feature。
#[tauri::command]
pub fn toggle_devtools(app: AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("[Motr] 找不到主窗口，无法切换开发者工具");
        return;
    };

    if window.is_devtools_open() {
        window.close_devtools();
    } else {
        window.open_devtools();
    }
}

#[tauri::command]
pub fn shutdown_upnp(app_handle: AppHandle) -> Result<(), String> {
    upnp::shutdown(&app_handle);
    Ok(())
}

/// 设置页保存后调用：让路由器上的 UPnP 端口映射与
/// user.json 的 enable-upnp、system.json 的两个监听端口对齐（见 upnp.rs）
#[tauri::command]
pub fn apply_upnp(app: AppHandle) -> Result<(), String> {
    upnp::apply(&app);
    Ok(())
}

/// 删除文件到回收站（任务操作里的「同时删除文件」、删记录时用）
#[tauri::command]
pub async fn trash_item(path: String) -> Result<(), String> {
    let target = path.clone();

    match tauri::async_runtime::spawn_blocking(move || trash::delete(&path)).await {
        Ok(Ok(())) => Ok(()),
        // 真正的删除失败（文件不存在、权限不足、回收站不可用等）
        Ok(Err(err)) => {
            log::warn!("trash_item failed for {target}: {err}");
            Err(err.to_string())
        }
        // 阻塞任务本身出错（panic / 被取消）
        Err(err) => {
            log::warn!("trash_item task failed for {target}: {err}");
            Err(err.to_string())
        }
    }
}
