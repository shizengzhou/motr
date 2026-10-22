import { getCurrentWindow, ProgressBarStatus } from '@tauri-apps/api/window';
import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';

import store from '@/store';
import is from '@/utils/is';
import { ask, message } from '@/utils/dialog';
import { getLocaleManager } from '@/components/Locale';
import { checkIsNeedRun } from '../shared/utils';
import { AUTO_CHECK_UPDATE_INTERVAL } from '../shared/constants';

/**
 * 应用更新管理器（对应原项目 src/main/UpdateManager.js）。
 *
 * 流程：检查 → 有更新则询问 → 下载并安装 → 提示完成 → 重启到新版本。
 * 没有更新时只在用户手动检查（userCheck）时提示，自动检查静默跳过。
 */
export default class UpdateManager {
  constructor(options = {}) {
    this.application = options.application || null;
    this.checking = false;
  }

  get i18n() {
    return getLocaleManager().getI18n();
  }

  // 下载进度打到窗口进度条上（原项目是 win.setProgressBar），
  // 语义与 EngineClient 里下载进度那套一致：null 表示清掉。
  setProgressBar(percent) {
    const state =
      percent === null
        ? { status: ProgressBarStatus.None }
        : {
            status: ProgressBarStatus.Normal,
            progress: Math.round(percent * 100),
          };

    getCurrentWindow()
      .setProgressBar(state)
      .catch((err) => {
        console.warn('[Motr] set update progress bar fail:', err);
      });
  }

  // 原项目在 autoUpdater 的 checking-for-update 事件里写 last-check-update-time
  saveLastCheckTime() {
    return this.application.configManager.setUserConfig(
      'last-check-update-time',
      Date.now()
    );
  }

  check(config) {
    const shouldCheck = checkIsNeedRun(
      config.autoCheckUpdate,
      config.lastCheckUpdateTime,
      AUTO_CHECK_UPDATE_INTERVAL
    );

    if (!shouldCheck) {
      return;
    }

    this.checkForUpdates();
  }

  async checkForUpdates({ userCheck = false } = {}) {
    if (!is.isTauri() || this.checking) {
      return;
    }

    this.checking = true;

    try {
      await this.saveLastCheckTime();

      // 与原版一致：更新检查走系统网络栈（系统代理），不读设置里的代理
      const update = await check();

      if (!update) {
        if (userCheck) {
          await message(this.i18n.t('app.update-not-available-message'), {
            title: this.i18n.t('app.check-for-updates-title'),
          });
        }
        return;
      }

      console.info('[Motr] update available:', update.version);

      const accepted = await ask(this.i18n.t('app.update-available-message'), {
        title: this.i18n.t('app.check-for-updates-title'),
        kind: 'info',
      });

      if (!accepted) {
        return;
      }

      let contentLength = 0;
      let transferred = 0;

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength || 0;
            transferred = 0;
            this.setProgressBar(0);
            break;
          case 'Progress':
            transferred += event.data.chunkLength;
            if (contentLength > 0) {
              this.setProgressBar(transferred / contentLength);
            }
            break;
          case 'Finished':
            this.setProgressBar(1);
            break;
          default:
            break;
        }
      });

      await message(this.i18n.t('app.update-downloaded-message'), {
        title: this.i18n.t('app.check-for-updates-title'),
      });

      // 等价原项目的 updater.quitAndInstall()：先把会话落盘，再走我们自己的重启
      //（`commands::relaunch` 会先停引擎 —— process 插件的 relaunch 会留下孤儿引擎，
      // 新版本进程起不来引擎，界面却连着旧的；见 utils/native.js 的 restartApp）
      await store.dispatch('task/saveSession');
      await relaunch();
    } catch (err) {
      console.warn('[Motr] check for updates fail:', err);
      await message(String(err), {
        title: this.i18n.t('app.check-for-updates-title'),
        kind: 'error',
      });
    } finally {
      this.setProgressBar(null);
      this.checking = false;
    }
  }
}
