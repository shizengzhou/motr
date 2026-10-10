// import { access, constants } from 'node:fs';
import { resolve } from 'path-browserify';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { revealItemInDir, openPath } from '@tauri-apps/plugin-opener';
import { Message } from 'element-ui';

import { isMagnetTask, getFileNameFromFile } from '@shared/utils';
import { APP_THEME, TASK_STATUS } from '@shared/constants';

export const showItemInFolder = (fullPath, { errorMsg }) => {
  if (!fullPath) {
    return;
  }

  fullPath = resolve(fullPath);
  try {
    revealItemInDir(fullPath);
  } catch (err) {
    console.warn(`[Motrix] ${fullPath} ${err}`);
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

export const getTaskFullPath = (task) => {
  const { dir, files, bittorrent } = task;
  let result = resolve(dir);

  // Magnet link task
  if (isMagnetTask(task)) {
    return result;
  }

  if (bittorrent && bittorrent.info && bittorrent.info.name) {
    result = resolve(result, bittorrent.info.name);
    return result;
  }

  const [file] = files;
  const path = file.path ? resolve(file.path) : '';
  let fileName = '';

  if (path) {
    result = path;
  } else {
    if (files && files.length === 1) {
      fileName = getFileNameFromFile(file);
      if (fileName) {
        result = resolve(result, fileName);
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
  const theme = await getCurrentWindow().theme();
  return theme === 'dark' ? APP_THEME.DARK : APP_THEME.LIGHT;
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
