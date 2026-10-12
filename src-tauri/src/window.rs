use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_window_state::{StateFlags, WindowExt};

use crate::config_manager::{self, AppRunMode, auto_hide_window_enabled};

/// 失焦时是否该隐藏窗口
///
/// 两种情况之一即可：设置里开了「自动隐藏窗口」，或者运行模式是**托盘应用** ——
/// 托盘模式下 Dock 里没有图标，窗口就该像菜单栏应用一样失焦即收起来。
fn hide_on_blur(auto_hide_window: bool, run_mode: AppRunMode) -> bool {
    auto_hide_window || matches!(run_mode, AppRunMode::Tray)
}

/// 现读 user.json 判断（设置页改动后立即生效，不需要通知）
fn should_hide_on_blur(app: &AppHandle) -> bool {
    hide_on_blur(auto_hide_window_enabled(app), config_manager::run_mode(app))
}

/// 是否已进入退出流程（对应原项目 `WindowManager` 的 `willQuit`）。
///
/// 置位后关闭请求不再拦成「隐藏」，而是放行真关窗口，这样 ⌘Q / 菜单里的退出才能退掉。
static WILL_QUIT: AtomicBool = AtomicBool::new(false);

/// 退出流程开始时调用（`RunEvent::ExitRequested`）
pub fn mark_quitting() {
    WILL_QUIT.store(true, Ordering::SeqCst);
}

fn is_quitting() -> bool {
    WILL_QUIT.load(Ordering::SeqCst)
}

/// 窗口状态里保存/恢复哪些东西。
///
/// 只保留尺寸和位置 —— 原项目存的也是 `window.getBounds()` 的 x/y/width/height，
/// 不记全屏/最大化（退出时若在最大化状态，插件存的是最大化之前的尺寸，回来就是那个尺寸）。
///
/// 故意不含 `VISIBLE`：恢复时绝不顺手把窗口显示出来（托盘应用模式要一直藏着，
/// 显示与否由 lib.rs 里运行模式那段决定）。
/// 也不含 `DECORATIONS`：装饰由 tauri.conf.json 决定（macOS 上是 Overlay 标题栏）。
pub(crate) const STATE_FLAGS: StateFlags = StateFlags::SIZE.union(StateFlags::POSITION);

/// 「恢复上次退出时的窗口大小和位置」。
///
/// 对齐原项目 `WindowManager.getPageBounds` + `openWindow` 里那句 `window.setBounds(bounds)`：
/// 开了这个开关才用上次的 bounds，关着就用 tauri.conf.json 里的默认尺寸（1024×768）。
///
/// 窗口状态存在 `window-state` 插件自己的文件里（`.window-state.json`），
/// 由它在 `RunEvent::Exit` 时写入，这里只负责**按开关决定要不要读回来** ——
/// 插件默认是无条件自动恢复的，lib.rs 里用 `skip_initial_state("main")` 把它关掉了。
pub fn restore_state(app: &AppHandle) {
    if !config_manager::keep_window_state(app) {
        log::info!("[Motr] 未开启「恢复窗口状态」，使用默认窗口尺寸");
        return;
    }

    let Some(window) = app.get_webview_window("main") else {
        log::warn!("[Motr] 找不到主窗口，无法恢复窗口状态");
        return;
    };

    // 插件会校验保存的位置是否还在当前显示器范围内，越界的位置不会照搬
    match window.restore_state(STATE_FLAGS) {
        Ok(()) => log::info!("[Motr] 已恢复上次退出时的窗口大小和位置"),
        Err(err) => log::warn!("[Motr] 恢复窗口状态失败: {err}"),
    }
}

/// 「自动隐藏窗口」：窗口一失去焦点就隐藏。
///
/// 对齐原 Electron 版的 `WindowManager.onWindowBlur` -> `window.hide()`：
/// 监听的是应用级的 window blur（不是窗口内部某个元素的 blur），
/// 触发后不做任何额外判断（不查可见/最小化/全屏状态，也没有防抖）。
///
/// 开关值每次失焦时从 user.json 现读，所以设置页保存后立即生效。
pub fn init(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("[Motr] 找不到主窗口，自动隐藏窗口功能不可用");
        return;
    };

    let handle = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            if !should_hide_on_blur(&handle) {
                return;
            }

            hide_main_window(&handle);
        }
        // 点关闭按钮 = 隐藏窗口，不退出（对齐原项目 WindowManager.handleWindowClose：
        // index 页写死 bindCloseToHide，只有 willQuit 时才真关）
        WindowEvent::CloseRequested { api, .. } => {
            if is_quitting() {
                return;
            }

            api.prevent_close();

            let Some(window) = handle.get_webview_window("main") else {
                return;
            };

            // 全屏时先退出全屏再隐藏（原项目同一处逻辑）
            if window.is_fullscreen().unwrap_or(false) {
                let _ = window.set_fullscreen(false);
            }

            hide_main_window(&handle);
        }
        _ => {}
    });
}

/// 隐藏主窗口（失焦自动隐藏、点关闭按钮、切到「托盘应用」都用它）
pub(crate) fn hide_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    if let Err(err) = window.hide() {
        log::error!("[Motr] 隐藏窗口失败: {err}");
    }
}

/// 显示并聚焦主窗口（托盘左键、菜单里的 application:show、再启动一次、点 Dock 图标都用它）
pub(crate) fn show_main_window(app: &AppHandle) {
    // macOS: 应用被 hide 掉时窗口不会出现，需要先 unhide
    #[cfg(target_os = "macos")]
    let _ = app.show();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 失焦隐藏的判据：设置开着时任何模式都隐藏；设置关着时只有「托盘应用」模式隐藏
    #[test]
    fn hide_on_blur_rules() {
        for mode in [AppRunMode::Standard, AppRunMode::Tray, AppRunMode::HideTray] {
            assert!(hide_on_blur(true, mode), "设置开着时 {mode:?} 也该隐藏");
        }

        assert!(
            hide_on_blur(false, AppRunMode::Tray),
            "托盘应用模式必须自动隐藏窗口"
        );
        assert!(!hide_on_blur(false, AppRunMode::Standard));
        assert!(!hide_on_blur(false, AppRunMode::HideTray));
    }

    /// 恢复的只有尺寸和位置（不多不少，对齐原项目存的 getBounds）
    #[test]
    fn state_flags_cover_size_and_position() {
        assert!(STATE_FLAGS.contains(StateFlags::SIZE));
        assert!(STATE_FLAGS.contains(StateFlags::POSITION));
        // 除了这两位没有别的（StateFlags 没有实现 PartialEq，比 bits）
        assert_eq!(
            STATE_FLAGS.bits(),
            StateFlags::SIZE.union(StateFlags::POSITION).bits()
        );
    }

    /// 这几项都不能恢复：`VISIBLE` 会让托盘应用模式一启动就弹出窗口，
    /// `DECORATIONS` 会覆盖 tauri.conf.json 里的 Overlay 标题栏设置，
    /// `MAXIMIZED` / `FULLSCREEN` 原项目也不恢复
    #[test]
    fn state_flags_exclude_visible_decorations_and_window_mode() {
        for flag in [
            StateFlags::VISIBLE,
            StateFlags::DECORATIONS,
            StateFlags::MAXIMIZED,
            StateFlags::FULLSCREEN,
        ] {
            assert!(!STATE_FLAGS.contains(flag), "{flag:?} 不该被恢复");
        }
    }
}
