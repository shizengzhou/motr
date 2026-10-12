use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::config_manager;

/// 「开机自动启动」：让系统里的登录项与 user.json 里的 open-at-login 保持一致。
///
/// 对应原 Electron 版的 `AutoLaunchManager`（`app.setLoginItemSettings`）：
/// 先读系统当前状态，需要变才动它（原项目的 `enable()` 也是先查再设）。
///
/// 由设置页保存后调用（commands::apply_auto_launch）。
///
/// 与原项目一致：**只在配置变化时**改写登录项，启动时不做反向同步 ——
/// 否则用户刚在「系统设置 → 登录项」里手动关掉，一启动又被写回去。
pub fn apply(app: &AppHandle) {
    let wanted = config_manager::open_at_login(app);
    let manager = app.autolaunch();

    let current = match manager.is_enabled() {
        Ok(current) => current,
        Err(err) => {
            log::error!("[Motr] 读取开机自启状态失败: {err}");
            return;
        }
    };

    if !needs_update(current, wanted) {
        return;
    }

    let result = if wanted {
        manager.enable()
    } else {
        manager.disable()
    };

    match result {
        Ok(()) => log::info!("[Motr] 开机自启已{}", if wanted { "开启" } else { "关闭" }),
        Err(err) => log::error!("[Motr] 设置开机自启失败: {err}"),
    }
}

/// 系统登录项的状态与配置不一致时才需要改
fn needs_update(current: bool, wanted: bool) -> bool {
    current != wanted
}

/// 本次启动是否由「开机自启」拉起
///
/// 参数名与原项目 `LOGIN_SETTING_OPTIONS` 一致（`--opened-at-login=1`），
/// 插件会把它写进登录项，所以开机自启和被手动启动可以区分开。
pub fn opened_at_login() -> bool {
    has_opened_at_login_arg(std::env::args())
}

fn has_opened_at_login_arg<I: Iterator<Item = String>>(args: I) -> bool {
    args.into_iter().any(|arg| arg == "--opened-at-login=1")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 状态一致时不重复设置
    #[test]
    fn only_updates_when_state_differs() {
        assert!(!needs_update(true, true), "都开着，不用动");
        assert!(!needs_update(false, false), "都关着，不用动");
        assert!(needs_update(false, true), "要开");
        assert!(needs_update(true, false), "要关");
    }

    /// 认得开机自启带的参数（与原项目 LOGIN_SETTING_OPTIONS 相同）
    #[test]
    fn detects_opened_at_login_arg() {
        let args = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();

        assert!(has_opened_at_login_arg(
            args(&[
                "/Applications/Motr.app/Contents/MacOS/Motr",
                "--opened-at-login=1"
            ])
            .into_iter()
        ));
        assert!(!has_opened_at_login_arg(
            args(&["/Applications/Motr.app/Contents/MacOS/Motr"]).into_iter()
        ));
        assert!(!has_opened_at_login_arg(
            args(&["/Applications/Motr.app/Contents/MacOS/Motr", "--other"]).into_iter()
        ));
    }
}
