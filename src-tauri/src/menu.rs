use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use base64::prelude::{BASE64_STANDARD, Engine as _};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use tauri::{
    AppHandle, Emitter, Manager, Wry,
    menu::{IsMenuItem, Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    path::BaseDirectory,
};
use tauri_plugin_dialog::{DialogExt, FilePath};
use tauri_plugin_store::StoreExt;

/// 快捷键表（与语言无关，直接编进二进制）
const KEYMAP: &str = include_str!("../resources/menus/keymap.json");

/// 缺语言时回落的语言；它同时是菜单结构与英文文案的源头
const FALLBACK_LOCALE: &str = "en-US";

/// menus/<locale>/<platform>.json 的内容：
/// darwin / win32 / linux 是 `{ "menu": [...] }`，tray 是裸数组
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MenuFile {
    Wrapped { menu: Vec<MenuNode> },
    Bare(Vec<MenuNode>),
}

impl MenuFile {
    fn into_nodes(self) -> Vec<MenuNode> {
        match self {
            MenuFile::Wrapped { menu } => menu,
            MenuFile::Bare(nodes) => nodes,
        }
    }
}

/// 菜单项：字段与原项目模板一一对应，`text` 是当前语言的菜单文本
#[derive(Debug, Clone, Deserialize)]
pub struct MenuNode {
    /// 菜单项 id（原项目里是 i18n key，也当菜单项 id 用）
    #[serde(default)]
    pub id: Option<String>,
    /// 当前语言的菜单文本（由 scripts/gen-menu-locales.mjs 生成时写入）
    #[serde(default)]
    pub text: Option<String>,
    /// 目前只用到 "separator"
    #[serde(rename = "type", default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    /// 以下三个字段原项目用来标识点击后要发的命令，先留着以后再接功能
    #[serde(rename = "command-arg", default)]
    #[allow(dead_code)]
    pub command_arg: Option<serde_json::Value>,
    #[serde(rename = "command-before", default)]
    #[allow(dead_code)]
    pub command_before: Option<String>,
    #[serde(rename = "command-after", default)]
    #[allow(dead_code)]
    pub command_after: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub submenu: Option<Vec<MenuNode>>,
}

/// 有系统预定义项、能直接交给系统处理的 role
const SUPPORTED_ROLES: &[&str] = &[
    "undo",
    "redo",
    "cut",
    "copy",
    "paste",
    "selectall",
    "hide",
    "hideothers",
    "unhide",
    "quit",
    "minimize",
    "close",
    "zoom",
    "togglefullscreen",
    "front",
];

/// 菜单项点击后要做的事：直接对应菜单 JSON 里的字段
#[derive(Debug, Clone, Default, PartialEq)]
struct MenuAction {
    /// 菜单项的 command（可以为空，比如只写了 role 的项）
    command: Option<String>,
    command_arg: Option<Value>,
    command_before: Option<String>,
    command_after: Option<String>,
    /// 系统 role；Tauri 没有预定义项的那几个（toggledevtools）由 Rust 自己实现
    role: Option<String>,
}

/// 菜单项 id -> 点击动作；每次构建菜单时整体重填
///
/// 由 lib.rs 注册成 Tauri 托管状态（`.manage(MenuActions::default())`）
#[derive(Default)]
pub struct MenuActions(Mutex<HashMap<String, MenuAction>>);

/// 递归收集菜单项：带 command 的，以及 role 是 Tauri 缺的预定义项的（toggledevtools）
fn collect_actions(nodes: &[MenuNode], actions: &mut HashMap<String, MenuAction>) {
    for node in nodes {
        let local_role = node
            .role
            .as_deref()
            .is_some_and(|role| local_role_step(role).is_some());

        if let Some(id) = node
            .id
            .as_ref()
            .filter(|_| node.command.is_some() || local_role)
        {
            actions.insert(
                id.clone(),
                MenuAction {
                    command: node.command.clone(),
                    command_arg: node.command_arg.clone(),
                    command_before: node.command_before.clone(),
                    command_after: node.command_after.clone(),
                    role: node.role.clone(),
                },
            );
        }
        if let Some(children) = &node.submenu {
            collect_actions(children, actions);
        }
    }
}

/// 启动时按「当前平台 + 当前语言」构建 App 菜单与托盘菜单。
///
/// 菜单读不出来时只记日志，不让 App 起不来。
pub fn init(app: &AppHandle) {
    if let Err(err) = apply(app, &config_locale(app)) {
        log::error!("[Motr] 构建菜单失败: {err}");
    }
}

/// 菜单点击：应用菜单和托盘菜单都走这里。
///
/// 与原项目 `src/main/utils/menu.js` 的 handleCommand 等价：
/// 依次走 command-before -> command -> command-after，每一步先看 Rust 认不认
/// （认得的自己处理，比如显示窗口、选种子文件），认不出才下发给前端。
/// 系统 role 组成的项（复制/粘贴/退出…）由系统自己处理，不会走到这里。
pub fn on_menu_event(app: &AppHandle, id: &str) {
    match menu_actions(app).get(id).cloned() {
        Some(action) => run_action(app, &action),
        None => log::info!("[Motr] 菜单项 `{id}` 没有配 command，暂不处理"),
    }
}

/// 取菜单动作表
///
/// 数据只是 `id -> 动作` 的映射，读和整体替换都不会留下"改一半"的状态，
/// 所以即使锁中毒（有过持锁 panic）也照常取用：`PoisonError::into_inner` 拿到数据继续走
fn menu_actions(app: &AppHandle) -> MutexGuard<'_, HashMap<String, MenuAction>> {
    // State 是临时值，用 inner() 取到绑定在 AppHandle 上的引用，
    // 这样返回的 guard 生命周期才够长
    app.state::<MenuActions>()
        .inner()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// 按原项目顺序执行菜单项的动作
fn run_action(app: &AppHandle, action: &MenuAction) {
    for step in plan(action) {
        run_step(app, &step);
    }
}

/// 菜单项点击后要做的一步
#[derive(Debug, Clone, PartialEq)]
enum MenuStep {
    /// 显示并聚焦窗口（`application:show[?page=xxx]`）
    Show(Option<String>),
    /// 选种子文件（`application:open-file`）
    OpenFile,
    /// 用系统默认浏览器打开链接（帮助菜单里的几项）
    OpenUrl { url: &'static str },
    /// 切换开发者工具（`help.toggle-dev-tools`，Tauri 没有这个 role）
    ToggleDevTools,
    /// 退出应用（`application:quit`，托盘菜单和 Win/Linux 文件菜单里的「退出」）
    Quit,
    /// 下发给前端执行
    Frontend { command: String, arg: Option<Value> },
}

/// 菜单项的动作 -> 执行步骤（纯函数，方便测试）
///
/// 与原项目 `handleCommand` 一致：command-before -> command -> command-after；
/// **每一步**都先看 Rust 认不认这个命令（原项目 `sendCommandToAll` 也是先看本地有没有监听），
/// 认不出才下发给前端。
fn plan(action: &MenuAction) -> Vec<MenuStep> {
    fn step(spec: &str, arg: Option<Value>) -> MenuStep {
        let (command, params) = split_command(spec);

        match command {
            // 原项目 application:show 的 page 参数只用来选窗口（同时负责把窗口显示出来）；
            // 本项目只有一个主窗口，所以就是"显示并聚焦"
            "application:show" => MenuStep::Show(
                params
                    .get("page")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            ),
            "application:open-file" => MenuStep::OpenFile,
            "application:quit" => MenuStep::Quit,
            // 帮助菜单里的链接，原项目是主进程的 help:* 处理
            _ => match help_url(command) {
                Some(url) => MenuStep::OpenUrl { url },
                None => MenuStep::Frontend {
                    command: command.to_string(),
                    // command-before/after 里的 ?参数按原项目那样作为唯一参数下发
                    arg: arg.or_else(|| (!params.is_empty()).then(|| Value::Object(params))),
                },
            },
        }
    }

    let mut steps = Vec::new();
    if let Some(before) = action.command_before.as_deref() {
        steps.push(step(before, None));
    }

    match (action.command.as_deref(), action.role.as_deref()) {
        (Some(command), _) => steps.push(step(command, action.command_arg.clone())),
        // 没有 command：只有 Tauri 缺的 role 需要自己实现（目前就 toggledevtools）
        (None, Some(role)) => {
            if let Some(local) = local_role_step(role) {
                steps.push(local);
            }
        }
        (None, None) => {}
    }

    if let Some(after) = action.command_after.as_deref() {
        steps.push(step(after, None));
    }
    steps
}

/// 帮助菜单里的链接（对齐原项目 `Application.js` 的 `help:*` 处理）
fn help_url(command: &str) -> Option<&'static str> {
    match command {
        "help:official-website" => Some("https://motrix.app/"),
        "help:manual" => Some("https://motrix.app/manual"),
        "help:release-notes" => Some("https://motrix.app/release"),
        "help:report-problem" => Some("https://motrix.app/report"),
        _ => None,
    }
}

/// Tauri 没有预定义项、需要自己实现的 role
fn local_role_step(role: &str) -> Option<MenuStep> {
    match role {
        "toggledevtools" => Some(MenuStep::ToggleDevTools),
        _ => None,
    }
}

/// 执行一步
fn run_step(app: &AppHandle, step: &MenuStep) {
    match step {
        MenuStep::Show(page) => {
            crate::window::show_main_window(app);
            log::info!("[Motr] application:show (page: {page:?})");
        }
        MenuStep::OpenFile => pick_torrent_file(app),
        MenuStep::OpenUrl { url } => match tauri_plugin_opener::open_url(*url, None::<&str>) {
            Ok(()) => log::info!("[Motr] opened {url}"),
            Err(err) => log::warn!("[Motr] 打开链接失败 {url}: {err}"),
        },
        MenuStep::ToggleDevTools => toggle_dev_tools(app),
        MenuStep::Quit => quit_app(app),
        MenuStep::Frontend { command, arg } => send_command_to_frontend(app, command, arg.as_ref()),
    }
}

/// 退出应用（等价原项目 `Application.quit`：先停引擎，再 `app.exit()`）
fn quit_app(app: &AppHandle) {
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
fn toggle_dev_tools(app: &AppHandle) {
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

/// 「打开种子文件…」：等价原项目 `Application.handleFile`
///
/// 选文件 -> 读出来 base64 -> 交给前端走「新建 BT 任务」；
/// 窗口由菜单里的 `command-before: application:show?page=index` 负责显示。
fn pick_torrent_file(app: &AppHandle) {
    let app = app.clone();

    app.dialog()
        .file()
        .add_filter("Torrent", &["torrent"])
        .pick_file(move |file| {
            let Some(file) = file else {
                log::info!("[Motr] 打开种子文件：用户取消");
                return;
            };

            let path = match file {
                FilePath::Path(path) => path,
                // 桌面端不会出现，移动端是 content:// 这类 URL
                FilePath::Url(url) => {
                    log::warn!("[Motr] 打开种子文件：不支持的路径 {url}");
                    return;
                }
            };

            match torrent_payload(&path) {
                Ok(payload) => send_command_to_frontend(
                    &app,
                    "application:new-bt-task-with-file",
                    Some(&payload),
                ),
                Err(err) => log::warn!("[Motr] 打开种子文件失败: {err}"),
            }
        });
}

/// 读种子文件 -> 前端要的 `{ name, dataURL }`
///
/// 与原项目一致：dataURL 是**裸 base64**（前端用 `base64StringToBlob` 还原成 File）
///
/// 两个调用方：菜单里的「打开种子文件」，以及系统把 `.torrent` 递进来时
/// （文件关联，见 `protocol::open_torrent_file`）
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

/// 把命令下发给前端：前端 `Ipc.vue` 收到后交给 CommandManager 执行
///
/// 菜单点击之外，协议链接（protocol.rs）也走这里 —— 前端只需要认一种形状
pub(crate) fn send_command_to_frontend(app: &AppHandle, command: &str, arg: Option<&Value>) {
    // 前端是 commands.execute(command, ...args)，所以参数要包成数组
    let args = arg.map_or_else(|| json!([]), |value| json!([value]));
    let payload = json!({ "command": command, "args": args });

    match app.emit("command", payload) {
        Ok(()) => log::info!("[Motr] menu command -> {command} (args: {args})"),
        Err(err) => log::error!("[Motr] 下发菜单命令 `{command}` 失败: {err}"),
    }
}

/// 把 `application:show?page=index` 拆成命令与参数
fn split_command(spec: &str) -> (&str, Map<String, Value>) {
    match spec.split_once('?') {
        Some((command, query)) => (command, parse_query(query)),
        None => (spec, Map::new()),
    }
}

/// 极简 query 解析（原项目用 querystring.parse）：
/// `page=index&foo=bar` -> {"page":"index","foo":"bar"}
fn parse_query(query: &str) -> Map<String, Value> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (key.to_string(), Value::String(value.to_string())),
            None => (pair.to_string(), Value::Bool(true)),
        })
        .collect()
}

/// 按语言重建 App 菜单与托盘菜单（命令在 commands.rs）
pub(crate) fn apply(app: &AppHandle, locale: &str) -> Result<(), String> {
    let dir = menus_dir(app)?;
    if !dir.is_dir() {
        return Err(format!("菜单资源目录不存在: {}", dir.display()));
    }

    let locale = resolve_locale(locale, &available_locales(&dir));
    let platform = platform_file();

    let nodes = load_nodes(&dir, &locale, platform)?;
    let mut actions = HashMap::new();
    collect_actions(&nodes, &mut actions);

    let app_menu = build_menu(app, &nodes, MenuKind::App)?;
    app.set_menu(app_menu)
        .map_err(|err| format!("设置 App 菜单失败: {err}"))?;

    // 托盘（右键）菜单（不带快捷键提示）
    if let Some(tray) = app.tray_by_id(crate::tray::TRAY_ID) {
        let nodes = load_nodes(&dir, &locale, "tray")?;
        collect_actions(&nodes, &mut actions);
        let tray_menu = build_menu(app, &nodes, MenuKind::Tray)?;
        tray.set_menu(Some(tray_menu))
            .map_err(|err| format!("设置托盘菜单失败: {err}"))?;
    }

    // 点击时要执行什么，按 id 存起来（App 菜单与托盘菜单里的同名项命令是一致的）
    *menu_actions(app) = actions;

    log::info!("[Motr] menus built (locale: {locale}, platform: {platform})");
    Ok(())
}

/// 菜单资源目录：src-tauri/resources/menus（打包后是 app 的 Resources 目录）
fn menus_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .resolve("resources/menus", BaseDirectory::Resource)
        .map_err(|err| format!("解析 resources/menus 失败: {err}"))
}

/// 有哪些语言：menus 下带菜单文件的子目录（keymap.json 之类的散文件不算）
fn available_locales(dir: &Path) -> Vec<String> {
    const PLATFORM_FILES: &[&str] = &["darwin.json", "win32.json", "linux.json", "tray.json"];

    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut locales: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter(|entry| {
            PLATFORM_FILES
                .iter()
                .any(|file| entry.path().join(file).is_file())
        })
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    locales.sort();
    locales
}

/// 读取某语言的菜单文件
fn load_nodes(dir: &Path, locale: &str, platform: &str) -> Result<Vec<MenuNode>, String> {
    let file = dir.join(locale).join(format!("{platform}.json"));
    let content = std::fs::read_to_string(&file)
        .map_err(|err| format!("读取 {} 失败: {err}", file.display()))?;
    let parsed: MenuFile = serde_json::from_str(&content)
        .map_err(|err| format!("解析 {} 失败: {err}", file.display()))?;
    Ok(parsed.into_nodes())
}

/// 把各种形态的 locale 归一到已有的语言目录：
/// user.json 里通常是 "zh-CN"，系统语言可能是 "zh-Hans-CN" / "en_US" 之类
fn resolve_locale(locale: &str, available: &[String]) -> String {
    let normalized = locale.replace('_', "-");

    if let Some(exact) = available
        .iter()
        .find(|item| item.eq_ignore_ascii_case(&normalized))
    {
        return exact.clone();
    }

    // 按语言子标签兜底：zh-Hans-CN -> zh-CN，en -> en-US
    let language = normalized.split('-').next().unwrap_or_default();
    if let Some(matched) = available.iter().find(|item| {
        item.split('-')
            .next()
            .is_some_and(|part| part.eq_ignore_ascii_case(language))
    }) {
        return matched.clone();
    }

    FALLBACK_LOCALE.to_string()
}

/// 当前语言：存在 user.json 的 `locale` 里（UserConfig::locale），读不到就用系统语言
fn config_locale(app: &AppHandle) -> String {
    app.store("user.json")
        .ok()
        .and_then(|store| store.get("locale"))
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| tauri_plugin_os::locale().unwrap_or_else(|| FALLBACK_LOCALE.to_string()))
}

/// 菜单的用途：托盘右键菜单不显示快捷键
#[derive(Debug, Clone, Copy, PartialEq)]
enum MenuKind {
    /// App 菜单（macOS 菜单栏 / Win·Linux 窗口菜单）
    App,
    /// 托盘右键菜单
    Tray,
}

impl MenuKind {
    /// 要不要在菜单项后面显示快捷键提示
    ///
    /// 托盘菜单不显示：原生托盘菜单上挂 ⌘N 这类提示没有意义（点它也不会按快捷键生效），
    /// 快捷键本身由 App 菜单注册，两边是同一批命令，功能不受影响
    fn shows_shortcuts(self) -> bool {
        matches!(self, MenuKind::App)
    }
}

fn build_menu(app: &AppHandle, nodes: &[MenuNode], kind: MenuKind) -> Result<Menu<Wry>, String> {
    let items = build_items(app, nodes, kind)?;
    MenuBuilder::new(app)
        .items(&as_refs(&items))
        .build()
        .map_err(|err| format!("创建菜单失败: {err}"))
}

fn build_items(
    app: &AppHandle,
    nodes: &[MenuNode],
    kind: MenuKind,
) -> Result<Vec<Box<dyn IsMenuItem<Wry>>>, String> {
    let mut result: Vec<Box<dyn IsMenuItem<Wry>>> = Vec::with_capacity(nodes.len());
    for node in nodes {
        result.push(build_item(app, node, kind)?);
    }
    Ok(result)
}

fn build_item(
    app: &AppHandle,
    node: &MenuNode,
    kind: MenuKind,
) -> Result<Box<dyn IsMenuItem<Wry>>, String> {
    if node.kind.as_deref() == Some("separator") {
        return PredefinedMenuItem::separator(app)
            .map(|item| Box::new(item) as Box<dyn IsMenuItem<Wry>>)
            .map_err(|err| format!("创建分隔线失败: {err}"));
    }

    let text = node_text(node);

    // 有子菜单：建 Submenu
    if let Some(children) = &node.submenu {
        let items = build_items(app, children, kind)?;
        let submenu = SubmenuBuilder::new(app, &text)
            .items(&as_refs(&items))
            .build()
            .map_err(|err| format!("创建子菜单「{text}」失败: {err}"))?;
        return Ok(Box::new(submenu));
    }

    // 系统 role 优先交给系统
    if let Some(role) = node.role.as_deref() {
        if let Some(item) = predefined(app, role, Some(&text))? {
            return Ok(Box::new(item));
        }
        log::debug!("[Motr] role `{role}` 没有系统预定义项，先按普通菜单项显示");
    }

    // 普通菜单项：id 就是命令名（点击后的分派见 plan/run_step）
    let id = node.id.clone().unwrap_or_else(|| text.clone());
    let mut builder = MenuItemBuilder::with_id(id, &text);
    if let Some(accelerator) = accelerator_of(kind, node) {
        builder = builder.accelerator(accelerator);
    }
    builder
        .build(app)
        .map(|item| Box::new(item) as Box<dyn IsMenuItem<Wry>>)
        .map_err(|err| format!("创建菜单项「{text}」失败: {err}"))
}

/// 菜单项要显示的快捷键（按 keymap 里配置的命令 → 键位反查）
fn accelerator_of(kind: MenuKind, node: &MenuNode) -> Option<String> {
    if !kind.shows_shortcuts() {
        return None;
    }
    node.command.as_deref().and_then(accelerator_for_command)
}

/// 菜单文本：用生成时写入的 text，万一没有就退回 id
fn node_text(node: &MenuNode) -> String {
    node.text
        .clone()
        .or_else(|| node.id.clone())
        .unwrap_or_default()
}

/// 当前平台的菜单文件名
fn platform_file() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    }
}

/// keymap.json 的内容（keystroke -> command）
fn keymap() -> &'static HashMap<String, String> {
    static KEYMAP_MAP: OnceLock<HashMap<String, String>> = OnceLock::new();
    KEYMAP_MAP.get_or_init(|| serde_json::from_str(KEYMAP).expect("keymap.json 不是合法 JSON"))
}

/// 命令 -> muda 语法的快捷键（command -> "CmdOrCtrl+Shift+N"）
fn accelerator_for_command(command: &str) -> Option<String> {
    static BY_COMMAND: OnceLock<HashMap<String, String>> = OnceLock::new();
    let by_command = BY_COMMAND.get_or_init(|| {
        keymap()
            .iter()
            .map(|(keystroke, command)| (command.clone(), muda_accelerator(keystroke)))
            .collect()
    });
    by_command.get(command).cloned()
}

/// keymap 写法（cmdctrl-shift-n）-> muda 写法（CmdOrCtrl+Shift+N）
fn muda_accelerator(keystroke: &str) -> String {
    let mut tokens: Vec<&str> = keystroke.split('-').collect();
    let key = tokens.pop().unwrap_or_default().to_uppercase();

    let mut parts: Vec<String> = tokens
        .iter()
        .map(|token| {
            if token.eq_ignore_ascii_case("cmdctrl") {
                return "CmdOrCtrl".to_string();
            }
            let mut chars = token.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    parts.push(key);
    parts.join("+")
}

/// role -> 系统预定义项；Tauri 没有对应项的（delete / reload / toggledevtools）返回 None
fn predefined(
    app: &AppHandle,
    role: &str,
    text: Option<&str>,
) -> Result<Option<PredefinedMenuItem<Wry>>, String> {
    if !SUPPORTED_ROLES.contains(&role) {
        return Ok(None);
    }

    let item = match role {
        "undo" => PredefinedMenuItem::undo(app, text),
        "redo" => PredefinedMenuItem::redo(app, text),
        "cut" => PredefinedMenuItem::cut(app, text),
        "copy" => PredefinedMenuItem::copy(app, text),
        "paste" => PredefinedMenuItem::paste(app, text),
        "selectall" => PredefinedMenuItem::select_all(app, text),
        "hide" => PredefinedMenuItem::hide(app, text),
        "hideothers" => PredefinedMenuItem::hide_others(app, text),
        "unhide" => PredefinedMenuItem::show_all(app, text),
        "quit" => PredefinedMenuItem::quit(app, text),
        "minimize" => PredefinedMenuItem::minimize(app, text),
        "close" => PredefinedMenuItem::close_window(app, text),
        "zoom" => PredefinedMenuItem::maximize(app, text),
        "togglefullscreen" => PredefinedMenuItem::fullscreen(app, text),
        "front" => PredefinedMenuItem::bring_all_to_front(app, text),
        _ => return Ok(None),
    };
    item.map(Some)
        .map_err(|err| format!("创建系统菜单项 {role} 失败: {err}"))
}

/// `[Box<dyn IsMenuItem>]` -> `[&dyn IsMenuItem]`，MenuBuilder/SubmenuBuilder 需要后者
fn as_refs(items: &[Box<dyn IsMenuItem<Wry>>]) -> Vec<&dyn IsMenuItem<Wry>> {
    items.iter().map(|item| item.as_ref()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    const MENU_FILES: &[&str] = &["darwin.json", "win32.json", "linux.json", "tray.json"];
    /// 源头语言：其他语言的结构与它一致
    const SOURCE_LOCALE: &str = "en-US";

    fn menus_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/menus")
    }

    fn read_json(path: &Path) -> Value {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|err| panic!("读取 {} 失败: {err}", path.display()));
        serde_json::from_str(&content)
            .unwrap_or_else(|err| panic!("{} 不是合法 JSON: {err}", path.display()))
    }

    fn nodes_of(value: &Value) -> &Vec<Value> {
        value
            .as_array()
            .or_else(|| value.get("menu").and_then(Value::as_array))
            .unwrap_or_else(|| panic!("{value} 既不是数组也没有 menu 字段"))
    }

    fn locales() -> Vec<String> {
        available_locales(&menus_root())
    }

    /// 递归收集每个节点的「结构指纹」：id + type + role + command
    fn structure_fingerprint(node: &Value, result: &mut Vec<String>) {
        let mut parts = Vec::new();
        for key in [
            "id",
            "type",
            "role",
            "command",
            "command-arg",
            "command-before",
            "command-after",
        ] {
            if let Some(value) = node.get(key) {
                parts.push(format!("{key}={value}"));
            }
        }
        result.push(parts.join(","));
        if let Some(children) = node.get("submenu").and_then(Value::as_array) {
            // 用缩进表示层级，免得扁平化后丢掉父子关系
            let mut nested = Vec::new();
            children
                .iter()
                .for_each(|child| structure_fingerprint(child, &mut nested));
            result.extend(nested.into_iter().map(|line| format!("  {line}")));
        }
    }

    fn fingerprint(locale: &str, file: &str) -> Vec<String> {
        let value = read_json(&menus_root().join(locale).join(file));
        let mut result = Vec::new();
        nodes_of(&value)
            .iter()
            .for_each(|node| structure_fingerprint(node, &mut result));
        result
    }

    /// 每种语言、每个平台的文件都在，且能解析（Rust 侧是从磁盘读的，这里先兜住）
    #[test]
    fn menu_files_exist_and_parse() {
        let dir = menus_root();
        assert!(dir.is_dir(), "缺少 {}", dir.display());

        let locales = locales();
        assert!(
            locales.contains(&SOURCE_LOCALE.to_string()),
            "缺少 {SOURCE_LOCALE} 语言目录"
        );
        assert!(locales.len() >= 26, "语言目录只有 {} 个", locales.len());

        for locale in &locales {
            for file in MENU_FILES {
                let path = dir.join(locale).join(file);
                let value = read_json(&path);
                let content_type = if *file == "tray.json" {
                    value.is_array()
                } else {
                    value.get("menu").is_some_and(Value::is_array)
                };
                assert!(content_type, "{} 的结构不对", path.display());
            }
        }
    }

    /// 各语言的菜单结构必须和源头语言完全一致（只有 text 不同）
    #[test]
    fn structures_match_source_locale() {
        for file in MENU_FILES {
            let expected = fingerprint(SOURCE_LOCALE, file);
            for locale in locales().iter().filter(|item| *item != SOURCE_LOCALE) {
                assert_eq!(
                    fingerprint(locale, file),
                    expected,
                    "{locale}/{file} 的结构和源头不一致"
                );
            }
        }
    }

    /// 每一条菜单项都要有非空 text（生成时写入当前语言的文本）
    #[test]
    fn every_item_has_a_text() {
        fn check(node: &Value, file: &str) {
            if node.get("type").and_then(Value::as_str) != Some("separator") {
                let text = node.get("text").and_then(Value::as_str).unwrap_or_default();
                assert!(!text.is_empty(), "{file} 里有菜单项没有 text: {node}");
            }
            if let Some(children) = node.get("submenu").and_then(Value::as_array) {
                children.iter().for_each(|child| check(child, file));
            }
        }

        for locale in locales() {
            for file in MENU_FILES {
                let value = read_json(&menus_root().join(&locale).join(file));
                nodes_of(&value)
                    .iter()
                    .for_each(|node| check(node, &format!("{locale}/{file}")));
            }
        }
    }

    /// 文案确实按语言分目录生效了（不是所有语言都照抄英文）
    #[test]
    fn texts_are_translated() {
        let source = read_json(&menus_root().join(SOURCE_LOCALE).join("darwin.json"));
        let chinese = read_json(&menus_root().join("zh-CN").join("darwin.json"));

        let find = |value: &Value, id: &str| -> String {
            fn walk(node: &Value, id: &str) -> Option<String> {
                if node.get("id").and_then(Value::as_str) == Some(id) {
                    return node.get("text").and_then(Value::as_str).map(str::to_string);
                }
                node.get("submenu")
                    .and_then(Value::as_array)
                    .and_then(|children| children.iter().find_map(|child| walk(child, id)))
            }
            nodes_of(value)
                .iter()
                .find_map(|node| walk(node, id))
                .unwrap_or_else(|| panic!("找不到 {id}"))
        };

        assert_eq!(find(&source, "menu.task"), "Task");
        assert_eq!(find(&chinese, "menu.task"), "任务");
        assert_eq!(find(&chinese, "task.new-task"), "新建任务");
        assert_ne!(find(&source, "app.quit"), find(&chinese, "app.quit"));
    }

    /// 语言目录解析：user.json 的 "zh-CN" / 系统语言 "zh-Hans-CN" / "en_US" 都要落到已有语言
    #[test]
    fn locale_resolution() {
        let available = locales();

        assert_eq!(resolve_locale("zh-CN", &available), "zh-CN");
        assert_eq!(resolve_locale("en-US", &available), "en-US");
        assert_eq!(resolve_locale("zh-Hans-CN", &available), "zh-CN");
        assert_eq!(resolve_locale("en_US", &available), "en-US");
        assert_eq!(resolve_locale("en", &available), "en-US");
        assert_eq!(resolve_locale("ja-JP", &available), "ja");
        assert_eq!(resolve_locale("pt-BR", &available), "pt-BR");
        assert_eq!(resolve_locale("", &available), "en-US");
        assert_eq!(resolve_locale("xx-YY", &available), "en-US");
    }

    /// 模板里用到的 role 要么有系统预定义项，要么是明确要退化成普通项的
    #[test]
    fn template_roles_are_all_accounted_for() {
        // Tauri 没有对应预定义项（或不是菜单项本身的 role），先按普通菜单项显示
        const PLAIN_ROLES: &[&str] = &["delete", "toggledevtools", "window", "help"];

        fn collect(node: &Value, roles: &mut Vec<String>) {
            if let Some(role) = node.get("role").and_then(Value::as_str) {
                roles.push(role.to_string());
            }
            node.get("submenu")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .for_each(|child| collect(child, roles));
        }

        for locale in locales() {
            for file in MENU_FILES {
                let value = read_json(&menus_root().join(&locale).join(file));
                let mut roles = Vec::new();
                nodes_of(&value)
                    .iter()
                    .for_each(|node| collect(node, &mut roles));
                for role in roles {
                    assert!(
                        SUPPORTED_ROLES.contains(&role.as_str())
                            || PLAIN_ROLES.contains(&role.as_str()),
                        "{locale}/{file} 里的 role `{role}` 没有处理"
                    );
                }
            }
        }
    }

    /// 与原项目一致：子菜单结构与 mac 任务菜单环节
    #[test]
    fn platform_templates_match_original() {
        let submenu_ids = |file: &str| -> Vec<String> {
            let value = read_json(&menus_root().join(SOURCE_LOCALE).join(file));
            nodes_of(&value)
                .iter()
                .map(|item| {
                    item.get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect()
        };

        assert_eq!(
            submenu_ids("darwin.json"),
            vec![
                "menu.app",
                "menu.task",
                "menu.edit",
                "menu.window",
                "menu.help"
            ]
        );
        for file in ["win32.json", "linux.json"] {
            assert_eq!(
                submenu_ids(file),
                vec![
                    "menu.file",
                    "menu.task",
                    "menu.edit",
                    "menu.window",
                    "menu.help"
                ],
                "{file} 的子菜单结构应与原项目一致"
            );
        }

        let tray = read_json(&menus_root().join(SOURCE_LOCALE).join("tray.json"));
        assert_eq!(tray.as_array().map(Vec::len), Some(11));

        let darwin = read_json(&menus_root().join(SOURCE_LOCALE).join("darwin.json"));
        let task_items = nodes_of(&darwin)[1]
            .get("submenu")
            .and_then(Value::as_array)
            .unwrap();
        let commands: Vec<&str> = task_items
            .iter()
            .filter_map(|item| item.get("command").and_then(Value::as_str))
            .collect();
        assert_eq!(commands.len(), 12);
        assert_eq!(commands[0], "application:new-task");
        let separators = task_items
            .iter()
            .filter(|item| item.get("type").and_then(Value::as_str) == Some("separator"))
            .count();
        assert_eq!(separators, 2);
    }

    /// 快捷键：keymap 写法转 muda 写法，并与原项目一致
    #[test]
    fn accelerators_match_original_keymap() {
        assert_eq!(muda_accelerator("cmdctrl-n"), "CmdOrCtrl+N");
        assert_eq!(muda_accelerator("cmdctrl-shift-n"), "CmdOrCtrl+Shift+N");
        assert_eq!(muda_accelerator("ctrl-shift-a"), "Ctrl+Shift+A");
        assert_eq!(muda_accelerator("cmdctrl-,"), "CmdOrCtrl+,");

        assert_eq!(
            accelerator_for_command("application:new-task").as_deref(),
            Some("CmdOrCtrl+N")
        );
        assert_eq!(
            accelerator_for_command("application:preferences").as_deref(),
            Some("CmdOrCtrl+,")
        );
        // 没配快捷键的命令不该乱给
        assert_eq!(accelerator_for_command("application:delete-task"), None);
    }

    /// muda 语法白名单：修饰键和按键得是它认的写法（写错会静默丢弃）
    #[test]
    fn accelerators_are_valid_for_muda() {
        const MODIFIERS: &[&str] = &["CmdOrCtrl", "Cmd", "Ctrl", "Shift", "Alt", "Super", "Meta"];

        for (keystroke, command) in keymap() {
            let accelerator = accelerator_for_command(command).unwrap_or_default();
            assert!(!accelerator.is_empty(), "{keystroke} 转出来是空的");

            let tokens: Vec<&str> = accelerator.split('+').collect();
            let (modifiers, key) = tokens.split_at(tokens.len() - 1);
            assert!(!modifiers.is_empty(), "{accelerator} 缺少修饰键");
            for modifier in modifiers {
                assert!(
                    MODIFIERS.contains(modifier),
                    "{accelerator}: 不认识的修饰键 {modifier}"
                );
            }
            let key = key[0];
            assert!(
                key.len() == 1 || key == "Comma" || key == "Plus",
                "{accelerator}: 不确定 muda 是否支持按键 {key}"
            );
        }
    }

    /// 从磁盘读某个语言/平台文件的菜单项（tray 是裸数组，平台文件是 { "menu": [...] }）
    fn read_nodes(locale: &str, file: &str) -> Vec<MenuNode> {
        let value = read_json(&menus_root().join(locale).join(file));
        let parsed: MenuFile = serde_json::from_value(value).expect("菜单文件解析失败");
        parsed.into_nodes()
    }

    /// `application:show?page=index` 要能拆成命令 + 参数（原项目用 querystring.parse）
    #[test]
    fn command_spec_is_split() {
        let (command, params) = split_command("application:show?page=index");
        assert_eq!(command, "application:show");
        assert_eq!(params.get("page"), Some(&json!("index")));

        let (command, params) = split_command("application:new-task");
        assert_eq!(command, "application:new-task");
        assert!(params.is_empty());

        let (command, params) = split_command("application:show?a=1&b=2&flag");
        assert_eq!(command, "application:show");
        assert_eq!(params.get("a"), Some(&json!("1")));
        assert_eq!(params.get("b"), Some(&json!("2")));
        assert_eq!(params.get("flag"), Some(&json!(true)));
    }

    /// 新增任务：点菜单（或按 Cmd/Ctrl+N）后要下发给前端 application:new-task，
    /// 并把窗口显示出来（原项目模板里的 command-after）
    #[test]
    fn new_task_menu_action() {
        for file in MENU_FILES {
            let mut actions = HashMap::new();
            collect_actions(&read_nodes(SOURCE_LOCALE, file), &mut actions);

            let action = actions
                .get("task.new-task")
                .unwrap_or_else(|| panic!("{file} 里没有 task.new-task"));

            assert_eq!(action.command.as_deref(), Some("application:new-task"));
            assert_eq!(action.command_arg, None);
            assert_eq!(action.command_before, None);
            assert_eq!(
                action.command_after.as_deref(),
                Some("application:show?page=index")
            );
            // 前端 CommandManager 里注册过这个命令
            assert_eq!(
                action
                    .command
                    .as_deref()
                    .and_then(accelerator_for_command)
                    .as_deref(),
                Some("CmdOrCtrl+N")
            );
        }
    }

    /// 带 command 的菜单项都要进动作表，command-arg 也要原样带过去
    #[test]
    fn all_commands_are_collected() {
        let mut actions = HashMap::new();
        for file in MENU_FILES {
            collect_actions(&read_nodes(SOURCE_LOCALE, file), &mut actions);
        }

        for id in [
            "task.new-task",
            "task.open-file",
            "app.preferences",
            "app.show",
            "help.manual",
        ] {
            assert!(actions.contains_key(id), "动作表里缺少 {id}");
        }

        // 文件里每个带 command 的项都必须进动作表（用原始 JSON 再走一遍，独立于 collect_actions）
        fn assert_collected(value: &Value, actions: &HashMap<String, MenuAction>) {
            match value {
                Value::Array(items) => {
                    items
                        .iter()
                        .for_each(|item| assert_collected(item, actions));
                }
                Value::Object(map) => {
                    if let Some(command) = map.get("command").and_then(Value::as_str) {
                        let id = map
                            .get("id")
                            .and_then(Value::as_str)
                            .expect("带 command 的项要有 id");
                        assert_eq!(
                            actions.get(id).and_then(|action| action.command.as_deref()),
                            Some(command),
                            "{id} 没进动作表"
                        );
                    }
                    if let Some(children) = map.get("submenu") {
                        assert_collected(children, actions);
                    }
                }
                _ => {}
            }
        }
        for file in MENU_FILES {
            assert_collected(
                &read_json(&menus_root().join(SOURCE_LOCALE).join(file)),
                &actions,
            );
        }

        assert_eq!(
            actions
                .get("task.new-bt-task")
                .map(|action| action.command_arg.clone()),
            Some(Some(json!({ "type": "torrent" })))
        );
        assert_eq!(
            actions
                .get("task.open-file")
                .and_then(|action| action.command_before.clone()),
            Some("application:show?page=index".to_string()),
            "打开种子文件要先显示窗口"
        );
    }

    /// 递归找菜单项
    fn find_node<'a>(nodes: &'a [MenuNode], id: &str) -> Option<&'a MenuNode> {
        for node in nodes {
            if node.id.as_deref() == Some(id) {
                return Some(node);
            }
            if let Some(children) = &node.submenu {
                if let Some(found) = find_node(children, id) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// 从真实菜单文件里取某项的动作
    fn action_of(locale: &str, file: &str, id: &str) -> MenuAction {
        let mut actions = HashMap::new();
        collect_actions(&read_nodes(locale, file), &mut actions);
        actions
            .remove(id)
            .unwrap_or_else(|| panic!("{file} 里没有 {id}"))
    }

    /// 每项动作的执行计划：本地处理的自己处理，其余下发给前端
    #[test]
    fn plan_splits_local_and_frontend() {
        let with_command = |command: &str| MenuAction {
            command: Some(command.to_string()),
            ..Default::default()
        };

        // 打开种子文件：先显示窗口，再由 Rust 选文件（命令本身**不能**下发给前端）
        assert_eq!(
            plan(&MenuAction {
                command_before: Some("application:show?page=index".to_string()),
                ..with_command("application:open-file")
            }),
            vec![
                MenuStep::Show(Some("index".to_string())),
                MenuStep::OpenFile,
            ]
        );

        // 新增任务：命令下发给前端，之后再显示窗口（当前的 command-after）
        assert_eq!(
            plan(&MenuAction {
                command_after: Some("application:show?page=index".to_string()),
                ..with_command("application:new-task")
            }),
            vec![
                MenuStep::Frontend {
                    command: "application:new-task".to_string(),
                    arg: None,
                },
                MenuStep::Show(Some("index".to_string())),
            ]
        );

        // command-arg 原样作为唯一参数下发给前端
        assert_eq!(
            plan(&MenuAction {
                command_arg: Some(json!({ "type": "torrent" })),
                ..with_command("application:new-bt-task")
            }),
            vec![MenuStep::Frontend {
                command: "application:new-bt-task".to_string(),
                arg: Some(json!({ "type": "torrent" })),
            }]
        );

        // 纯显示窗口
        assert_eq!(
            plan(&with_command("application:show")),
            vec![MenuStep::Show(None)]
        );

        // 帮助菜单里的链接
        assert_eq!(
            plan(&with_command("help:manual")),
            vec![MenuStep::OpenUrl {
                url: "https://motrix.app/manual"
            }]
        );

        // 退出
        assert_eq!(
            plan(&with_command("application:quit")),
            vec![MenuStep::Quit]
        );

        // 只有 role、没有 command：Tauri 缺的预定义项由 Rust 实现
        assert_eq!(
            plan(&MenuAction {
                role: Some("toggledevtools".to_string()),
                ..Default::default()
            }),
            vec![MenuStep::ToggleDevTools]
        );
        // 系统自己处理的 role（quit 等）不会走到这里，plan 里也不该凭空造出步骤
        assert!(
            plan(&MenuAction {
                role: Some("quit".to_string()),
                ..Default::default()
            })
            .is_empty()
        );
    }

    /// 托盘菜单不挂快捷键；App 菜单照旧（同一批命令在两种菜单里表现不同）
    #[test]
    fn tray_menu_has_no_accelerators() {
        fn collect(nodes: &[MenuNode], kind: MenuKind, result: &mut Vec<String>) {
            for node in nodes {
                if let Some(accelerator) = accelerator_of(kind, node) {
                    result.push(accelerator);
                }
                if let Some(children) = &node.submenu {
                    collect(children, kind, result);
                }
            }
        }

        // 托盘：一个快捷键提示都不该有
        let tray = read_nodes(SOURCE_LOCALE, "tray.json");
        let mut tray_accelerators = Vec::new();
        collect(&tray, MenuKind::Tray, &mut tray_accelerators);
        assert!(
            tray_accelerators.is_empty(),
            "托盘菜单不该显示快捷键: {tray_accelerators:?}"
        );

        // 同一批命令在 App 菜单里照旧带快捷键（说明不是 keymap 里没配）
        assert!(
            tray.iter()
                .any(|node| node.command.as_deref() == Some("application:new-task")),
            "托盘里应有「新建任务」"
        );
        let mut app_accelerators = Vec::new();
        collect(
            &read_nodes(SOURCE_LOCALE, "darwin.json"),
            MenuKind::App,
            &mut app_accelerators,
        );
        assert!(
            app_accelerators.contains(&"CmdOrCtrl+N".to_string()),
            "App 菜单里「新建任务」应带 CmdOrCtrl+N: {app_accelerators:?}"
        );
    }

    /// 「退出」：托盘菜单里是本地命令；三个平台的 App 菜单用的是系统 role（系统自己处理）
    #[test]
    fn quit_is_handled_locally() {
        // 托盘菜单：application:quit -> 本地退出
        let action = action_of(SOURCE_LOCALE, "tray.json", "app.quit");
        assert_eq!(action.command.as_deref(), Some("application:quit"));
        assert_eq!(plan(&action), vec![MenuStep::Quit]);

        // App 菜单：role: quit（Tauri 有预定义项），不进动作表
        for file in ["darwin.json", "win32.json", "linux.json"] {
            let nodes = read_nodes(SOURCE_LOCALE, file);
            let quit =
                find_node(&nodes, "app.quit").unwrap_or_else(|| panic!("{file} 里没有 app.quit"));
            assert_eq!(
                quit.role.as_deref(),
                Some("quit"),
                "{file} 的退出应该用 role"
            );
            assert_eq!(quit.command, None, "{file} 的退出不该配 command");

            let mut actions = HashMap::new();
            collect_actions(&nodes, &mut actions);
            assert!(
                actions.get("app.quit").is_none(),
                "{file}: 系统 role 不该进动作表"
            );
        }
    }

    /// 帮助菜单：4 个链接 + 开发者工具（都用真实菜单文件验证）
    #[test]
    fn help_menu_is_handled_locally() {
        // 这四项只在 App 菜单里（托盘菜单只有「使用手册」）
        for (id, url) in [
            ("help.official-website", "https://motrix.app/"),
            ("help.manual", "https://motrix.app/manual"),
            ("help.release-notes", "https://motrix.app/release"),
            ("help.report-problem", "https://motrix.app/report"),
        ] {
            for file in ["darwin.json", "win32.json", "linux.json"] {
                let steps = plan(&action_of(SOURCE_LOCALE, file, id));
                assert_eq!(
                    steps,
                    vec![MenuStep::OpenUrl { url }],
                    "{file} 里 {id} 的计划不对"
                );
            }
        }

        // 开发者工具：模板里只有 role，没有 command
        let action = action_of(SOURCE_LOCALE, "darwin.json", "help.toggle-dev-tools");
        assert_eq!(action.command, None);
        assert_eq!(action.role.as_deref(), Some("toggledevtools"));
        assert_eq!(plan(&action), vec![MenuStep::ToggleDevTools]);

        // 托盘菜单里也有「使用手册」
        assert_eq!(
            plan(&action_of(SOURCE_LOCALE, "tray.json", "help.manual")),
            vec![MenuStep::OpenUrl {
                url: "https://motrix.app/manual"
            }]
        );
    }

    /// 「打开种子文件…」点下去应该是"显示窗口 + Rust 选文件"，不能再出现整条命令被下发给前端
    /// （App 菜单、托盘菜单、各平台都一样）
    #[test]
    fn task_menu_open_file_is_handled_locally() {
        for file in MENU_FILES {
            let steps = plan(&action_of(SOURCE_LOCALE, file, "task.open-file"));

            assert_eq!(
                steps,
                vec![
                    MenuStep::Show(Some("index".to_string())),
                    MenuStep::OpenFile,
                ],
                "{file} 里「打开种子文件」的执行计划不对"
            );
            assert!(
                !steps
                    .iter()
                    .any(|step| matches!(step, MenuStep::Frontend { .. })),
                "{file}: 这条命令不该下发给前端"
            );
        }
    }

    /// App 菜单里由 Rust 自己实现功能的项（不含只是显示窗口的 before/after）
    /// 新增本地功能时同步改这里
    #[test]
    fn app_menu_local_features() {
        /// 计划里除了"显示窗口"和"下发前端"，还有没有真正做事的一步
        fn has_feature_step(action: &MenuAction) -> bool {
            plan(action).iter().any(|step| {
                matches!(
                    step,
                    MenuStep::OpenFile
                        | MenuStep::OpenUrl { .. }
                        | MenuStep::ToggleDevTools
                        | MenuStep::Quit
                )
            })
        }

        let mut actions = HashMap::new();
        collect_actions(&read_nodes(SOURCE_LOCALE, "darwin.json"), &mut actions);

        let mut local: Vec<&str> = actions
            .iter()
            .filter(|(_, action)| has_feature_step(action))
            .map(|(id, _)| id.as_str())
            .collect();
        local.sort();

        assert_eq!(
            local,
            vec![
                "help.manual",
                "help.official-website",
                "help.release-notes",
                "help.report-problem",
                "help.toggle-dev-tools",
                "task.open-file",
            ]
        );
    }

    /// 种子文件解析：校验扩展名，读出的 dataURL 是裸 base64（前端 base64StringToBlob 要的）
    #[test]
    fn torrent_payload_is_base64() {
        let dir = std::env::temp_dir().join(format!("motr-torrent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();

        let torrent = dir.join("demo.torrent");
        let bytes: &[u8] = b"d8:announce4:url1e";
        std::fs::write(&torrent, bytes).unwrap();

        let payload = torrent_payload(&torrent).unwrap();
        assert_eq!(payload["name"], json!("demo.torrent"));
        assert_eq!(payload["dataURL"], json!(BASE64_STANDARD.encode(bytes)));

        let data_url = payload["dataURL"].as_str().unwrap();
        assert!(!data_url.starts_with("data:"), "前端要的是裸 base64");

        // 非种子文件要拒绝（大小写不敏感地判断扩展名）
        let other = dir.join("demo.txt");
        std::fs::write(&other, b"x").unwrap();
        assert!(torrent_payload(&other).is_err());
        assert!(torrent_payload(&dir.join("missing.torrent")).is_err());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 各语言的菜单结构一致，动作表也应一致（新增任务在各语言下都能用）
    #[test]
    fn new_task_action_matches_across_locales() {
        for locale in locales() {
            let mut actions = HashMap::new();
            collect_actions(&read_nodes(&locale, "darwin.json"), &mut actions);
            let action = actions.get("task.new-task").unwrap();
            assert_eq!(
                action.command.as_deref(),
                Some("application:new-task"),
                "{locale} 的菜单命令不一致"
            );
        }
    }

    /// 菜单文本优先用生成的 text，缺了退回 id
    #[test]
    fn text_priority() {
        let node: MenuNode =
            serde_json::from_str(r#"{"id":"task.new-task","text":"新建任务"}"#).unwrap();
        assert_eq!(node_text(&node), "新建任务");

        let node: MenuNode = serde_json::from_str(r#"{"id":"task.new-task"}"#).unwrap();
        assert_eq!(node_text(&node), "task.new-task");
    }

    /// tray 是裸数组、平台文件是 { "menu": [...] }，两种形状都要能解析
    #[test]
    fn both_menu_file_shapes_are_parsed() {
        let wrapped: MenuFile =
            serde_json::from_str(r#"{"menu":[{"id":"a.b","text":"A"}]}"#).unwrap();
        assert_eq!(wrapped.into_nodes().len(), 1);

        let bare: MenuFile = serde_json::from_str(r#"[{"id":"a.b","text":"A"}]"#).unwrap();
        assert_eq!(bare.into_nodes().len(), 1);
    }

    /// command-* 字段留着以后接功能，解析不能出错
    #[test]
    fn command_fields_are_parsed() {
        let node: MenuNode = serde_json::from_str(
            r#"{"id":"task.open-file","text":"打开种子文件...","command":"application:open-file","command-before":"application:show?page=index"}"#,
        )
        .unwrap();
        assert_eq!(node.command.as_deref(), Some("application:open-file"));
        assert_eq!(
            node.command_before.as_deref(),
            Some("application:show?page=index")
        );
        assert_eq!(node.text.as_deref(), Some("打开种子文件..."));
    }
}
