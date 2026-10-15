use std::{
    collections::HashMap,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use tauri::{AppHandle, Manager};
use tauri_plugin_shell::{
    ShellExt,
    process::{CommandChild, CommandEvent},
};

use crate::{
    commands,
    config_manager::{Config, SystemConfig, UserConfig},
    context::{get_aria2_conf_path, get_session_path},
    window,
};

#[derive(Default)]
pub struct EngineChild(pub Mutex<Option<CommandChild>>);

/// 「重置下载会话记录」时"停引擎"和"删会话文件"之间的等待。
///
/// 原项目是 3 秒（它那边 SIGTERM，aria2 退出时会把会话写回文件，得等写完再删）；
/// 我们这边 `SharedChild::kill()` 是 SIGKILL，进程直接消失、不会回写，端口也是
/// 内核立刻释放的，所以只留 1 秒 —— 够让进程真的退出，又不会把这个动作拖长，
/// 也少一个"用户正好在这几秒里退出应用"的窗口
const SESSION_RESET_DELAY: Duration = Duration::from_secs(1);

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

    // 上次遗留的引擎（开发时被强杀、应用崩溃留下的孤儿）会占着 RPC 端口，
    // 不先收掉它的话，下面新起的这个引擎会绑定失败直接退出
    reap_stale_engine(app_handle);
    warn_if_rpc_port_taken(config);

    let args = get_start_args(app_handle, config);
    let (mut rx, child) = app_handle
        .shell()
        .sidecar("aria2c")
        .map_err(|err| err.to_string())?
        .args(args)
        .spawn()
        .map_err(|err| err.to_string())?;

    let pid = child.pid();
    *app_handle.state::<EngineChild>().0.lock().unwrap() = Some(child);
    write_pid_file(app_handle, pid);

    let handle = app_handle.clone();
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
                    // 死掉的子进程必须从状态里摘掉：留着的话下次 start() 会以为
                    // 引擎还在，直接 return，于是永远起不来新引擎
                    let still_ours = {
                        // 先把 State 绑到变量上：临时值会在语句结束时就释放
                        let engine_state = handle.state::<EngineChild>();
                        let mut state = engine_state.0.lock().unwrap();

                        if state.as_ref().map(CommandChild::pid) == Some(pid) {
                            *state = None;
                            true
                        } else {
                            false
                        }
                    };
                    remove_pid_file(&handle, pid);

                    match payload.code {
                        // 正常退出（我们自己 stop 掉时是 signal=Some(9)，code=None）
                        Some(0) | None => log::info!(
                            "[Motr] engine terminated: code={:?} signal={:?}",
                            payload.code,
                            payload.signal
                        ),
                        // 非 0 退出：最常见的就是 RPC 端口被上次遗留的引擎占着
                        Some(code) => log::error!(
                            "[Motr] 引擎异常退出（code={code}{}）：多半是 RPC 端口被占用 —— \
                             上一次遗留的 aria2 还在跑。先把它结束掉，再重启应用",
                            if still_ours { "" } else { "，且已被别处接管" }
                        ),
                    }

                    break;
                }
                _ => {}
            }
        }
    });

    Ok(())
}

/// 引擎 PID 文件的路径：应用数据目录下的 `engine.pid`（与上游 `getEnginePidPath()` 同位置）。
///
/// 用途是下次启动时认出"上次没退干净的引擎" —— 开发时 `tauri dev` 重建会强杀应用
/// （`RunEvent::Exit` 不跑，shell 插件收不到子进程），aria2 就变成孤儿活下来，
/// 占着 RPC 端口，新引擎起不来而界面还连着它，最难查的就是这个状态
fn pid_file_path(app_handle: &AppHandle) -> Option<PathBuf> {
    app_handle
        .path()
        .app_data_dir()
        .ok()
        .map(|dir| dir.join("engine.pid"))
}

fn read_pid_file(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

fn write_pid_file(app_handle: &AppHandle, pid: u32) {
    let Some(path) = pid_file_path(app_handle) else {
        return;
    };

    if let Err(err) = std::fs::write(&path, pid.to_string()) {
        log::warn!("[Motr] 写引擎 PID 文件失败（{}）: {err}", path.display());
    }
}

/// 只删"记的就是这个 pid"的那份，免得把新引擎刚写进去的删掉
fn remove_pid_file(app_handle: &AppHandle, pid: u32) {
    let Some(path) = pid_file_path(app_handle) else {
        return;
    };

    if read_pid_file(&path) != Some(pid) {
        return;
    }

    if let Err(err) = std::fs::remove_file(&path) {
        log::warn!("[Motr] 删引擎 PID 文件失败（{}）: {err}", path.display());
    }
}

/// 认出上次遗留的引擎并收掉它，给新引擎让出 RPC 端口。
///
/// **只在确认那个 pid 现在跑的就是 aria2 时才动手**（pid 会被系统回收，万一里头
/// 已经是别人的进程，杀了就是事故）。Windows 上没法便宜地核对命令行，那边只记一条
/// 日志，留给手动处理（`taskkill /PID <pid> /F`）
fn reap_stale_engine(app_handle: &AppHandle) {
    let Some(path) = pid_file_path(app_handle) else {
        return;
    };
    let Some(pid) = read_pid_file(&path) else {
        return;
    };

    #[cfg(windows)]
    {
        log::warn!(
            "[Motr] 发现上次遗留的引擎 PID 文件（{pid}），Windows 上不自动收：\
             确认后请手动结束该进程并删掉 {}",
            path.display()
        );
    }

    #[cfg(not(windows))]
    {
        if !is_aria2_process(pid) {
            // 进程没了（正常的），或者 pid 已经被别人占用（不正常的）
            if !is_process_alive(pid) {
                let _ = std::fs::remove_file(&path);
            } else {
                log::warn!(
                    "[Motr] 引擎 PID 文件里记着 {pid}，但它现在不是 aria2，不动它；\
                     确认没问题后删掉 {} 即可",
                    path.display()
                );
            }

            return;
        }

        log::warn!("[Motr] 发现上次遗留的引擎（pid {pid}）仍占着端口，先收掉它");

        match Command::new("kill").arg("-9").arg(pid.to_string()).status() {
            Ok(status) if status.success() => {
                let _ = std::fs::remove_file(&path);
            }
            Ok(status) => log::warn!("[Motr] 结束遗留引擎 {pid} 失败（kill 退出码 {status}）"),
            Err(err) => log::warn!("[Motr] 结束遗留引擎 {pid} 失败: {err}"),
        }
    }
}

/// 进程还在不在（`kill -0` 只做信号权限检查，不真发信号）
#[cfg(not(windows))]
fn is_process_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// 这个 pid 现在跑的是不是 aria2（`ps -p <pid> -o command=`）
#[cfg(not(windows))]
fn is_aria2_process(pid: u32) -> bool {
    let pid = pid.to_string();

    Command::new("ps")
        .args(["-p", &pid, "-o", "command="])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).contains("aria2c"))
        .unwrap_or(false)
}

/// RPC 端口上已经有人在监听？（只探测，不动手 —— 可能是用户自己在跑别的 aria2）
fn warn_if_rpc_port_taken(config: &Config) {
    let port = config.system_config.rpc_listen_port;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    if TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok() {
        log::warn!(
            "[Motr] RPC 端口 {port} 已经有服务在监听：可能是上次遗留的引擎。\
             本次启动的引擎可能起不来，界面会连到那个旧引擎上 —— \
             确认后结束那个进程（或删掉应用数据目录里的 engine.pid）再重启应用"
        );
    }
}

pub fn stop(app_handle: &AppHandle) {
    log::info!("[Motr] engine.stop");

    if let Some(child) = app_handle.state::<EngineChild>().0.lock().unwrap().take() {
        if let Err(e) = child.kill() {
            log::warn!("[Motr] engine.stop failed: {e}");
        }
    }
}

/// 「重置下载会话记录」：停引擎 → 删掉 `download.session` → 重新起引擎。
///
/// 对齐原项目 `Application.resetSession()`：先 `stopEngine()`，隔一会儿
/// `unlink(sessionPath)`，然后 `engine.start()`。
///
/// **为什么不能只删文件**：`resources/aria2.conf` 里有 `save-session-interval=10`，
/// 引擎每 10 秒就把当前任务重写回会话文件 —— 光删文件，十秒后它带着同样的任务
/// 又回来了（任务还在引擎内存里）。所以顺序必须是「先停引擎 → 再删 → 用空会话重启」。
///
/// 会话文件就是引擎启动参数里的 `--save-session` / `--input-file`
/// （见 [`get_start_args`]）：删掉之后再起引擎时 `--input-file` 不会被带上，
/// 于是不会把上次未完成的任务恢复回来；设置页那边同时会清掉本地任务记录。
///
/// 异步执行、立即返回：整个过程要等一会儿，不能让设置页的按钮卡住。
pub fn reset_session(app_handle: &AppHandle) {
    let app = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        stop(&app);
        tokio::time::sleep(SESSION_RESET_DELAY).await;

        let session_path = get_session_path(&app);

        match remove_session_file(&session_path) {
            Ok(true) => log::info!("[Motr] 已删除下载会话文件: {session_path}"),
            Ok(false) => log::info!("[Motr] 下载会话文件本来就不存在: {session_path}"),
            Err(err) => log::warn!("[Motr] 删除下载会话文件失败（{session_path}）: {err}"),
        }

        // 退出途中就别再把引擎拉起来了，否则会留一个孤儿 aria2 占着端口
        if window::is_quitting() {
            log::info!("[Motr] 应用正在退出，不再重启引擎");

            return;
        }

        match commands::get_app_config(app.clone()) {
            Ok(mut config) => {
                // 与启动时那套一样（lib.rs 的 setup）：先把 user.json 的代理映射
                // 进 system.json 与 Config，再起引擎
                crate::proxy::apply(&app, &mut config);

                match start(&app, &config) {
                    Ok(()) => log::info!(
                        "[Motr] 会话已重置：引擎按空会话重启（download.session 会在 10 秒内\
                         被写回一个空文件，那是 save-session-interval 在起作用）"
                    ),
                    Err(err) => log::error!("[Motr] 重置会话后重启引擎失败: {err}"),
                }
            }
            Err(err) => log::error!("[Motr] 重置会话后读配置失败: {err}"),
        }
    });
}

/// 删掉会话文件，返回是否真的删掉了一个。
/// 文件本来就不在也算成功（`Ok(false)`）—— 引擎没写过会话而已
fn remove_session_file(path: &str) -> std::io::Result<bool> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err),
    }
}

pub fn get_start_args(app_handle: &AppHandle, config: &Config) -> Vec<String> {
    let conf_path = get_aria2_conf_path(app_handle);
    let session_path = get_session_path(app_handle);
    let session_is_exit = Path::new(&session_path).exists();

    let mut result = vec![
        format!("--conf-path={conf_path}"),
        format!("--save-session={session_path}"),
    ];
    if session_is_exit {
        result.push(format!("--input-file={session_path}"));
    }

    let extra = transform_config(&engine_config(&config.user_config, &config.system_config));
    log::info!("[Motr] engine extra args: {} 项", extra.len());
    result.extend(extra);

    result
}

/// system.json 的内容 -> aria2 配置 map，再叠加上来自 user.json 的特判
///（原项目 `Engine.js` 也是在 system 配置上做同样的特判）：
///
/// - `keep-seeding`（或 seed-ratio 为 0）→ seed-ratio=0 并去掉 seed-time，等于一直做种
///
/// ⚠️ **不要往这里塞 `enable-upnp`**：随包的 aria2 构建没有编译 UPnP 支持
///（二进制里连 "upnp" 字符串都没有），传 `--enable-upnp=…` 会让它直接以
/// "unrecognized option" 退出 —— 引擎起不来，一个任务都下不了。
///
/// 原项目也不靠 aria2 做端口映射：它在 Electron 主进程里用 `@motrix/nat-api`
/// （UPnP IGD + NAT-PMP）自己 map/unmap `listen-port` 与 `dht-listen-port`。
/// 移植版的「启用 UPnP」要真正生效，需要在 Rust 侧接一个同类客户端，见
/// `src/shared/configKeys.js` 里 `enable-upnp` 的说明。
fn engine_config(
    user_config: &UserConfig,
    system_config: &SystemConfig,
) -> HashMap<String, serde_json::Value> {
    let mut extra_config: HashMap<String, serde_json::Value> =
        serde_json::to_value(system_config)
            .unwrap()
            .as_object()
            .cloned()
            .unwrap()
            .into_iter()
            .collect();

    if user_config.keep_seeding || system_config.seed_ratio == 0.0 {
        extra_config.insert("seed-ratio".to_string(), serde_json::Value::from(0));
        extra_config.remove("seed-time");
    }

    extra_config
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

    /// 引擎参数里绝不能出现 `enable-upnp`：随包的 aria2 没编译 UPnP 支持，
    /// 传了它就会以 "unrecognized option" 退出（原项目也是在 app 侧自己映射端口）
    #[test]
    fn never_passes_enable_upnp_to_engine() {
        let mut user = UserConfig::default();
        let system = SystemConfig::default();

        for enable_upnp in [true, false] {
            user.enable_upnp = enable_upnp;
            assert!(
                !engine_config(&user, &system).contains_key("enable-upnp"),
                "enable-upnp 不该进 aria2 参数"
            );
        }
    }

    /// 「持续做种」覆盖 seed-ratio 并去掉 seed-time
    #[test]
    fn keep_seeding_overrides_seed_options() {
        let mut user = UserConfig::default();
        let mut system = SystemConfig::default();
        system.seed_ratio = 2.0;

        assert_eq!(engine_config(&user, &system)["seed-ratio"], json!(2.0));

        user.keep_seeding = true;
        let result = engine_config(&user, &system);
        assert_eq!(result["seed-ratio"], json!(0));
        assert!(
            !result.contains_key("seed-time"),
            "持续做种时要去掉 seed-time"
        );
    }

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

    /// 读引擎 PID 文件：正常数字（带换行）、垃圾内容、文件不存在
    #[test]
    fn reads_pid_file() {
        let path = std::env::temp_dir().join("motr-engine-pid-test.pid");

        std::fs::write(&path, "12345\n").unwrap();
        assert_eq!(read_pid_file(&path), Some(12345));

        std::fs::write(&path, "not-a-pid").unwrap();
        assert_eq!(read_pid_file(&path), None, "垃圾内容应当当作没有");

        let _ = std::fs::remove_file(&path);
        assert_eq!(read_pid_file(&path), None, "文件不存在也不能报错");
    }

    /// 重置会话时删文件：删掉了返回 true；本来就不在返回 false 而不是报错
    ///（引擎没写过会话是正常情况，不该记一条 warn）
    #[test]
    fn removes_session_file_once() {
        let path = std::env::temp_dir().join("motr-session-reset-test.session");
        let path = path.to_string_lossy().to_string();

        std::fs::write(&path, b"[{\"gid\":\"0123456789abcdef\"}]").unwrap();

        assert!(remove_session_file(&path).unwrap(), "第一次应该真的删掉了");
        assert!(!Path::new(&path).exists(), "文件应该没了");
        assert!(
            !remove_session_file(&path).unwrap(),
            "第二次应该报「本来就不在」"
        );
    }
}
