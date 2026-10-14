use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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

/// 是否已经在退出流程里。除了窗口关闭处理，重置会话那边也用它 ——
/// 退出途中别再把手动停掉的引擎拉起来，否则会留一个孤儿 aria2
pub(crate) fn is_quitting() -> bool {
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

    #[cfg(target_os = "macos")]
    activate_app(app);

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// 「用户主动重启后把窗口带到前台」的标记文件名
///
/// 重启是 `app.restart()` → 重新 exec 同一个二进制，应用数据目录不变，
/// 所以用文件把这个意图传给下一个进程（也省得给重启加命令行参数）
const RESTART_FOCUS_FILE: &str = "focus-after-restart";

/// 标记只在刚重启起来那一下有效：中途崩掉或放了很久的残留标记不该让窗口突然冒出来
const RESTART_FOCUS_TTL: Duration = Duration::from_secs(60);

/// 「本次启动是用户主动重启，聚焦这一枪还没补」的进程内标记
///
/// 为什么要分两步：`setup` 是 Tauri 在 `RunEvent::Ready` 处理里同步调的，
/// 那一刻窗口刚创建、应用尚未激活，`set_focus()` 常常不算数（启动期焦点也容易
/// 被系统或别的程序抢走）。所以 `setup` 只负责让窗口**可见**，聚焦留到这里，
/// 由事件循环在应用真正跑起来之后延迟补一枪 —— 见 lib.rs 的 `RunEvent::Ready`
static PENDING_FOCUS: AtomicBool = AtomicBool::new(false);

fn restart_focus_path(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join(RESTART_FOCUS_FILE))
}

/// 重启前记下"这次是用户主动重启"（设置里改端口、装完更新都走 `commands::relaunch`）
pub(crate) fn mark_focus_after_restart(app: &AppHandle) {
    let Some(path) = restart_focus_path(app) else {
        return;
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            log::warn!("[Motr] 创建重启标记目录失败: {err}");

            return;
        }
    }

    if let Err(err) = std::fs::write(&path, b"1") {
        log::warn!("[Motr] 写重启标记失败: {err}");
    } else {
        // warn 级（release 里 info 会被编掉）：排查"重启后窗口怎么没到前台"时，
        // 第一件事就是确认这次重启有没有打过标记
        log::warn!("[Motr] 已标记：本次重启后要显示并聚焦主窗口");
    }
}

/// 启动时读取并消费重启标记：是"用户主动重启"且标记还新鲜就返回 `true`
///
/// 同时把 [`PENDING_FOCUS`] 置位，供事件循环里那次延迟聚焦取用
pub(crate) fn take_focus_after_restart(app: &AppHandle) -> bool {
    let Some(path) = restart_focus_path(app) else {
        return false;
    };

    let Ok(meta) = std::fs::metadata(&path) else {
        return false;
    };

    // 无论判定结果如何都不留残留
    let _ = std::fs::remove_file(&path);

    let age = meta.modified().ok().and_then(|time| time.elapsed().ok());

    if marker_is_fresh(age) {
        PENDING_FOCUS.store(true, Ordering::Relaxed);

        true
    } else {
        false
    }
}

/// 取走"还需要补一次聚焦"的标记（取走即清零，只会聚焦一次）
pub(crate) fn focus_after_restart_pending() -> bool {
    PENDING_FOCUS.swap(false, Ordering::Relaxed)
}

/// 标记是否够新（纯判断，方便测）
fn marker_is_fresh(age: Option<Duration>) -> bool {
    matches!(age, Some(age) if age <= RESTART_FOCUS_TTL)
}

/// macOS：把应用激活到前台。
///
/// 为什么不能只靠 `window.set_focus()`：tao 内部用的是
/// `activateIgnoringOtherApps:YES`，而这个 API 从 macOS 14 起被系统忽略
/// （协作式激活模型）—— 现象就是"窗口显示出来了但压在别的应用后面，
/// 必须手动点一下 Dock 才行"（点 Dock 是一次用户发起的激活，永远有效）。
///
/// 这里优先用现代 API `NSApplication.activate()`（macOS 14+），
/// 老系统回落到 `activateIgnoringOtherApps:`。
#[cfg(target_os = "macos")]
fn activate_app(app: &AppHandle) {
    // AppKit 的激活调用必须在主线程；不在主线程（比如异步重试任务里）就派发回去
    if objc2::MainThreadMarker::new().is_some() {
        activate_app_on_main_thread();
    } else {
        let _ = app.run_on_main_thread(activate_app_on_main_thread);
    }
}

#[cfg(target_os = "macos")]
fn activate_app_on_main_thread() {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    unsafe {
        let Some(class) = AnyClass::get(c"NSApplication") else {
            return;
        };

        let app: *mut AnyObject = msg_send![class, sharedApplication];

        if app.is_null() {
            return;
        }

        // macOS 14+：NSApplication.activate()；老系统没有这个选择子，回落到旧 API
        let modern = objc2::sel!(activate);
        if msg_send![app, respondsToSelector: modern] {
            let _: () = msg_send![app, activate];
        } else {
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
    }
}

/// 重启后把窗口带回来并聚焦：应用刚起来时激活可能被系统忽略、焦点也可能被抢，
/// 所以隔一小段时间重试，直到窗口真的成为 key window（`is_focused`）为止。
///
/// 拿到焦点就收手 —— 不会跟用户后面手动切换窗口的行为打架；用完次数也放弃，
/// 只记一条告警
pub(crate) fn focus_main_window_until_focused(app: &AppHandle) {
    const INTERVAL: Duration = Duration::from_millis(400);
    const MAX_ATTEMPTS: u32 = 12; // 约 5 秒

    let handle = app.clone();

    tauri::async_runtime::spawn(async move {
        for attempt in 1..=MAX_ATTEMPTS {
            tokio::time::sleep(INTERVAL).await;

            let focused = handle
                .get_webview_window("main")
                .and_then(|window| window.is_focused().ok())
                .unwrap_or(false);

            if focused {
                // 用 warn 级：release 构建里 log 的编译期等级会把 info 整条编掉
                log::warn!("[Motr] 主动重启：主窗口已获得焦点（第 {attempt} 次尝试）");

                return;
            }

            // 显示/激活都要在主线程上做（AppKit 不是线程安全的）
            let target = handle.clone();
            let _ = handle.run_on_main_thread(move || {
                show_main_window(&target);
            });
        }

        log::warn!("[Motr] 主动重启：重试 {MAX_ATTEMPTS} 次后主窗口仍未获得焦点");
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 重启标记只在刚重启那一下有效：太久、或者读不到时间，都不认
    #[test]
    fn restart_marker_must_be_fresh() {
        assert!(marker_is_fresh(Some(Duration::from_secs(0))));
        assert!(marker_is_fresh(Some(RESTART_FOCUS_TTL)));

        assert!(!marker_is_fresh(None));
        assert!(!marker_is_fresh(Some(
            RESTART_FOCUS_TTL + Duration::from_secs(1)
        )));
    }

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
