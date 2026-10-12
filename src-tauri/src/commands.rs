use tauri::{AppHandle, Manager};
use tauri_plugin_os::{arch, platform};
use tauri_plugin_store::StoreExt;

use crate::config_manager::{Config, SystemConfig, UserConfig};
use crate::context::{Context, get_aria2_conf_path, get_session_path};
use crate::{auto_launch, menu, run_mode, tray};

/// 设置页保存后调用：让系统登录项与 user.json 里的 open-at-login 对齐
#[tauri::command]
pub fn apply_auto_launch(app: AppHandle) -> Result<(), String> {
    auto_launch::apply(&app);
    Ok(())
}

/// 切换语言后重建菜单（前端 locale 变化时调用，只传语言值，菜单数据都在资源里）
#[tauri::command]
pub fn rebuild_menus(app: AppHandle, locale: String) -> Result<(), String> {
    menu::apply(&app, &locale)
}

/// 把渲染进程画好的「图标 + 上下行速率」PNG 换成托盘图标
///（和原 Electron 版一致：canvas 在渲染进程画，这边只负责 setImage）
#[tauri::command]
pub fn set_tray_speedometer(app: AppHandle, png: Vec<u8>) -> Result<(), String> {
    tray::set_speedometer(&app, &png)
}

/// 恢复静态托盘图标（关掉「托盘显示实时速度」、组件卸载时由前端调用）
#[tauri::command]
pub fn reset_tray_icon(app: AppHandle) -> Result<(), String> {
    tray::reset_icon(&app)
}

/// 设置页保存后调用：按最新的 user.json 重新应用运行模式
#[tauri::command]
pub fn apply_run_mode(app: AppHandle) -> Result<(), String> {
    run_mode::apply_from_config(&app);
    Ok(())
}

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
        session_path: get_session_path(&app_handle),
        engine_path: String::new(),
        aria2_bin_path: String::new(),
        aria2_conf_path: get_aria2_conf_path(&app_handle),
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
