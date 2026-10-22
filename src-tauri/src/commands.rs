use tauri::{AppHandle, Manager};
use tauri_plugin_os::{arch, platform};

use crate::config_manager::{self, Config};
use crate::context::{Context, get_aria2_conf_path, get_session_path};
use crate::{engine, ipc, protocol, upnp, window};

/// 设置页保存：把「用户配置」「系统配置」写进 user.json / system.json。
///
/// 前端传进来的是已经分好类、kebab-case 的两份键值（`Api.js` 的 `separateConfig`）。
/// 为什么不让前端直接写文件：见 `config_manager::write_keys` 的说明 —— 前端与 Rust
/// 各有一份 store 缓存，前端直写会让 Rust 的缓存变旧（读不到新值、退出时被覆盖回去）。
#[tauri::command]
pub fn save_config(
    app: AppHandle,
    user: Option<serde_json::Map<String, serde_json::Value>>,
    system: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<(), String> {
    if let Some(user) = user {
        config_manager::write_keys(&app, "user.json", &user)?;
    }

    if let Some(system) = system {
        config_manager::write_keys(&app, "system.json", &system)?;
    }

    Ok(())
}

/// 设置页点「恢复初始设置」时调用：停引擎 → 清空两份配置 → 重启应用。
///
/// 对齐上游 `application:factory-reset`（`offConfigListeners()` +
/// `configManager.reset()` + `relaunch()`）：`offConfigListeners` 那步在这个移植版
/// 没有对应物 —— 配置监听要么在前端，要么是每次现读（如 `apply_run_mode`）
#[tauri::command]
pub fn factory_reset(app: AppHandle) -> Result<(), String> {
    // 必须先停引擎：`app.restart()` 内部只做 `cleanup_before_exit()`，不会发
    // `RunEvent::Exit`，shell 插件的收子进程逻辑因此不会跑
    engine::stop(&app);

    config_manager::reset_stores(&app)?;

    restart_actively(app)
}

/// 重启应用（前端 `utils/native.js` 的 `restartApp`、「更新下载完重启」也用它）。
///
/// 同样的理由要先停引擎：直接 `app.restart()` 会留下一个占着 RPC 端口的孤儿
/// aria2（新进程的引擎起不来，界面却还连着旧的 —— 上一轮那个"重置无效"就是这么来的）
#[tauri::command]
pub fn relaunch(app: AppHandle) -> Result<(), String> {
    engine::stop(&app);

    restart_actively(app)
}

/// 用户主动重启的公共收尾：打上"重启后显示并聚焦"的标记 → 重启。
///
/// 标记这一步不能省：新进程只有看到它，才会跳过"托盘模式 / 开机自启就不显示窗口"
/// 那套判断，并在就绪后主动激活到前台（见 lib.rs 的启动显示逻辑与 window.rs 的
/// `focus_main_window_until_focused`）。漏掉的表现就是"重启后窗口压在别的应用后面，
/// 得点一下 Dock 才出来"—— `factory_reset` 之前就漏了这一步。
fn restart_actively(app: AppHandle) -> Result<(), String> {
    window::mark_focus_after_restart(&app);

    restart_after_response(app);

    Ok(())
}

/// 让这次 invoke 的响应先回到前端，再重启应用
///（`app.restart()` 不返回，同步调用的话前端拿不到响应）
fn restart_after_response(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        app.restart();
    });
}

/// 设置页点「重置下载会话记录」时调用：停引擎、删掉 download.session、重新起引擎
///（流程与原项目 `Application.resetSession()` 一致，见 engine.rs）
#[tauri::command]
pub fn reset_session(app: AppHandle) -> Result<(), String> {
    engine::reset_session(&app);
    Ok(())
}

/// 前端挂上 `command` 监听后调用：补发冷启动时攒下的种子文件命令（见 protocol.rs）
#[tauri::command]
pub fn protocol_ready(app: AppHandle) -> Result<(), String> {
    protocol::ready(&app);
    Ok(())
}

/// 设置页保存后调用：让路由器上的 UPnP 端口映射与
/// user.json 的 enable-upnp、system.json 的两个监听端口对齐（见 upnp.rs）
#[tauri::command]
pub fn apply_upnp(app: AppHandle) -> Result<(), String> {
    upnp::apply(&app);
    Ok(())
}

/// 读种子文件 -> 前端要的 `{ name, dataURL }`（裸 base64）
///
/// 前端菜单的「打开种子文件」用系统对话框选完文件后调它 —— 文件读取留在 Rust，
/// 前端就不需要给 fs 插件开任意路径的读权限。
#[tauri::command]
pub fn read_torrent_file(path: String) -> Result<serde_json::Value, String> {
    ipc::torrent_payload(std::path::Path::new(&path))
}

/// 退出应用（等价原项目 `Application.quit`：先停引擎，再 `app.exit()`）
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    ipc::quit_app(&app)
}

/// 切换主窗口的开发者工具（App 菜单里的 help.toggle-dev-tools 用）
#[tauri::command]
pub fn toggle_devtools(app: AppHandle) {
    ipc::toggle_dev_tools(&app)
}

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

#[tauri::command]
pub fn get_app_config(app_handle: AppHandle) -> Result<Config, String> {
    let context = Context {
        platform: platform().to_string(),
        arch: arch().to_string(),
        log_path: app_handle
            .path()
            .app_log_dir()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        session_path: get_session_path(&app_handle),
        engine_path: String::new(),
        aria2_bin_path: String::new(),
        aria2_conf_path: get_aria2_conf_path(&app_handle),
    };

    // user.json / system.json：重读磁盘（前端可能刚写过）→ 缺的键用默认值兜底 →
    // 把合并结果写回文件。原版用 electron-store 的 defaults，配置一读就把默认值落盘，
    // 所以配置文件始终是完整可读的；移植后丢了这步，文件里只剩"被改过的键"
    //（例：last-check-update-time 从没被写过就查不到它当前是几）。
    let user_config = config_manager::load_user_config(&app_handle);
    let system_config = config_manager::load_system_config(&app_handle);

    Ok(Config {
        context,
        user_config,
        system_config,
    })
}
