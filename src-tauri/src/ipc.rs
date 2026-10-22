use std::path::Path;

use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

/// 把命令下发给前端：前端 `Ipc.vue` 收到后交给 CommandManager 执行
///
/// 协议链接（protocol.rs）和自定义命令（如种子文件）都走这里 ——
/// 前端只需要认一种形状。
pub(crate) fn send_command_to_frontend(app: &AppHandle, command: &str, arg: Option<&Value>) {
    // 前端是 commands.execute(command, ...args)，所以参数要包成数组
    let args = arg.map_or_else(|| json!([]), |value| json!([value]));
    let payload = json!({ "command": command, "args": args });

    match app.emit("command", payload) {
        Ok(()) => log::info!("[Motr] command -> {command} (args: {args})"),
        Err(err) => log::error!("[Motr] 下发命令 `{command}` 失败: {err}"),
    }
}

/// 读种子文件 -> 前端要的 `{ name, dataURL }`
///
/// 与原项目一致：dataURL 是**裸 base64**（前端用 `base64StringToBlob` 还原成 File）。
/// 调用方：前端菜单的「打开种子文件」，以及系统把 `.torrent` 递进来时
/// （文件关联，见 `protocol::open_torrent_file`）。
pub(crate) fn torrent_payload(path: &Path) -> Result<Value, String> {
    if !path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
    {
        return Err(format!("不是种子文件: {}", path.display()));
    }

    let bytes =
        std::fs::read(path).map_err(|err| format!("读取 {} 失败: {err}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download.torrent")
        .to_string();

    Ok(json!({
        "name": name,
        "dataURL": BASE64_STANDARD.encode(&bytes),
    }))
}

/// 退出应用（等价原项目 `Application.quit`：先停引擎，再 `app.exit()`）
pub(crate) fn quit_app(app: &AppHandle) {
    log::info!("[Motr] quitting...");

    // RunEvent::Exit 里也会再调一次 engine::stop（幂等，第二次是空操作），
    // 这里显式先停，保证 aria2 在退出前就被收掉
    crate::engine::stop(app);

    app.exit(0);
}

/// 帮助菜单里的「切换开发者工具」（原项目用的是 Electron 的 toggledevtools role）
///
/// 依赖 Cargo.toml 里 tauri 的 `devtools` feature：
/// 没开的话这些方法在 release 构建里不存在（debug 构建本来就有）。
pub(crate) fn toggle_dev_tools(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("[Motr] 找不到主窗口，无法切换开发者工具");
        return;
    };

    if window.is_devtools_open() {
        window.close_devtools();
    } else {
        window.open_devtools();
    }
    log::info!("[Motr] devtools toggled");
}
