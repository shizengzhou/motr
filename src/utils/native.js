// import { access, constants } from 'node:fs';
// 注意用 join 而不是 resolve：path-browserify 的 resolve 在参数不是 POSIX 绝对路径
// （/ 开头）时会走 process.cwd()，Windows 路径（C:\...）必然触发，webview 里没有
// process，直接抛 "process is not defined"——删除任务文件就是这样失败的
import { join } from 'path-browserify';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import { revealItemInDir, openPath } from '@tauri-apps/plugin-opener';

import store from '@/store';
import { Message } from 'element-ui';

import { isMagnetTask, getFileNameFromFile } from '@shared/utils';
import { TASK_STATUS } from '@shared/constants';

export const showItemInFolder = (fullPath, { errorMsg }) => {
  if (!fullPath) {
    return;
  }

  fullPath = join(fullPath);
  try {
    revealItemInDir(fullPath);
  } catch (err) {
    console.warn(`[Motr] ${fullPath} ${err}`);
    Message.error(errorMsg);
  }
};

export const openItem = async (fullPath) => {
  if (!fullPath) {
    return;
  }

  const result = await openPath(fullPath);
  return result;
};

// 系统通知。原 Electron 版用的是渲染进程的 Web Notification（Electron 直接支持那套），
// 在 Tauri 的 webview 里不生效，所以统一走 notification 插件。
// 权限只在第一次问一次，拒绝之后就一直静默跳过（等价于原项目没权限时什么都不弹）。
// 注意：Tauri 的通知没有 onclick，原项目里「点通知跳到文件所在目录」这一步没有对应能力，
// 需要的话可以用 registerActionTypes + onAction 做成通知上的按钮（macOS 支持）。
let notificationPermission = null;

const ensureNotificationPermission = () => {
  if (!notificationPermission) {
    notificationPermission = isPermissionGranted()
      .then((granted) =>
        granted ? true : requestPermission().then((res) => res === 'granted')
      )
      .catch((err) => {
        console.warn('[Motr] notification permission check fail:', err);
        return false;
      });
  }

  return notificationPermission;
};

/**
 * 重启应用。等价原项目 `Application.relaunch()`（先 `stopAllSettled()` 停引擎、
 * 再 `app.relaunch()` + `exit`）：这里先把 aria2 会话落盘，再交给 Rust 侧的
 * `commands::relaunch` —— 它会**先停掉引擎**再重启，新进程用新的配置重新起引擎。
 *
 * 注意不要直接用 process 插件的 `relaunch()`：`app.restart()` 内部只做
 * `cleanup_before_exit()`，**不会**发 `RunEvent::Exit`，shell 插件收不到 aria2
 * 子进程，会留下一个占着 RPC 端口的孤儿引擎（新引擎起不来、界面却连着旧的）。
 *
 * 用在「改完必须重启才生效」的设置上（`needRestartKeys`：RPC 端口/密钥、
 * 监听端口、隐藏应用菜单）。
 */
export const restartApp = async () => {
  await store.dispatch('task/saveSession');
  await invoke('relaunch');
};

export const showNotification = async ({ title, body }) => {
  if (!title) {
    return;
  }

  const granted = await ensureNotificationPermission();
  if (!granted) {
    return;
  }

  sendNotification({ title, body });
};

export const getTaskFullPath = (task) => {
  const { dir, files, bittorrent } = task;
  let result = join(dir);

  // Magnet link task
  if (isMagnetTask(task)) {
    return result;
  }

  if (bittorrent && bittorrent.info && bittorrent.info.name) {
    result = join(result, bittorrent.info.name);
    return result;
  }

  const [file] = files;
  const path = file.path ? join(file.path) : '';
  let fileName = '';

  if (path) {
    result = path;
  } else {
    if (files && files.length === 1) {
      fileName = getFileNameFromFile(file);
      if (fileName) {
        result = join(result, fileName);
      }
    }
  }

  return result;
};

const trashItem = (path) => invoke('trash_item', { path });

export const moveTaskFilesToTrash = async (task) => {
  /**
   * For magnet link tasks, there is bittorrent, but there is no bittorrent.info.
   * The path is not a complete path before it becomes a BT task.
   * In order to avoid accidentally deleting the directory
   * where the task is located, it directly returns true when deleting.
   */
  if (isMagnetTask(task)) {
    return true;
  }

  const { dir, status } = task;
  const path = getTaskFullPath(task);
  if (!path || dir === path) {
    throw new Error('task.file-path-error');
  }

  let deleteResult1 = true;
  try {
    await trashItem(path);
  } catch (err) {
    throw new Error(err instanceof Error ? err.message : String(err));
  }

  // There is no configuration file for the completed task.
  if (status === TASK_STATUS.COMPLETE) {
    return deleteResult1;
  }

  let deleteResult2 = true;
  const extraFilePath = `${path}.aria2`;
  try {
    await trashItem(extraFilePath);
  } catch (err) {
    throw new Error(err instanceof Error ? err.message : String(err));
  }

  return deleteResult1 && deleteResult2;
};

export const getSystemTheme = async () => {
  return await getCurrentWindow().theme();
};

export const delayDeleteTaskFiles = (task, delay) => {
  return new Promise((resolve, reject) => {
    setTimeout(async () => {
      try {
        const result = await moveTaskFilesToTrash(task);
        resolve(result);
      } catch (err) {
        reject(err.message);
      }
    }, delay);
  });
};
