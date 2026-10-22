//! 「文件关联」：把系统递进来的 `.torrent` 文件转成前端命令。
//!
//! Deep Link（`magnet:` / `thunder:` / `mo:` / `motrix:` 的监听、解析、注册）
//! 已经迁到前端 `src/main/ProtocolManager.js`；这里只保留前端拿不到的两类入口：
//!
//! - **macOS / iOS / Android**：系统通过 `RunEvent::Opened` 把文件递进来
//!   （见 lib.rs）；
//! - **Windows / Linux**：文件关联就是命令行参数（首次启动在
//!   `std::env::args`，再次启动由 single-instance 回调收到）。
//!
//! 链接 URL 不在这里处理：single-instance 插件开了 `deep-link` feature，
//! 会把参数里的 URL 转给 deep-link 插件，再由前端 `onOpenUrl` 接住；
//! 这里再处理一次会导致同一个链接弹两次窗。

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::{ipc, window};

/// 前端挂载前收到的命令要攒着（冷启动直接被 `.torrent` 拉起时的必经之路）
#[derive(Default)]
pub struct ProtocolState {
    /// 前端是否已经挂上 `command` 监听（`Ipc.vue` 的 created 里调 `protocol_ready`）
    ready: bool,
    /// 还没能发出去的「前端命令 + 参数」
    pending: Vec<(String, Value)>,
}

/// 带着锁读写状态（闭包必须是同步的，拿锁跨 `await` 的用法这里没有）
fn with_state<T>(app: &AppHandle, action: impl FnOnce(&mut ProtocolState) -> T) -> T {
    let state = app.state::<Mutex<ProtocolState>>();
    let mut state = state.lock().unwrap();

    action(&mut state)
}

/// 处理 `RunEvent::Opened` 里的文件 URL；非文件 URL 交给前端 Deep Link
pub fn handle_file_urls(app: &AppHandle, urls: impl IntoIterator<Item = Url>) {
    for url in urls {
        if url.scheme() != "file" {
            continue;
        }

        log::info!("[Motr] 打开文件: {url}");
        open_torrent_file(app, &url);
    }
}

/// 双击（或 `open -a`）`.torrent` 时系统把文件交给我们
fn open_torrent_file(app: &AppHandle, url: &Url) {
    let path = match url.to_file_path() {
        Ok(path) => path,
        Err(_) => {
            log::warn!("[Motr] 无法把 {url} 解析成本地路径");

            return;
        }
    };

    open_torrent_path(app, &path);
}

/// 处理命令行参数里的文件关联。
///
/// 只挑"看起来是本地种子文件"的参数，其余（argv[0]、`--xxx`、
/// macOS 的 `-psn_...`、以及各种 URL）一律跳过。
pub fn handle_open_args(app: &AppHandle, args: impl IntoIterator<Item = String>, cwd: &str) {
    for arg in args {
        if !looks_like_torrent_arg(&arg) {
            continue;
        }

        // 相对路径按调用方给的工作目录解析（系统递进来的一般是绝对路径，
        // 但用户也可能在终端里 `Motr ./x.torrent`）
        let path = PathBuf::from(&arg);
        let path = if path.is_absolute() {
            path
        } else {
            Path::new(cwd).join(path)
        };

        open_torrent_path(app, &path);
    }
}

/// 参数看上去是不是本地种子文件：不是 URL、且以 `.torrent` 结尾（纯判断，方便测）
fn looks_like_torrent_arg(arg: &str) -> bool {
    // URL 参数交给 deep-link 那条路。
    // 注意 `magnet:` / `thunder:` 不带 `//`，要单独看
    let lower = arg.to_ascii_lowercase();
    if lower.contains("://") || lower.starts_with("magnet:") || lower.starts_with("thunder:") {
        return false;
    }

    Path::new(arg)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("torrent"))
}

/// 本地种子文件的共同落点：读出来转成前端要的 `{ name, dataURL }` 并下发命令。
///
/// 复用菜单那条路（`ipc::torrent_payload`），并且同样经过 [`dispatch`] 的
/// "前端没就绪就先攒着"队列 —— 冷启动直接被文件拉起时就靠这个
fn open_torrent_path(app: &AppHandle, path: &Path) {
    // 窗口默认是隐藏的（托盘型应用），不显式显示的话用户什么都看不到
    window::show_main_window(app);

    match ipc::torrent_payload(path) {
        Ok(payload) => dispatch(app, "application:new-bt-task-with-file", payload),
        Err(err) => log::warn!("[Motr] 打开种子文件失败: {err}"),
    }
}

/// 下发一条前端命令（`Ipc.vue` 收到 `command` 事件后交给 CommandManager）。
///
/// 前端还没挂上监听就先攒着，等 [`ready`] 再补发 —— 冷启动就带文件时，
/// 前端必然还没就绪，直接 emit 会丢。
fn dispatch(app: &AppHandle, command: &str, arg: Value) {
    // 判断和"发送/入队"放在同一个锁里：分开做的话，恰好在这两步之间
    // 前端调 protocol_ready 就会把命令漏掉（进队列之后再也没人来取）
    with_state(app, |state| {
        if state.ready {
            ipc::send_command_to_frontend(app, command, Some(&arg));
            return;
        }

        log::info!("[Motr] 前端还没就绪，先攒下 `{command}`");
        state.pending.push((command.to_string(), arg));
    });
}

/// 前端挂上 `command` 监听后调用（commands::protocol_ready）：补发攒下的命令
pub fn ready(app: &AppHandle) {
    let pending = with_state(app, |state| {
        state.ready = true;
        std::mem::take(&mut state.pending)
    });

    for (command, arg) in pending {
        log::info!("[Motr] 补发启动期间的命令 `{command}`");
        ipc::send_command_to_frontend(app, &command, Some(&arg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 命令行参数的筛选：只认本地种子文件，URL 一律交给 deep-link 那条路
    ///（否则同一个链接会被处理两次、弹两次新建任务）
    #[test]
    fn only_local_torrent_args_are_picked() {
        for ok in [
            "/Users/me/Downloads/a.torrent",
            r"C:\Users\me\Downloads\b.TORRENT",
            "./c.torrent",
            "d.torrent",
        ] {
            assert!(looks_like_torrent_arg(ok), "应被识别为种子文件: {ok}");
        }

        for skip in [
            "/Applications/Motr.app/Contents/MacOS/Motr", // argv[0]：可执行文件
            "-psn_0_12345",                               // macOS 启动参数
            "--hidden",                                   // 普通开关
            "magnet:?xt=urn:btih:abc&dn=foo.torrent",     // URL：deep-link 管
            "thunder://QUFodHRwOi8vZXhhbXBsZS5jb20vYS50b3JyZW50",
            "https://example.com/a.torrent",
            "/tmp/notatorrent", // 没有扩展名
            "/tmp/a.torrent.bak",
        ] {
            assert!(!looks_like_torrent_arg(skip), "不该被识别: {skip}");
        }
    }
}
