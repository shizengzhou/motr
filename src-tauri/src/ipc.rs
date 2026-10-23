use std::path::Path;

use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};

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
/// 调用方：系统把 `.torrent` 递进来时（文件关联，见 `protocol::open_torrent_path`）——
/// 菜单里的「打开种子文件」由前端用 dialog + fs 插件自己读，不走这里。
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
