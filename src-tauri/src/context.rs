use dunce::simplified;
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
    let path = app_handle
        .path()
        .resolve(format!("{}/aria2.conf", type_()), Resource)
        .unwrap();

    // Windows 上 Tauri 的 current_exe() 做过 canonicalize()，resolve 出来的资源路径
    // 会带 `\\?\` 长路径前缀。aria2c 读不了这种路径——文件明明存在也会报
    // "Configuration file ... is not found" 然后退出，引擎起不来。
    // dunce::simplified 去掉 `\\?\` 前缀（Unix 上原样返回）
    simplified(&path).to_string_lossy().to_string()
}
