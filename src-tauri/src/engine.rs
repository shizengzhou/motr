use std::{collections::HashMap, path::Path, sync::Mutex};

use tauri::{AppHandle, Manager};
use tauri_plugin_shell::{
    ShellExt,
    process::{CommandChild, CommandEvent},
};

use crate::{
    config_manager::Config,
    context::{get_aria2_conf_path, get_session_path},
};

#[derive(Default)]
pub struct EngineChild(pub Mutex<Option<CommandChild>>);

pub fn start(app_handle: &AppHandle, config: &Config) -> Result<(), String> {
    if app_handle
        .state::<EngineChild>()
        .0
        .lock()
        .unwrap()
        .is_some()
    {
        return Ok(());
    }

    let args = get_start_args(app_handle, config);
    let (mut rx, child) = app_handle
        .shell()
        .sidecar("aria2c")
        .map_err(|err| err.to_string())?
        .args(args)
        .spawn()
        .map_err(|err| err.to_string())?;
    *app_handle.state::<EngineChild>().0.lock().unwrap() = Some(child);

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    log::info!(
                        "[Motr] engine stdout===> {}",
                        String::from_utf8_lossy(&line)
                    )
                }
                CommandEvent::Stderr(line) => {
                    log::warn!(
                        "[Motr] engine stderr===> {}",
                        String::from_utf8_lossy(&line)
                    )
                }
                CommandEvent::Terminated(payload) => {
                    log::info!(
                        "[Motr] engine terminated: code={:?} signal={:?}",
                        payload.code,
                        payload.signal
                    );
                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}

pub fn stop(app_handle: &AppHandle) {
    log::info!("[Motr] engine.stop");

    if let Some(child) = app_handle.state::<EngineChild>().0.lock().unwrap().take() {
        if let Err(e) = child.kill() {
            log::warn!("[Motr] engine.stop failed: {e}");
        }
    }
}

pub fn get_start_args(app_handle: &AppHandle, config: &Config) -> Vec<String> {
    let conf_path = get_aria2_conf_path(app_handle);
    println!("conf_path: {conf_path}");
    let session_path = get_session_path(app_handle);
    let session_is_exit = Path::new(&session_path).exists();

    let mut result = vec![
        format!("--conf-path={conf_path}"),
        format!("--save-session={session_path}"),
    ];
    if session_is_exit {
        result.push(format!("--input-file={session_path}"));
    }

    let mut extra_config: HashMap<String, serde_json::Value> =
        serde_json::to_value(&config.system_config)
            .unwrap()
            .as_object()
            .cloned()
            .unwrap()
            .into_iter()
            .collect();
    let keep_speeding = config.user_config.keep_seeding;
    let seed_ratio = config.system_config.seed_ratio;
    if keep_speeding || seed_ratio == 0.0 {
        extra_config.insert("seed-ratio".to_string(), serde_json::Value::from(0));
        extra_config.remove("seed-time");
    }
    println!("extra_config {:?}", extra_config);

    let extra = transform_config(&extra_config);
    result.extend(extra);

    result
}

pub fn transform_config(config: &HashMap<String, serde_json::Value>) -> Vec<String> {
    config
        .iter()
        .filter_map(|(k, v)| match v {
            // serde_json::Value 的 Display 是按 JSON 语法输出的，字符串会带引号（"abc"）。
            // 这些参数是直接作为 argv 交给 aria2 的（不经过 shell，引号不会被剥掉），
            // 所以字符串必须取原始内容，否则 --dht-file-path="/Users/..." 会被 aria2
            // 当成相对路径，在进程 cwd 下创建出名为 `"` 的目录
            serde_json::Value::String(s) if s.is_empty() => None,
            serde_json::Value::String(s) => Some(format!("--{k}={s}")),
            // number / bool / null 的 Display 本身不带引号
            _ => Some(format!("--{k}={v}")),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn transform_config_keeps_string_values_unquoted() {
        let mut config = HashMap::new();
        config.insert(
            "dht-file-path".to_string(),
            json!("/Users/me/Library/Application Support/com.shizeng.Motr/dht.dat"),
        );
        config.insert("dir".to_string(), json!("/Users/me/Downloads"));
        config.insert("user-agent".to_string(), json!("Motr/1.0"));
        config.insert("continue".to_string(), json!(true));
        config.insert("rpc-listen-port".to_string(), json!(16800));
        config.insert("all-proxy".to_string(), json!(""));

        let mut args = transform_config(&config);
        args.sort();

        assert_eq!(
            args,
            vec![
                "--continue=true".to_string(),
                "--dht-file-path=/Users/me/Library/Application Support/com.shizeng.Motr/dht.dat"
                    .to_string(),
                "--dir=/Users/me/Downloads".to_string(),
                "--rpc-listen-port=16800".to_string(),
                "--user-agent=Motr/1.0".to_string(),
            ]
        );
    }
}
