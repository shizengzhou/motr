import { invoke } from '@tauri-apps/api/core';
import { warn } from '@tauri-apps/plugin-log';

/**
 * 电源管理（对应原项目 src/main/core/EnergyManager.js）。
 *
 * 原项目调 Electron 的 `powerSaveBlocker.start('prevent-app-suspension')`，
 * Tauri 没有对应 API，实际断言在 Rust 侧用 keepawake 落到系统原生接口
 *（见 src-tauri/src/energy.rs）。这里只保留原项目的两个方法名，
 * 让调用方（EngineClient 的 downloading watcher）读起来和原版一致。
 *
 * 两个方法都幂等（Rust 侧判断当前有没有断言），重复调用无副作用；
 * 不在这里记状态，避免「start/stop 交叉」时前端状态与真实断言不一致。
 */
export default class EnergyManager {
  async startPowerSaveBlocker() {
    try {
      await invoke('start_power_save_blocker');
    } catch (err) {
      warn(`[Motr] EnergyManager.startPowerSaveBlocker error: ${err}`);
    }
  }

  async stopPowerSaveBlocker() {
    try {
      await invoke('stop_power_save_blocker');
    } catch (err) {
      warn(`[Motr] EnergyManager.stopPowerSaveBlocker error: ${err}`);
    }
  }
}
