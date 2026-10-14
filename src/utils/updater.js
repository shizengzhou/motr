import { getCurrentWindow, ProgressBarStatus } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { ask, message } from '@tauri-apps/plugin-dialog';
import { check } from '@tauri-apps/plugin-updater';

import store from '@/store';
import { PROXY_SCOPES } from '@shared/constants';
import is from '@/utils/is';
import { proxyUrlForScope } from '@/utils/proxy';
import { getLocaleManager } from '@/components/Locale';

const i18n = getLocaleManager().getI18n();

// 下载进度打到窗口进度条上（原项目是 win.setProgressBar），
// 语义与 EngineClient 里下载进度那套一致：-1/None 表示清掉。
const setProgressBar = (percent) => {
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
};

// 原项目在 autoUpdater 的 checking-for-update 事件里写 last-check-update-time
const saveLastCheckTime = () =>
  store.dispatch('preference/save', { lastCheckUpdateTime: Date.now() });

let checking = false;

/**
 * 检查更新。流程对齐原项目 UpdateManager：
 * 有更新 → 问「是否下载」→ 下载并安装 → 提示「下载完成」→ 重启到新版本；
 * 没有更新时只在用户手动检查（userCheck）时提示，自动检查静默跳过。
 */
export const checkForUpdates = async ({ userCheck = false } = {}) => {
  if (!is.isTauri() || checking) {
    return;
  }

  checking = true;

  try {
    await saveLastCheckTime();

    const update = await check({
      // 「代理 → 更新应用」勾了才用设置里的代理，否则给 undefined，由插件回落默认（系统代理）
      proxy: proxyUrlForScope(
        store.state.preference.config.proxy,
        PROXY_SCOPES.UPDATE_APP
      ),
    });

    if (!update) {
      if (userCheck) {
        await message(i18n.t('app.update-not-available-message'), {
          title: i18n.t('app.check-for-updates-title'),
        });
      }
      return;
    }

    console.info('[Motr] update available:', update.version);

    const accepted = await ask(i18n.t('app.update-available-message'), {
      title: i18n.t('app.check-for-updates-title'),
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
          setProgressBar(0);
          break;
        case 'Progress':
          transferred += event.data.chunkLength;
          if (contentLength > 0) {
            setProgressBar(transferred / contentLength);
          }
          break;
        case 'Finished':
          setProgressBar(1);
          break;
        default:
          break;
      }
    });

    await message(i18n.t('app.update-downloaded-message'), {
      title: i18n.t('app.check-for-updates-title'),
    });

    // 等价原项目的 updater.quitAndInstall()：先把会话落盘，再走我们自己的重启
    //（`commands::relaunch` 会先停引擎 —— process 插件的 relaunch 会留下孤儿引擎，
    // 新版本进程起不来引擎，界面却连着旧的；见 utils/native.js 的 restartApp）
    await store.dispatch('task/saveSession');
    await invoke('relaunch');
  } catch (err) {
    console.warn('[Motr] check for updates fail:', err);
    await message(String(err), {
      title: i18n.t('app.check-for-updates-title'),
      kind: 'error',
    });
  } finally {
    setProgressBar(null);
    checking = false;
  }
};
