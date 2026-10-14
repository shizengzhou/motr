use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_store::{Store, StoreExt};

use crate::context::Context;

pub const ENGINE_MAX_CONNECTION_PER_SERVER: u32 = 64;

pub const ENGINE_RPC_PORT: u16 = 16800;

pub const CHROME_UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/111.0.0.0 Safari/537.36";

/// See: https://github.com/ngosang/trackerslist
pub const NGOSANG_TRACKERS_BEST_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best.txt";

pub const NGOSANG_TRACKERS_BEST_IP_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_best_ip.txt";

pub const NGOSANG_TRACKERS_ALL_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all.txt";

pub const NGOSANG_TRACKERS_ALL_IP_URL: &str =
    "https://raw.githubusercontent.com/ngosang/trackerslist/master/trackers_all_ip.txt";

pub const NGOSANG_TRACKERS_BEST_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/ngosang/trackerslist/trackers_best.txt";

pub const NGOSANG_TRACKERS_BEST_IP_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/ngosang/trackerslist/trackers_best_ip.txt";

pub const NGOSANG_TRACKERS_ALL_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/ngosang/trackerslist/trackers_all.txt";

pub const NGOSANG_TRACKERS_ALL_IP_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/ngosang/trackerslist/trackers_all_ip.txt";

/// See: https://github.com/XIU2/TrackersListCollection
pub const XIU2_TRACKERS_BEST_URL: &str =
    "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/best.txt";

pub const XIU2_TRACKERS_ALL_URL: &str =
    "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/all.txt";

pub const XIU2_TRACKERS_HTTP_URL: &str =
    "https://raw.githubusercontent.com/XIU2/TrackersListCollection/master/http.txt";

pub const XIU2_TRACKERS_BEST_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection/best.txt";

pub const XIU2_TRACKERS_ALL_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection/all.txt";

pub const XIU2_TRACKERS_HTTP_URL_CDN: &str =
    "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection/http.txt";

// For bt-exclude-tracker
pub const XIU2_TRACKERS_BLACK_URL: &str =
    "https://cdn.jsdelivr.net/gh/XIU2/TrackersListCollection/blacklist.txt";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", default)]
pub struct UserConfig {
    pub auto_check_update: bool,
    pub auto_hide_window: bool,
    pub auto_sync_tracker: bool,
    pub enable_upnp: bool,
    pub engine_max_connection_per_server: u32,
    pub favorite_directories: Vec<String>,
    pub hide_app_menu: bool,
    pub history_directories: Vec<String>,
    pub keep_seeding: bool,
    pub keep_window_state: bool,
    pub last_check_update_time: i64,
    pub last_sync_tracker_time: i64,
    pub locale: String,
    pub log_level: String,
    pub new_task_show_downloading: bool,
    pub no_confirm_before_delete_task: bool,
    pub open_at_login: bool,
    pub protocols: Protocols,
    pub proxy: Proxy,
    pub resume_all_when_app_launched: bool,
    pub run_mode: u8,
    pub show_progress_bar: bool,
    pub task_notification: bool,
    pub theme: String,
    pub tracker_source: Vec<String>,
    pub tray_theme: String,
    pub tray_speedometer: bool,
    pub update_channel: String,
    pub window_state: WindowState,
}

impl Default for UserConfig {
    fn default() -> Self {
        let locale = tauri_plugin_os::locale().unwrap();

        Self {
            auto_check_update: is_macos(),
            auto_hide_window: false,
            auto_sync_tracker: true,
            enable_upnp: true,
            engine_max_connection_per_server: ENGINE_MAX_CONNECTION_PER_SERVER,
            favorite_directories: vec![],
            hide_app_menu: is_windows_or_linux(),
            history_directories: vec![],
            keep_seeding: false,
            keep_window_state: false,
            last_check_update_time: 0,
            last_sync_tracker_time: 0,
            locale: locale,
            log_level: "warn".to_string(),
            new_task_show_downloading: true,
            no_confirm_before_delete_task: false,
            open_at_login: false,
            protocols: Protocols::default(),
            proxy: Proxy::default(),
            resume_all_when_app_launched: false,
            run_mode: AppRunMode::Standard as u8,
            show_progress_bar: true,
            task_notification: true,
            theme: AppTheme::Auto.as_str().to_string(),
            tracker_source: vec![
                NGOSANG_TRACKERS_BEST_IP_URL_CDN.to_string(),
                NGOSANG_TRACKERS_BEST_URL_CDN.to_string(),
            ],
            tray_theme: AppTheme::Auto.as_str().to_string(),
            tray_speedometer: is_macos(),
            update_channel: "latest".to_string(),
            window_state: WindowState {},
        }
    }
}

impl UserConfig {
    pub fn from_store<R: Runtime>(store: &Store<R>) -> Result<Self, String> {
        let map: serde_json::Map<String, serde_json::Value> = store.entries().into_iter().collect();

        serde_json::from_value(serde_json::Value::Object(map)).map_err(|err| err.to_string())
    }
}

/// system.json 的完整结构（aria2 引擎参数）
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case", default)]
pub struct SystemConfig {
    pub all_proxy: String,
    pub allow_overwrite: bool,
    pub auto_file_renaming: bool,
    pub bt_exclude_tracker: String,
    pub bt_force_encryption: bool,
    pub bt_load_saved_metadata: bool,
    pub bt_save_metadata: bool,
    pub bt_tracker: String,
    /// `continue` 是 Rust 关键字，字段名用 raw identifier，显式 rename 保证 JSON 键名正确
    #[serde(rename = "continue")]
    pub r#continue: bool,
    /// 应用数据目录下的 dht.dat，运行时才能确定，见 [`SystemConfig::with_app_handle`]
    pub dht_file_path: String,
    /// 应用数据目录下的 dht6.dat
    pub dht_file_path6: String,
    pub dht_listen_port: u16,
    pub dir: String,
    pub enable_dht6: bool,
    pub follow_metalink: bool,
    pub follow_torrent: bool,
    pub listen_port: u16,
    pub max_concurrent_downloads: u16,
    pub max_connection_per_server: u32,
    pub max_download_limit: u64,
    pub max_overall_download_limit: u64,
    pub max_overall_upload_limit: u64,
    pub no_proxy: String,
    pub pause_metadata: bool,
    pub pause: bool,
    pub rpc_listen_port: u16,
    pub rpc_secret: String,
    pub seed_ratio: f64,
    pub seed_time: u32,
    pub split: u32,
    pub user_agent: String,
}

impl Default for SystemConfig {
    /// `dht_file_path` / `dht_file_path6` / `dir` 依赖运行时路径，默认留空，
    /// 用 [`SystemConfig::with_app_handle`] 获取带路径的完整配置
    fn default() -> Self {
        Self {
            all_proxy: String::new(),
            allow_overwrite: false,
            auto_file_renaming: true,
            bt_exclude_tracker: String::new(),
            bt_force_encryption: false,
            bt_load_saved_metadata: true,
            bt_save_metadata: true,
            bt_tracker: String::new(),
            r#continue: true,
            dht_file_path: String::new(),
            dht_file_path6: String::new(),
            dht_listen_port: 26701,
            dir: String::new(),
            enable_dht6: true,
            follow_metalink: true,
            follow_torrent: true,
            listen_port: 21301,
            max_concurrent_downloads: 5,
            max_connection_per_server: ENGINE_MAX_CONNECTION_PER_SERVER,
            max_download_limit: 0,
            max_overall_download_limit: 0,
            max_overall_upload_limit: 0,
            no_proxy: String::new(),
            pause_metadata: false,
            pause: true,
            rpc_listen_port: ENGINE_RPC_PORT,
            rpc_secret: String::new(),
            seed_ratio: 2.0,
            seed_time: 2880,
            split: ENGINE_MAX_CONNECTION_PER_SERVER,
            user_agent: CHROME_UA.to_string(),
        }
    }
}

impl SystemConfig {
    /// 读 system.json：**先铺上依赖 app handle 的默认值，再用存档里的键覆盖**。
    ///
    /// 不能写成 `from_store(...).unwrap_or_else(|_| with_app_handle(...))`：
    /// store 只要读得动就返回 `Ok`（空文件、只存了几个键都算 `Ok`），那句兜底永远不跑，
    /// 于是 `dir` / `dht-file-path*` 这些"默认值得靠 app handle 才算得出来"的字段
    /// 就成了 `Default` 里的空串。
    ///
    /// 触发场景：设置页的「恢复初始设置」清空 system.json 之后（以及全新安装）。
    /// 表现是设置里「存储路径」变空、新建任务对话框的存储路径也空，
    /// 而且 aria2 会拿到一个空的 `--dir`。
    pub fn from_store_with_defaults<R: Runtime>(
        app_handle: &AppHandle<R>,
        store: &Store<R>,
    ) -> Self {
        let stored = store.entries().into_iter().collect();

        match merge_system_config(Self::with_app_handle(app_handle), stored) {
            Ok(config) => config,
            Err(err) => {
                log::warn!("[Motr] merge system.json failed: {err}");

                Self::with_app_handle(app_handle)
            }
        }
    }

    pub fn with_app_handle<R: Runtime>(app_handle: &AppHandle<R>) -> Self {
        let mut config = Self::default();

        if let Ok(data_dir) = app_handle.path().app_data_dir() {
            config.dht_file_path = data_dir.join("dht.dat").to_string_lossy().into_owned();
            config.dht_file_path6 = data_dir.join("dht6.dat").to_string_lossy().into_owned();
        }

        if let Ok(download_dir) = app_handle.path().download_dir() {
            config.dir = download_dir.to_string_lossy().into_owned();
        }

        config
    }

    pub fn from_store<R: Runtime>(store: &Store<R>) -> Result<Self, String> {
        let map: serde_json::Map<String, serde_json::Value> = store.entries().into_iter().collect();

        serde_json::from_value(serde_json::Value::Object(map)).map_err(|err| err.to_string())
    }
}

/// 把存档里的值盖在默认值上（存档里没有的键保持默认）。
///
/// 从 `from_store_with_defaults` 里抽出来的纯函数：那边要 AppHandle，这里只要能序列化的
/// 默认值就能测 —— "清空/少键的存档不能把 app handle 算出来的默认值抹掉"这条断言靠它守
fn merge_system_config(
    defaults: SystemConfig,
    stored: serde_json::Map<String, serde_json::Value>,
) -> Result<SystemConfig, String> {
    let mut merged = serde_json::to_value(defaults)
        .map_err(|err| err.to_string())?
        .as_object()
        .cloned()
        .unwrap_or_default();

    merged.extend(stored);

    serde_json::from_value(serde_json::Value::Object(merged)).map_err(|err| err.to_string())
}

/// 注意：三个子结构体的键不能重名 —— flatten 会把它们摊到同一层，
/// 重名会在 JSON 里产生重复键，`serde_json::to_value` 时后者覆盖前者并丢字段。
#[derive(Debug, Serialize)]
pub struct Config {
    #[serde(flatten)]
    pub context: Context,
    #[serde(flatten)]
    pub system_config: SystemConfig,
    #[serde(flatten)]
    pub user_config: UserConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Protocols {
    pub magnet: bool,
    pub thunder: bool,
}

impl Default for Protocols {
    fn default() -> Self {
        Self {
            magnet: true,
            thunder: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Proxy {
    pub enable: bool,
    pub server: String,
    pub bypass: String,
    pub scope: Vec<String>,
}

impl Default for Proxy {
    fn default() -> Self {
        Self {
            enable: false,
            server: "".to_string(),
            bypass: "".to_string(),
            scope: vec![
                ProxyScope::Download.as_str().to_string(),
                ProxyScope::UpdateApp.as_str().to_string(),
                ProxyScope::UpdateTrackers.as_str().to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ProxyScope {
    Download,
    UpdateApp,
    UpdateTrackers,
}

impl ProxyScope {
    pub fn as_str(&self) -> &str {
        match self {
            ProxyScope::Download => "download",
            ProxyScope::UpdateApp => "update-app",
            ProxyScope::UpdateTrackers => "update-trackers",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AppRunMode {
    Standard = 1,
    Tray = 2,
    HideTray = 3,
}

impl AppRunMode {
    /// 从 user.json 里的数值解析（未知值按默认 Standard，与原项目一致）
    pub fn from_u8(mode: u8) -> Self {
        match mode {
            2 => Self::Tray,
            3 => Self::HideTray,
            _ => Self::Standard,
        }
    }
}

#[derive(Debug, Serialize)]
pub enum AppTheme {
    Auto,
    Light,
    Dark,
}

impl AppTheme {
    fn as_str(&self) -> &str {
        match self {
            AppTheme::Auto => "auto",
            AppTheme::Light => "light",
            AppTheme::Dark => "dark",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowState {}

/// 「恢复初始设置」：清空 user.json 与 system.json。
///
/// 对应上游 `ConfigManager.reset()` 里的 `userConfig.clear()` + `systemConfig.clear()`：
/// 读配置的两个 `from_store` 都带 serde default，键没了就回落默认值，所以"空文件 == 初始设置"。
///
/// 用 store 的 API 清空而不是直接删文件：插件缓存里还留着旧值，而且它在
/// `RunEvent::Exit` 时会把缓存写盘（见 tauri-plugin-store 的 `on_event`），
/// 直接删文件会被它写回来
pub fn reset_stores<R: Runtime>(app_handle: &AppHandle<R>) -> Result<(), String> {
    for path in ["user.json", "system.json"] {
        let store = app_handle.store(path).map_err(|err| err.to_string())?;

        store.clear();
        store.save().map_err(|err| err.to_string())?;

        log::info!("[Motr] 已清空 {path}（恢复初始设置）");
    }

    Ok(())
}

/// 让本进程的 store 缓存与磁盘对齐。
///
/// 前端（JS 侧的 `LazyStore`）会**直接**写 `user.json` / `system.json`，而本进程的
/// store 缓存是启动时从磁盘加载的。不重读会踩两个坑：
/// - 读：设置页保存后立刻重读，拿到的还是旧值；
/// - 写：插件在 `RunEvent::Exit` 会把**缓存**写回文件（tauri-plugin-store 的
///   `on_event`），于是前端这次会话写进去的东西被启动时的旧缓存覆盖掉
///   （表现：`last-check-update-time` 永远是启动那一刻的旧值 ⇒ 每次启动都检查更新）。
pub fn reload_store<R: Runtime>(app_handle: &AppHandle<R>, file: &str) {
    match app_handle.store(file) {
        Ok(store) => {
            if let Err(err) = store.reload_ignore_defaults() {
                log::warn!("[Motr] {file}: 重读磁盘失败: {err}");
            }
        }
        Err(err) => log::warn!("[Motr] {file}: 打开失败: {err}"),
    }
}

/// 把「默认值 + 存档」合并后的完整配置写回存档：缺失的键补齐、值不同的以合并结果为准。
///
/// 原版用 `electron-store` 的 `defaults`，配置一读就把默认值写进文件，所以
/// `user.json` / `system.json` 永远是完整可读的（缺什么键一眼可见、也能照着文件手改）。
/// 移植后只剩内存兜底 ⇒ 文件里只有"被改过的键"，看起来像配置丢了
///（例：`last-check-update-time` 从没被写过，就查不到它当前是几）。
///
/// 调用前请先 `reload_store`（见那里的说明），否则会用旧缓存覆盖前端的写入。
pub fn sync_store<R: Runtime, T: Serialize>(app_handle: &AppHandle<R>, file: &str, merged: &T) {
    let Ok(store) = app_handle.store(file) else {
        log::warn!("[Motr] {file}: 打开失败，跳过补默认值");
        return;
    };

    let Ok(serde_json::Value::Object(target)) = serde_json::to_value(merged) else {
        log::warn!("[Motr] {file}: 序列化失败，跳过补默认值");
        return;
    };

    let stored: HashMap<String, serde_json::Value> = store.entries().into_iter().collect();
    let pending = keys_to_write(&stored, &target);

    if pending.is_empty() {
        return;
    }

    for (key, value) in pending {
        store.set(key, value);
    }

    if let Err(err) = store.save() {
        log::warn!("[Motr] {file}: 写盘失败: {err}");
    }
}

/// 存档里缺的、或与合并结果不一致的键（纯函数，便于测）
///
/// 只挑"要写的"，不删存档里多出来的键 —— 结构体里没有的键（老版本残留、
/// 前端自己加的）原样保留。
fn keys_to_write(
    stored: &HashMap<String, serde_json::Value>,
    merged: &serde_json::Map<String, serde_json::Value>,
) -> Vec<(String, serde_json::Value)> {
    merged
        .iter()
        .filter(|(key, value)| stored.get(key.as_str()) != Some(value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// 读 user.json：重读磁盘 → 用默认值兜底 → 把合并结果落盘（保持配置文件完整）
pub fn load_user_config<R: Runtime>(app_handle: &AppHandle<R>) -> UserConfig {
    let Ok(store) = app_handle.store("user.json") else {
        log::warn!("[Motr] 打开 user.json 失败，使用默认值");
        return UserConfig::default();
    };

    reload_store(app_handle, "user.json");

    let config = UserConfig::from_store(&store).unwrap_or_else(|err| {
        log::warn!("[Motr] 解析 user.json 失败，使用默认值: {err}");
        UserConfig::default()
    });

    sync_store(app_handle, "user.json", &config);

    config
}

/// 读 system.json：重读磁盘 → 用（依赖 app handle 的）默认值兜底 → 落盘
///
/// 默认值里的 `dir` / `dht-file-path*` 要靠 app handle 现算，所以不能用 `Default`，
/// 详见 `from_store_with_defaults`。
pub fn load_system_config<R: Runtime>(app_handle: &AppHandle<R>) -> SystemConfig {
    let Ok(store) = app_handle.store("system.json") else {
        log::warn!("[Motr] 打开 system.json 失败，使用默认值");
        return SystemConfig::with_app_handle(app_handle);
    };

    reload_store(app_handle, "system.json");

    let config = SystemConfig::from_store_with_defaults(app_handle, &store);

    sync_store(app_handle, "system.json", &config);

    config
}

/// 退出前把两个存档的缓存刷新成磁盘上的最新内容。
///
/// 放在 `RunEvent::ExitRequested`（先于 `Exit` 触发）里执行，正好赶在
/// tauri-plugin-store 写盘之前，避免用旧缓存覆盖前端本次会话的写入。
pub fn refresh_stores<R: Runtime>(app_handle: &AppHandle<R>) {
    for path in ["user.json", "system.json"] {
        reload_store(app_handle, path);
    }
}

/// 「下载协议」的两个开关（magnet / thunder）：读 user.json，缺键或读不到时按默认值
///（magnet 开、thunder 关，与原项目一致）
/// 开关能在运行中改（见 protocol.rs），所以这里是现读现用。
pub fn protocols<R: Runtime>(app_handle: &AppHandle<R>) -> Protocols {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| config.protocols)
        .unwrap_or_default()
}

/// 「启用 UPnP」是否开启：读 user.json，缺键或读不到时按默认值（true，与原项目一致）
///
/// 开关能在运行中改（见 upnp.rs），所以这里是现读现用，不是只在启动时读一次。
pub fn enable_upnp<R: Runtime>(app_handle: &AppHandle<R>) -> bool {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| config.enable_upnp)
        .unwrap_or(true)
}

/// 需要映射到路由器的两个监听端口（BT / DHT）：读 system.json，缺键或读不到时按默认值
pub fn listen_ports<R: Runtime>(app_handle: &AppHandle<R>) -> (u16, u16) {
    let system_config = app_handle
        .store("system.json")
        .ok()
        .and_then(|store| SystemConfig::from_store(&store).ok())
        .unwrap_or_default();

    (system_config.listen_port, system_config.dht_listen_port)
}

/// 设置页里的日志等级：读 user.json，缺键或读不到时按默认值（warn）
///
/// 只在启动时用一次（logging::init），所以设置页保存后下次启动生效。
pub fn log_level<R: Runtime>(app_handle: &AppHandle<R>) -> String {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| config.log_level)
        .unwrap_or_else(|| "warn".to_string())
}

/// 「恢复上次退出时的窗口大小和位置」是否开启：读 user.json，缺键或读不到时按默认值（false）
///
/// 只在启动时用一次（window::restore_state），所以设置页保存后下次启动生效。
pub fn keep_window_state<R: Runtime>(app_handle: &AppHandle<R>) -> bool {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| config.keep_window_state)
        .unwrap_or(false)
}

/// 「自动隐藏窗口」是否开启：读 user.json，缺键或读不到时按默认值（false）
///
/// 窗口失焦时现读现用，所以设置页保存后立刻生效，不需要前端额外通知。
pub fn auto_hide_window_enabled<R: Runtime>(app_handle: &AppHandle<R>) -> bool {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| config.auto_hide_window)
        .unwrap_or(false)
}

/// 「开机自动启动」是否开启：读 user.json，缺键或读不到时按默认值（false）
pub fn open_at_login<R: Runtime>(app_handle: &AppHandle<R>) -> bool {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| config.open_at_login)
        .unwrap_or(false)
}

/// 当前的运行模式：读 user.json，缺键或读不到时按默认值（Standard）
///
/// 与「自动隐藏窗口」一样现读现用：设置页保存后由前端调 `apply_run_mode` 立即生效。
pub fn run_mode<R: Runtime>(app_handle: &AppHandle<R>) -> AppRunMode {
    app_handle
        .store("user.json")
        .ok()
        .and_then(|store| UserConfig::from_store(&store).ok())
        .map(|config| AppRunMode::from_u8(config.run_mode))
        .unwrap_or(AppRunMode::Standard)
}

pub fn is_macos() -> bool {
    #[cfg(target_os = "macos")]
    return true;

    false
}

pub fn is_windows_or_linux() -> bool {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    return true;

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 「恢复初始设置」把两份配置清空后，读出来必须全是默认值。
    ///
    /// `from_store` 内部就是 `serde_json::from_value`，空 map 会走各字段的
    /// serde default —— 这就是"清空 == 初始设置"的依据（清空而不是删文件的原因
    /// 见 `reset_stores`）
    #[test]
    fn empty_stores_fall_back_to_defaults() {
        let user: UserConfig = serde_json::from_value(serde_json::json!({})).unwrap();
        let system: SystemConfig = serde_json::from_value(serde_json::json!({})).unwrap();

        let default_user = UserConfig::default();
        let default_system = SystemConfig::default();

        assert_eq!(user.log_level, default_user.log_level);
        assert_eq!(user.run_mode, default_user.run_mode);
        assert_eq!(user.locale, default_user.locale);
        assert_eq!(user.protocols.magnet, default_user.protocols.magnet);
        assert_eq!(user.protocols.thunder, default_user.protocols.thunder);
        assert_eq!(user.enable_upnp, default_user.enable_upnp);
        assert_eq!(user.keep_window_state, default_user.keep_window_state);

        assert_eq!(system.rpc_listen_port, default_system.rpc_listen_port);
        assert_eq!(system.listen_port, default_system.listen_port);
        assert_eq!(system.dht_listen_port, default_system.dht_listen_port);
        assert_eq!(system.user_agent, default_system.user_agent);
        assert_eq!(system.dir, default_system.dir);
    }

    /// 存档里只存了零星几个键（或整个被清空）时，"靠 app handle 算出来的默认值"
    /// 不能被抹成空 —— 「恢复初始设置」清空 system.json 之后就是这个场景
    #[test]
    fn stored_values_override_handle_defaults() {
        let mut defaults = SystemConfig::default();
        defaults.dir = "/Users/me/Downloads".to_string();
        defaults.dht_file_path = "/data/dht.dat".to_string();

        // 空存档（恢复初始设置后的 system.json）：默认值原样保留
        let merged = merge_system_config(defaults.clone(), Default::default()).unwrap();
        assert_eq!(merged.dir, "/Users/me/Downloads");
        assert_eq!(merged.dht_file_path, "/data/dht.dat");

        // 存档里有的键要生效，没有的继续用默认值
        let stored = [("listen-port".to_string(), serde_json::json!(12345))]
            .into_iter()
            .collect();
        let merged = merge_system_config(defaults, stored).unwrap();
        assert_eq!(merged.listen_port, 12345, "存档里的键要盖过默认值");
        assert_eq!(merged.dir, "/Users/me/Downloads", "没存过的键保持默认");
    }

    /// 补默认值时只写"缺的/不一致的"键：
    /// - 内容一致时不动文件（否则每次启动都改 mtime、还会把前端写的值当成"差异"）
    /// - 存档里多出来的键不删（老版本残留、前端自定义的键都要留着）
    #[test]
    fn only_missing_or_changed_keys_are_written() {
        let merged: serde_json::Map<String, serde_json::Value> =
            serde_json::from_value(serde_json::json!({
                "auto-check-update": true,
                "locale": "zh-CN",
                "last-check-update-time": 0,
            }))
            .unwrap();

        // 存档里已有正确的值 + 一个结构体里没有的键 ⇒ 只补缺的那个
        let stored = [
            ("auto-check-update".to_string(), serde_json::json!(true)),
            ("locale".to_string(), serde_json::json!("zh-CN")),
            ("legacy-key".to_string(), serde_json::json!("keep-me")),
        ]
        .into_iter()
        .collect();

        let pending = keys_to_write(&stored, &merged);
        assert_eq!(pending.len(), 1, "只有缺失的 last-check-update-time 需要补");
        assert_eq!(pending[0].0, "last-check-update-time");
        assert_eq!(pending[0].1, serde_json::json!(0), "默认值本身也要落盘");

        // 完全一致 ⇒ 一个都不用写
        let stored = [
            ("auto-check-update".to_string(), serde_json::json!(true)),
            ("locale".to_string(), serde_json::json!("zh-CN")),
            ("last-check-update-time".to_string(), serde_json::json!(0)),
        ]
        .into_iter()
        .collect();
        assert!(keys_to_write(&stored, &merged).is_empty());

        // 值不同 ⇒ 以合并结果为准（前端改过的键要被落盘修正）
        let stored = [("locale".to_string(), serde_json::json!("en-US"))]
            .into_iter()
            .collect();
        let pending = keys_to_write(&stored, &merged);
        assert!(pending.iter().any(|(key, _)| key == "locale"));
    }

    /// 默认 scope 的三个值必须和前端 `PROXY_SCOPES`（@shared/constants.js）一致，
    /// 否则设置页那个多选框会显示成"一个都没选中"
    #[test]
    fn proxy_default_scope_matches_frontend() {
        let proxy = Proxy::default();
        assert_eq!(
            proxy.scope,
            vec!["download", "update-app", "update-trackers"]
        );
    }

    #[test]
    fn system_config_matches_system_json() {
        let value = serde_json::to_value(SystemConfig::default()).unwrap();
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|key| key.as_str())
            .collect();
        keys.sort_unstable();

        let mut expected = vec![
            "all-proxy",
            "allow-overwrite",
            "auto-file-renaming",
            "bt-exclude-tracker",
            "bt-force-encryption",
            "bt-load-saved-metadata",
            "bt-save-metadata",
            "bt-tracker",
            "continue",
            "dht-file-path",
            "dht-file-path6",
            "dht-listen-port",
            "dir",
            "enable-dht6",
            "follow-metalink",
            "follow-torrent",
            "listen-port",
            "max-concurrent-downloads",
            "max-connection-per-server",
            "max-download-limit",
            "max-overall-download-limit",
            "max-overall-upload-limit",
            "no-proxy",
            "pause",
            "pause-metadata",
            "rpc-listen-port",
            "rpc-secret",
            "seed-ratio",
            "seed-time",
            "split",
            "user-agent",
        ];
        expected.sort_unstable();

        assert_eq!(keys, expected);
        assert_eq!(value["continue"], true);
        assert_eq!(value["rpc-listen-port"], 16800);
        assert_eq!(value["dht-listen-port"], 26701);
        assert_eq!(value["listen-port"], 21301);
        assert_eq!(value["max-concurrent-downloads"], 5);
        assert_eq!(value["max-connection-per-server"], 64);
        assert_eq!(value["split"], 64);
        assert_eq!(value["seed-ratio"], 2.0);
        assert_eq!(value["seed-time"], 2880);
        assert_eq!(value["pause"], true);
    }

    fn test_context() -> Context {
        Context {
            platform: "macos".to_string(),
            arch: "aarch64".to_string(),
            log_path: "/tmp/logs".to_string(),
            session_path: String::new(),
            engine_path: String::new(),
            aria2_bin_path: String::new(),
            aria2_conf_path: String::new(),
        }
    }

    #[test]
    fn config_is_flat_merge_of_three_parts() {
        let config = Config {
            context: test_context(),
            user_config: UserConfig::default(),
            system_config: SystemConfig::default(),
        };
        let value = serde_json::to_value(&config).unwrap();
        let map = value.as_object().unwrap();

        // Context 7 + UserConfig 29 + SystemConfig 31，说明三部分没有重名键（否则会少）
        assert_eq!(map.len(), 7 + 29 + 31);

        // 三个来源的键都在同一层，且没有嵌套字段
        assert_eq!(value["platform"], "macos");
        assert_eq!(value["log-path"], "/tmp/logs");
        assert_eq!(value["log-level"], "warn");
        assert_eq!(value["theme"], "auto");
        assert_eq!(value["rpc-listen-port"], 16800);
        assert!(value.get("context").is_none());
        assert!(value.get("user-config").is_none());
        assert!(value.get("system-config").is_none());
    }

    #[test]
    fn user_config_accepts_partial_json() {
        // 模拟 user.json 只有部分键（store 里存的数据）
        let value = serde_json::json!({
            "log-level": "debug",
            "proxy": { "enable": true },
        });

        let config: UserConfig = serde_json::from_value(value).unwrap();

        assert_eq!(config.log_level, "debug");
        assert!(config.proxy.enable);
        // 没写到的键回落到默认值
        assert_eq!(config.proxy.scope, Proxy::default().scope);
        assert_eq!(config.theme, "auto");
        assert!(config.auto_sync_tracker);
    }
}
