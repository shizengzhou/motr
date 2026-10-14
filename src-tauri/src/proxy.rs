use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use crate::config_manager::{Config, Proxy, ProxyScope};

/// 代理是否该交给 aria2：开了、填了服务器、且 scope 勾了「下载」
fn should_use_for_download(proxy: &Proxy) -> bool {
    proxy.enable
        && !proxy.server.is_empty()
        && proxy
            .scope
            .iter()
            .any(|scope| scope == ProxyScope::Download.as_str())
}

/// user.json 的 `proxy` -> aria2 的 `all-proxy` / `no-proxy`。
///
/// 对齐原项目 `ConfigManager.fixSystemConfig` / `Application.watchProxyChange`：
/// `all-proxy` 就是用户填的 `[http://][USER:PASSWORD@]HOST[:PORT]`（凭证写在 URL 里，
/// aria2 认这种写法），`no-proxy` 是 bypass 列表。
///
/// 一处刻意的差别：原项目关掉代理时**不清理**这两个键，旧值会留在 system.json 里，
/// 于是 aria2 继续走代理；这里关掉就写空串，让它立刻回到直连。
pub fn engine_options(proxy: &Proxy) -> [(&'static str, String); 2] {
    if should_use_for_download(proxy) {
        [
            ("all-proxy", proxy.server.clone()),
            ("no-proxy", proxy.bypass.clone()),
        ]
    } else {
        [("all-proxy", String::new()), ("no-proxy", String::new())]
    }
}

/// 启动时把上面这份映射写进 system.json，并同步进即将喂给 aria2 的 `Config`
///（原项目对应 `ConfigManager.fixSystemConfig`，同样是在起引擎之前完成）。
pub fn apply(app: &AppHandle, config: &mut Config) {
    let options = engine_options(&config.user_config.proxy);
    let all_proxy = options[0].1.clone();
    let no_proxy = options[1].1.clone();

    config.system_config.all_proxy.clone_from(&all_proxy);
    config.system_config.no_proxy.clone_from(&no_proxy);

    let store = match app.store("system.json") {
        Ok(store) => store,
        Err(err) => {
            log::warn!("[Motr] open system.json failed: {err}");
            return;
        }
    };

    // 先重读磁盘：前端（JS 侧的 LazyStore）会直接写这个文件，缓存里可能还是启动时的
    // 旧值 —— 而下面那次 `save()` 是把**整个缓存**写回文件，不重读就会把前端
    // 刚写的键覆盖掉
    crate::config_manager::reload_store(app, "system.json");

    let stored = |key: &str| {
        store
            .get(key)
            .and_then(|current| current.as_str().map(str::to_string))
            .unwrap_or_default()
    };

    if stored("all-proxy") == all_proxy && stored("no-proxy") == no_proxy {
        return;
    }

    store.set("all-proxy", Value::from(all_proxy));
    store.set("no-proxy", Value::from(no_proxy));

    if let Err(err) = store.save() {
        log::warn!("[Motr] save system.json failed: {err}");
    } else {
        log::info!("[Motr] 代理设置已同步到 aria2");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(enable: bool, server: &str, bypass: &str, scope: &[&str]) -> Proxy {
        Proxy {
            enable,
            server: server.to_string(),
            bypass: bypass.to_string(),
            scope: scope.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn empty() -> [(&'static str, String); 2] {
        [("all-proxy", String::new()), ("no-proxy", String::new())]
    }

    /// 开了代理、填了服务器、scope 含「下载」-> 交给 aria2
    #[test]
    fn maps_enabled_proxy_to_engine_options() {
        let configured = proxy(
            true,
            "http://127.0.0.1:7890",
            "localhost,127.0.0.1",
            &["download"],
        );

        assert_eq!(
            engine_options(&configured),
            [
                ("all-proxy", "http://127.0.0.1:7890".to_string()),
                ("no-proxy", "localhost,127.0.0.1".to_string()),
            ]
        );
    }

    /// 没开 / 没服务器 / scope 不含「下载」/ scope 为空 -> 清空两个键
    #[test]
    fn clears_engine_proxy_when_not_applicable() {
        let cases = [
            proxy(false, "http://127.0.0.1:7890", "localhost", &["download"]),
            proxy(true, "", "localhost", &["download"]),
            proxy(true, "http://127.0.0.1:7890", "localhost", &["update-app"]),
            proxy(true, "http://127.0.0.1:7890", "localhost", &[]),
        ];

        for case in cases {
            assert_eq!(
                engine_options(&case),
                empty(),
                "enable={} server={:?} scope={:?} 不该走代理",
                case.enable,
                case.server,
                case.scope
            );
        }
    }

    /// 只有「下载」这一个 scope 会写进引擎参数，其余 scope 不影响 aria2
    #[test]
    fn only_download_scope_reaches_engine() {
        let mut configured = proxy(true, "http://127.0.0.1:7890", "", &["download"]);

        configured.scope.push(ProxyScope::UpdateApp.as_str().to_string());
        assert_eq!(
            engine_options(&configured)[0].1,
            "http://127.0.0.1:7890",
            "多勾一个 scope 不该改变引擎参数"
        );

        configured.scope = vec![ProxyScope::UpdateTrackers.as_str().to_string()];
        assert_eq!(engine_options(&configured), empty());
    }
}
