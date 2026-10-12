use tauri::AppHandle;

use crate::config_manager::{self, AppRunMode};
use crate::{menu, tray};

/// 应用「运行模式」。三种模式的差异对齐原 Electron 版（TrayManager / DockManager）：
///
/// | 模式 | 托盘 | Dock 图标（macOS） |
/// | --- | --- | --- |
/// | Standard | 创建 | 显示 |
/// | Tray | 创建 | 隐藏（Accessory） |
/// | HideTray | 不创建 | 显示 |
///
/// 窗口关闭=隐藏、启动是否显示窗口、退出方式在原项目里与 run-mode 无关，所以这里也不涉及。
pub fn apply(app: &AppHandle, mode: AppRunMode) {
    apply_tray(app, mode);

    #[cfg(target_os = "macos")]
    apply_dock(app, mode);

    log::info!("[Motr] run mode applied: {mode:?}");
}

/// 启动时按 user.json 里的值应用一次，并返回应用的模式（见 lib.rs setup）
pub fn init(app: &AppHandle) -> AppRunMode {
    apply_config(app)
}

/// 读 user.json 里的运行模式并应用
fn apply_config(app: &AppHandle) -> AppRunMode {
    let mode = config_manager::run_mode(app);
    apply(app, mode);
    mode
}

/// 启动时是否要显示窗口：托盘应用模式下不显示（只留菜单栏图标）。
///
/// 窗口在 tauri.conf.json 里设的是 `visible: false`，由这里决定要不要 show，
/// 所以不会像"先显示再隐藏"那样闪一下。
pub fn window_visible_at_startup(mode: AppRunMode) -> bool {
    !matches!(mode, AppRunMode::Tray)
}

/// 该模式是否需要托盘（HideTray 不创建）
fn tray_wanted(mode: AppRunMode) -> bool {
    !matches!(mode, AppRunMode::HideTray)
}

/// 该模式是否要隐藏 Dock 图标（仅 Tray）
fn dock_hidden(mode: AppRunMode) -> bool {
    matches!(mode, AppRunMode::Tray)
}

fn apply_tray(app: &AppHandle, mode: AppRunMode) {
    if !tray_wanted(mode) {
        if app.remove_tray_by_id(tray::TRAY_ID).is_some() {
            log::info!("[Motr] HideTray：已移除托盘图标");
        }
        return;
    }

    if app.tray_by_id(tray::TRAY_ID).is_some() {
        return;
    }

    match tray::init(app) {
        // 新建的托盘要重新挂右键菜单（原项目在 TrayManager.init 里 setupMenu）
        Ok(()) => menu::init(app),
        Err(err) => log::error!("[Motr] 创建托盘失败: {err}"),
    }
}

#[cfg(target_os = "macos")]
fn apply_dock(app: &AppHandle, mode: AppRunMode) {
    use tauri::ActivationPolicy;

    let policy = if dock_hidden(mode) {
        ActivationPolicy::Accessory
    } else {
        ActivationPolicy::Regular
    };

    if let Err(err) = app.set_activation_policy(policy) {
        log::error!("[Motr] 设置 Dock 显隐失败: {err}");
    }
}

/// 设置页保存后应用（命令在 commands.rs）：按最新的 user.json 重新应用运行模式，
/// 并把「托盘应用」模式的窗口当场收起来
pub(crate) fn apply_from_config(app: &AppHandle) -> AppRunMode {
    let mode = apply_config(app);

    // 这种模式下它就是菜单栏应用，窗口留着会挡在设置页上。
    // 启动时不走这里，窗口可见性由 lib.rs 按模式决定。
    if matches!(mode, AppRunMode::Tray) {
        crate::window::hide_main_window(app);
    }

    mode
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 数值与前端 constants.js 的 APP_RUN_MODE 对齐
    #[test]
    fn run_mode_values_match_frontend() {
        assert_eq!(AppRunMode::from_u8(1), AppRunMode::Standard);
        assert_eq!(AppRunMode::from_u8(2), AppRunMode::Tray);
        assert_eq!(AppRunMode::from_u8(3), AppRunMode::HideTray);
        assert_eq!(AppRunMode::Standard as u8, 1);
        assert_eq!(AppRunMode::Tray as u8, 2);
        assert_eq!(AppRunMode::HideTray as u8, 3);
    }

    /// 未知值回落到默认（Standard），和原项目默认值一致
    #[test]
    fn unknown_run_mode_falls_back_to_standard() {
        assert_eq!(AppRunMode::from_u8(0), AppRunMode::Standard);
        assert_eq!(AppRunMode::from_u8(9), AppRunMode::Standard);
    }

    /// 只有 HideTray 不要托盘，只有 Tray 隐藏 Dock
    #[test]
    fn per_mode_targets() {
        assert!(tray_wanted(AppRunMode::Standard));
        assert!(tray_wanted(AppRunMode::Tray));
        assert!(!tray_wanted(AppRunMode::HideTray));

        assert!(!dock_hidden(AppRunMode::Standard));
        assert!(dock_hidden(AppRunMode::Tray));
        assert!(!dock_hidden(AppRunMode::HideTray));
    }

    /// 托盘应用模式启动时不显示窗口，其余两种显示
    #[test]
    fn startup_window_visibility() {
        assert!(window_visible_at_startup(AppRunMode::Standard));
        assert!(!window_visible_at_startup(AppRunMode::Tray));
        assert!(window_visible_at_startup(AppRunMode::HideTray));
    }
}
