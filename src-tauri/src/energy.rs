//! 电源管理（对应原项目 `src/main/core/EnergyManager.js`）
//!
//! 原项目用 Electron 的 `powerSaveBlocker.start('prevent-app-suspension')`：
//! 有任务在下载时阻止系统自动休眠（**不**阻止熄屏），下载全部结束后释放。
//!
//! Tauri 没有对应的 API，这里用 `keepawake` crate 落到各平台原生接口：
//! - macOS：IOPMAssertion（`PreventUserIdleSystemSleep`）
//! - Windows：`SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`
//! - Linux：systemd-logind `Inhibit("idle")`（D-Bus）
//!
//! 断言对象是 RAII：调用 `stop` 或进程退出时 drop，即向系统释放。
//!
//! 启动/停止的时机与原项目一致 —— 只有「活跃下载数由 0 变正 / 由正变 0」
//! 时才切换（前端 `EngineClient.vue` 的 `downloading` watcher）。

use std::sync::Mutex;

use tauri::{AppHandle, Manager, State};

/// 当前持有的电源断言；`None` 表示没有活跃下载、未阻止休眠
#[derive(Default)]
pub struct EnergyState(Mutex<Option<keepawake::KeepAwake>>);

fn lock(state: &EnergyState) -> std::sync::MutexGuard<'_, Option<keepawake::KeepAwake>> {
    // 这里没有需要保持一致性的共享数据，锁中毒时继续使用内部值即可
    state.0.lock().unwrap_or_else(|err| err.into_inner())
}

/// 开始阻止系统休眠（幂等：已经有断言时直接返回）
pub fn start(state: &EnergyState) -> Result<(), String> {
    let mut guard = lock(state);

    if guard.is_some() {
        return Ok(());
    }

    match keepawake::Builder::default()
        // 只阻止「空闲休眠」，不阻止熄屏 —— 与原项目 prevent-app-suspension 的语义一致
        .idle(true)
        .display(false)
        .reason("Active downloads in progress")
        .app_name("Motr")
        .app_reverse_domain("com.shizeng.Motr")
        .create()
    {
        Ok(awake) => {
            *guard = Some(awake);
            log::info!("[Motr] power save blocker started");
            Ok(())
        }
        Err(err) => {
            log::warn!("[Motr] start power save blocker failed: {err}");
            Err(err.to_string())
        }
    }
}

/// 释放断言（幂等：本来就没有时什么都不做）
pub fn stop(state: &EnergyState) {
    let mut guard = lock(state);

    if guard.take().is_some() {
        log::info!("[Motr] power save blocker stopped");
    }
}

/// 退出前清掉断言（进程退出时 RAII 也会释放，这里只是让顺序更明确）
pub fn shutdown(app: &AppHandle) {
    if let Some(state) = app.try_state::<EnergyState>() {
        stop(&state);
    }
}

#[tauri::command]
pub fn start_power_save_blocker(state: State<'_, EnergyState>) -> Result<(), String> {
    start(&state)
}

#[tauri::command]
pub fn stop_power_save_blocker(state: State<'_, EnergyState>) {
    stop(&state);
}
