use dunce::simplified;
use std::path::PathBuf;

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
        .map(|dir| dir.join("download.session"))
        // 解析不到目录也不该 panic：给个相对路径，引擎会照常报错
        .unwrap_or_else(|err| {
            log::warn!("[Motr] 解析应用数据目录失败: {err}");

            PathBuf::from("download.session")
        })
        .to_string_lossy()
        .to_string()
}

pub fn get_aria2_conf_path(app_handle: &AppHandle) -> String {
    let path = app_handle
        .path()
        .resolve(format!("{}/aria2.conf", type_()), Resource)
        .unwrap_or_else(|err| {
            log::warn!("[Motr] 解析 {}/aria2.conf 失败: {err}", type_());

            PathBuf::from("aria2.conf")
        });

    if !path.exists() {
        log::warn!(
            "[Motr] 打包资源里缺少 {}：aria2 可能起不来（检查 tauri.conf.json 的 resources）",
            path.display()
        );
    }

    // Windows 上 Tauri 的 current_exe() 做过 canonicalize()，resolve 出来的资源路径
    // 会带 `\\?\` 长路径前缀。aria2c 读不了这种路径——文件明明存在也会报
    // "Configuration file ... is not found" 然后退出，引擎起不来。
    // dunce::simplified 去掉 `\\?\` 前缀（Unix 上原样返回）
    simplified(&path).to_string_lossy().to_string()
}
