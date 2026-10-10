use tauri::{AppHandle, Manager};
use tauri_plugin_os::{arch, platform};
use tauri_plugin_store::StoreExt;

use crate::config_manager::{Config, SystemConfig, UserConfig};
use crate::context::Context;

#[tauri::command]
pub async fn trash_item(path: String) -> Result<(), String> {
    let target = path.clone();

    match tauri::async_runtime::spawn_blocking(move || trash::delete(&path)).await {
        Ok(Ok(())) => Ok(()),
        // 真正的删除失败（文件不存在、权限不足、回收站不可用等）
        Ok(Err(err)) => {
            log::warn!("trash_item failed for {target}: {err}");
            Err(err.to_string())
        }
        // 阻塞任务本身出错（panic / 被取消）
        Err(err) => {
            log::warn!("trash_item task failed for {target}: {err}");
            Err(err.to_string())
        }
    }
}

#[tauri::command]
pub fn get_app_config(app_handle: AppHandle) -> Result<Config, String> {
    let context = Context {
        platform: platform().to_string(),
        arch: arch().to_string(),
        log_path: app_handle
            .path()
            .app_log_dir()
            .unwrap()
            .to_string_lossy()
            .to_string(),
        session_path: String::new(),
        engine_path: String::new(),
        aria2_bin_path: String::new(),
        aria2_conf_path: String::new(),
    };

    // user.json 里有数据就用文件里的，缺失的键（或整份文件不存在）回落到默认值
    let user_config = match app_handle.store("user.json") {
        Ok(store) => UserConfig::from_store(&store).unwrap_or_else(|err| {
            log::warn!("load user.json failed: {err}");
            UserConfig::default()
        }),
        Err(err) => {
            log::warn!("open user.json failed: {err}");
            UserConfig::default()
        }
    };

    // system.json 里有数据就用文件里的，缺失的键（或整份文件不存在）回落到默认值
    let system_config = match app_handle.store("system.json") {
        Ok(store) => SystemConfig::from_store(&store).unwrap_or_else(|err| {
            log::warn!("load system.json failed: {err}");
            SystemConfig::with_app_handle(&app_handle)
        }),
        Err(err) => {
            log::warn!("open system.json failed: {err}");
            SystemConfig::with_app_handle(&app_handle)
        }
    };

    Ok(Config {
        context,
        user_config,
        system_config,
    })
}
