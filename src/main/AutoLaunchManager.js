import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';

/**
 * 开机自动启动（对应原项目 src/main/core/AutoLaunchManager.js）。
 *
 * 原项目用 `app.setLoginItemSettings()`；Tauri 侧对应
 * `@tauri-apps/plugin-autostart`，登录项参数由 Rust 注册插件时指定
 *（`--opened-at-login=1`，见 src-tauri/src/lib.rs）。
 *
 * 和原版一致：**只在设置页保存、配置变化时**改写登录项，启动时不做
 * 反向同步 —— 否则用户刚在系统设置里手动关掉，一启动又被写回去。
 * 启动时只读取系统状态回填 user.json（见 ConfigManager.fixUserConfig）。
 */
export default class AutoLaunchManager {
  constructor(options = {}) {
    this.application = options.application || null;
  }

  isEnabled() {
    return isEnabled();
  }

  /**
   * 让系统登录项与期望值保持一致。
   * 先读当前状态，一致就不重复设置（原项目 `enable()` 也是先查再设）。
   */
  async apply(enabled) {
    try {
      const current = await isEnabled();

      if (current === enabled) {
        return;
      }

      if (enabled) {
        await enable();
      } else {
        await disable();
      }

      console.info(`[Motr] 开机自启已${enabled ? '开启' : '关闭'}`);
    } catch (err) {
      console.warn('[Motr] apply auto launch fail:', err);
    }
  }
}
