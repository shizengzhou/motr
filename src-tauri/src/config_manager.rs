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
                ProxyScope::UploadApp.as_str().to_string(),
                ProxyScope::UploadTracker.as_str().to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ProxyScope {
    Download,
    UploadApp,
    UploadTracker,
}

impl ProxyScope {
    fn as_str(&self) -> &str {
        match self {
            ProxyScope::Download => "download",
            ProxyScope::UploadApp => "upload-app",
            ProxyScope::UploadTracker => "upload-tracker",
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

    #[test]
    fn proxy() {
        let proxy = Proxy::default();
        assert_eq!(
            proxy.scope,
            vec!["download", "upload-app", "upload-tracker"]
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
