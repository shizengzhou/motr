//! 「下载协议」：把 Motr 注册成 `magnet:` / `thunder:` 的默认打开方式、
//! 常驻注册自有协议 `mo:` / `motrix:`，并把系统递过来的链接交给该去的地方。
//!
//! 对齐原项目 `src/main/core/ProtocolManager.js`：
//!
//! - `setup()` → [`apply`]：按 user.json 的 `protocols` 逐个注册 / 取消注册；
//!   已经是默认客户端的就不重复设（原项目先问 `isDefaultProtocolClient`）。
//!   自有协议 `mo:` / `motrix:` 不受设置控制，永远注册（原项目里写死 true）
//! - `handle()` → [`handle_urls`] + [`action_of`]：分三类
//!   - 资源链接（`magnet:` / `thunder:` / `http(s):` / `ftp:`）→ 「新建任务」
//!   - 自有协议 `mo://<host>?...` → 按 host 找前端命令（见 [`OWN_COMMANDS`]）
//!   - 在文件管理器里定位文件（上游在主进程做的那条）
//! - 开发构建不动注册（原项目 `is.dev()` 直接 return）：否则会把 `magnet:`
//!   指向 `target/debug` 下的构建产物
//!
//! 平台差异来自 Tauri 的 deep-link 插件，不是移植取舍：
//!
//! - **Windows**：写 `HKCU\Software\Classes\<scheme>`，运行时能注册也能取消
//! - **Linux**：往 `~/.local/share/applications` 写 desktop 文件，
//!   需要系统里有 `xdg-mime` 与 `update-desktop-database`
//! - **macOS**：协议在**打包时**由 `tauri.conf.json` 的
//!   `plugins.deep-link.desktop.schemes` 声明进 Info.plist，运行时没有对应
//!   API（插件在 macOS 上返回 `UnsupportedPlatform`），所以设置里的两个开关
//!   在 macOS 上不改变系统行为 —— 详见 [`apply`]
//!
//! 这里的 `AppHandle` 不带泛型，因为要调 `menu::send_command_to_frontend`
//! 与 `window::show_main_window`（它们都是具体的 `&AppHandle`）。

use std::sync::Mutex;

use serde_json::{Map, Value, json};
use tauri::{AppHandle, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_opener::OpenerExt;
use url::Url;

use crate::config_manager::{self, Protocols};
use crate::{menu, window};

/// 能从系统递进来、并且能下载的 scheme（原项目 `ProtocolManager.handle` 里那张前缀表）
const RESOURCE_SCHEMES: [&str; 5] = ["ftp", "http", "https", "magnet", "thunder"];

/// 自有协议：`mo://` 与 `motrix://`。
///
/// 原项目里它们是写死的 `true`（`this.protocols = { mo: true, motrix: true, ...options }`），
/// 不受设置页那两个开关控制 —— 这里也一样，见 [`apply`]。
const OWN_SCHEMES: [&str; 2] = ["mo", "motrix"];

/// `mo://<host>?<query>` 里的 host -> 前端命令。
///
/// 与原项目 `src/main/configs/protocol.js` 的 8 条一一对应，只有
/// `reveal-in-folder` 不在表里：上游那条是**主进程**处理的
/// （`sendCommandToAll` 先 `this.emit`，有本地监听就不往下发），
/// 所以移植版也在 Rust 侧做，见 [`Action::RevealInFolder`]。
const OWN_COMMANDS: [(&str, &str); 7] = [
    ("task-list", "application:task-list"),
    ("new-task", "application:new-task"),
    ("new-bt-task", "application:new-bt-task"),
    ("pause-all-task", "application:pause-all-task"),
    ("resume-all-task", "application:resume-all-task"),
    ("preferences", "application:preferences"),
    ("about", "application:about"),
];

/// 上游在主进程处理的那条自有协议
const REVEAL_IN_FOLDER: &str = "reveal-in-folder";

/// 设置页里那两个开关对应的协议（顺序固定，magnet 在前）
pub fn wanted_schemes(protocols: &Protocols) -> [(&'static str, bool); 2] {
    [
        ("magnet", protocols.magnet),
        ("thunder", protocols.thunder),
    ]
}

/// 一条协议链接该干什么 —— 对应原项目 `ProtocolManager.handle()` 里的分派
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// 要下载的资源：原样交给「新建任务」（前端 `addTask` 会预填进对话框）
    NewTask(String),
    /// 自有协议对应的一条前端命令（`Ipc.vue` 收到后交给 CommandManager）
    Command {
        command: &'static str,
        args: Map<String, Value>,
    },
    /// 在文件管理器里定位文件；带 gid 时再让前端展开那条任务的详情
    RevealInFolder {
        path: Option<String>,
        gid: Option<String>,
    },
    /// 不认识的链接（原项目也是直接 return）
    Ignore,
}

/// 把系统递进来的链接翻译成动作。
///
/// 参数解析用 form-urlencoded（`url` 的 `query_pairs`），和原项目
/// `querystring.parse(search)` 是同一套解码规则。
pub fn action_of(url: &Url) -> Action {
    if RESOURCE_SCHEMES.contains(&url.scheme()) {
        // 原样带上：thunder 的 base64 就在 host 位置，改一个字母就废了
        return Action::NewTask(url.as_str().to_string());
    }

    if !OWN_SCHEMES.contains(&url.scheme()) {
        return Action::Ignore;
    }

    let Some(host) = url.host_str() else {
        return Action::Ignore;
    };

    let args: Map<String, Value> = url
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), Value::String(value.into_owned())))
        .collect();

    if host == REVEAL_IN_FOLDER {
        let take = |key: &str| {
            args.get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
        };

        return Action::RevealInFolder {
            path: take("path"),
            gid: take("gid"),
        };
    }

    match OWN_COMMANDS.iter().find(|(name, _)| *name == host) {
        Some((_, command)) => Action::Command { command, args },
        None => Action::Ignore,
    }
}

/// 前端挂载前收到的命令要攒着（冷启动就带着链接时的必经之路）
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

/// 让系统里的默认客户端与 user.json 的 `protocols` 对齐，并挂上有自协议。
///
/// 启动时（lib.rs setup）和设置页保存后（commands::apply_protocols）各调一次，
/// 对应原项目的 `initProtocolManager()` 与 `watchProtocolsChange()`。
pub fn apply(app: &AppHandle) {
    if tauri::is_dev() {
        log::info!("[Motr] 开发构建不注册系统协议（原项目 is.dev() 同样直接 return）");

        return;
    }

    let protocols = config_manager::protocols(app);

    // 自有协议永远是开的（原项目写死 true），排在设置页那两个之后
    let wanted = wanted_schemes(&protocols)
        .into_iter()
        .chain(OWN_SCHEMES.map(|scheme| (scheme, true)));

    for (scheme, enabled) in wanted {
        // macOS 的协议是打包时声明进 Info.plist 的，运行时没有 API 可改
        //（deep-link 插件在 macOS 上返回 UnsupportedPlatform）
        if config_manager::is_macos() {
            log::info!(
                "[Motr] macOS 上 `{scheme}` 由打包声明决定（改 tauri.conf.json 的 \
                 plugins.deep-link），设置里这个开关不改系统行为（当前：{}）",
                if enabled { "开" } else { "关" }
            );

            continue;
        }

        let deep_link = app.deep_link();

        if enabled {
            // 原项目先 isDefaultProtocolClient 问一遍，已经是默认客户端就不重复设
            if deep_link.is_registered(scheme).unwrap_or(false) {
                log::debug!("[Motr] `{scheme}` 已经是默认客户端");

                continue;
            }

            match deep_link.register(scheme) {
                Ok(()) => log::info!("[Motr] `{scheme}` 已设为默认客户端"),
                Err(err) => log::warn!("[Motr] `{scheme}` 设为默认客户端失败: {err}"),
            }
        } else {
            // 本来就不是我们（比如 thunder 默认关着、从没注册过）：不用去删，
            // 否则每次启动都刷一条"取消失败"
            //（查不出来时按 true 处理，走原来的"试着取消"）
            if !deep_link.is_registered(scheme).unwrap_or(true) {
                log::debug!("[Motr] `{scheme}` 本来就不是默认客户端");

                continue;
            }

            match deep_link.unregister(scheme) {
                Ok(()) => log::info!("[Motr] `{scheme}` 已取消默认客户端"),
                Err(err) => log::warn!("[Motr] `{scheme}` 取消默认客户端失败: {err}"),
            }
        }
    }
}

/// 接上「系统递进来链接」的两条路：
///
/// - **运行中**：deep-link 插件的 `deep-link://new-url` 事件。macOS 来自
///   `open-url`；Windows / Linux 来自第二实例的命令行参数 —— single-instance
///   插件开了 `deep-link` feature，会把参数先转给 deep-link 插件
/// - **冷启动**：插件在自己的 setup 里就把参数收下了，这里用 `get_current()` 取出来
pub fn init(app: &AppHandle) {
    let handle = app.clone();

    app.deep_link().on_open_url(move |event| {
        handle_urls(&handle, event.urls());
    });

    match app.deep_link().get_current() {
        Ok(Some(urls)) => handle_urls(app, urls),
        Ok(None) => {}
        Err(err) => log::warn!("[Motr] 取启动参数里的协议链接失败: {err}"),
    }
}

/// 处理系统递进来的一批链接（原项目 `ProtocolManager.handle`）
pub fn handle_urls(app: &AppHandle, urls: impl IntoIterator<Item = Url>) {
    for url in urls {
        log::info!("[Motr] 协议链接: {url}");

        // 原项目 handleProtocol：先把窗口带到前台，再处理
        window::show_main_window(app);

        match action_of(&url) {
            Action::NewTask(resource) => dispatch(
                app,
                "application:new-task",
                json!({ "type": "uri", "uri": resource }),
            ),
            Action::Command { command, args } => {
                dispatch(app, command, Value::Object(args));
            }
            Action::RevealInFolder { path, gid } => reveal_in_folder(app, path, gid),
            Action::Ignore => log::debug!("[Motr] 忽略不认识的协议链接: {url}"),
        }
    }
}

/// 在系统的文件管理器里定位文件，带 gid 时再让前端展开那条任务的详情。
///
/// 对齐上游主进程里那段 `application:reveal-in-folder`：`showItemInFolder(path)`
/// 配上 `sendCommandToAll('application:show-task-detail', { gid })`。
fn reveal_in_folder(app: &AppHandle, path: Option<String>, gid: Option<String>) {
    if let Some(path) = path {
        if let Err(err) = app.opener().reveal_item_in_dir(&path) {
            log::warn!("[Motr] 在文件管理器里定位 `{path}` 失败: {err}");
        }
    }

    if let Some(gid) = gid {
        dispatch(app, "application:show-task-detail", json!({ "gid": gid }));
    }
}

/// 下发一条前端命令（`Ipc.vue` 收到 `command` 事件后交给 CommandManager）。
///
/// 前端还没挂上监听就先攒着，等 [`ready`] 再补发 —— 冷启动就带链接时，
/// 前端必然还没就绪，直接 emit 会丢。
fn dispatch(app: &AppHandle, command: &str, arg: Value) {
    // 判断和"发送/入队"放在同一个锁里：分开做的话，恰好在这两步之间
    // 前端调 protocol_ready 就会把命令漏掉（进队列之后再也没人来取）
    with_state(app, |state| {
        if state.ready {
            menu::send_command_to_frontend(app, command, Some(&arg));
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
        menu::send_command_to_frontend(app, &command, Some(&arg));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(raw: &str) -> Url {
        Url::parse(raw).unwrap_or_else(|err| panic!("{raw} 解析失败: {err}"))
    }

    fn args(pairs: &[(&str, &str)]) -> Map<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
            .collect()
    }

    /// 两个开关与两个协议一一对应（magnet 在前，顺序固定）
    #[test]
    fn maps_protocol_switches() {
        let protocols = Protocols {
            magnet: true,
            thunder: false,
        };
        assert_eq!(
            wanted_schemes(&protocols),
            [("magnet", true), ("thunder", false)]
        );

        let protocols = Protocols {
            magnet: false,
            thunder: true,
        };
        assert_eq!(
            wanted_schemes(&protocols),
            [("magnet", false), ("thunder", true)]
        );
    }

    /// 资源链接原样交给「新建任务」。
    ///
    /// 特意盯住 thunder：它的 base64 就在 host 位置，[`Url`] 只要把它小写化
    /// 就等于废掉整条链接，所以这里断言前后完全一致。
    #[test]
    fn resource_links_become_new_task() {
        for raw in [
            "magnet:?xt=urn:btih:abcdef0123456789",
            "https://example.com/a.zip",
            "ftp://example.com/a.zip",
            "thunder://QUFodHRwOi8vZXhhbXBsZS5jb20vYS56aXBaWg==",
        ] {
            assert_eq!(action_of(&url(raw)), Action::NewTask(raw.to_string()));
        }
    }

    /// 自有协议：host -> 前端命令，query 解码成参数对象
    #[test]
    fn own_protocol_maps_host_to_command() {
        assert_eq!(
            action_of(&url("mo://new-task?uri=magnet%3A%3Fxt%3D1")),
            Action::Command {
                command: "application:new-task",
                args: args(&[("uri", "magnet:?xt=1")]),
            }
        );

        // 不带参数时是空对象（原项目 querystring.parse('') -> {}）
        assert_eq!(
            action_of(&url("motrix://about")),
            Action::Command {
                command: "application:about",
                args: Map::new(),
            }
        );

        // 两个 scheme 等价（原项目那两条 startsWith 分支走同一段逻辑）
        assert_eq!(action_of(&url("mo://preferences")), action_of(&url("motrix://preferences")));
    }

    /// 上游 8 条映射里唯一在主进程做的那条
    #[test]
    fn own_protocol_reveal_in_folder() {
        assert_eq!(
            action_of(&url("mo://reveal-in-folder?path=%2Ftmp%2Fa.zip&gid=abc")),
            Action::RevealInFolder {
                path: Some("/tmp/a.zip".to_string()),
                gid: Some("abc".to_string()),
            }
        );

        // 上游是 path / gid 各自独立判断的，只有一个也认
        assert_eq!(
            action_of(&url("mo://reveal-in-folder?gid=abc")),
            Action::RevealInFolder {
                path: None,
                gid: Some("abc".to_string()),
            }
        );
    }

    /// 认不出的 host、其它 scheme、普通命令行参数都要忽略（原项目同样直接 return）
    #[test]
    fn ignores_unknown_links() {
        for raw in [
            "mo://nope",
            "motrix://nope?x=1",
            "file:///tmp/a.zip",
            "https-not-really://example.com",
            "mailto:a@example.com",
        ] {
            assert_eq!(action_of(&url(raw)), Action::Ignore, "{raw}");
        }
    }

    /// 自有协议表：7 条发给前端 + 1 条在 Rust 侧处理（上游共 8 条）
    #[test]
    fn own_command_table_matches_upstream() {
        assert_eq!(OWN_SCHEMES, ["mo", "motrix"]);
        assert_eq!(OWN_COMMANDS.len() + 1, 8);

        for (host, command) in OWN_COMMANDS {
            assert_eq!(
                action_of(&url(&format!("mo://{host}"))),
                Action::Command {
                    command,
                    args: Map::new(),
                },
                "{host} 应映射到 {command}"
            );
        }
    }
}
