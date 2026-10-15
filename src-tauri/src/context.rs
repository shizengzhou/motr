use serde::Serialize;
use tauri::{AppHandle, Manager, path::BaseDirectory::Resource};
use tauri_plugin_os::type_;

/// 运行时环境信息，对应 JS `loadConfigFromNativeStore()` 里的 context
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Context {
    pub platform: String,
    pub arch: String,
    pub log_path: String,
    pub session_path: String,
    pub engine_path: String,
    pub aria2_bin_path: String,
    pub aria2_conf_path: String,
}

pub fn get_session_path(app_handle: &AppHandle) -> String {
    app_handle
        .path()
        .app_data_dir()
        .unwrap()
        .join("download.session")
        .to_string_lossy()
        .to_string()
}

pub fn get_aria2_conf_path(app_handle: &AppHandle) -> String {
    app_handle
        .path()
        .resolve(format!("{}/aria2.conf", type_()), Resource)
        .unwrap()
        .to_string_lossy()
        .to_string()
}
