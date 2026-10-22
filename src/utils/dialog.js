import {
  ask as rawAsk,
  confirm as rawConfirm,
  message as rawMessage,
  open as rawOpen,
  save as rawSave,
} from '@tauri-apps/plugin-dialog';

/**
 * 原生对话框的计数包装。
 *
 * 打开系统对话框会让主窗口失焦（WindowEvent::Focused(false)），
 * 如果这时「自动隐藏窗口」按失焦就 hide，窗口会被藏到对话框后面，
 * 对话框关掉后也不会自己回来。WindowManager 在 hide 前用 isDialogOpen()
 * 判断一下，对话框期间跳过自动隐藏。
 */
let openDialogs = 0;

export const isDialogOpen = () => openDialogs > 0;

const track = async (dialog, ...args) => {
  openDialogs += 1;

  try {
    return await dialog(...args);
  } finally {
    openDialogs -= 1;
  }
};

export const open = (...args) => track(rawOpen, ...args);

export const save = (...args) => track(rawSave, ...args);

export const message = (...args) => track(rawMessage, ...args);

export const ask = (...args) => track(rawAsk, ...args);

export const confirm = (...args) => track(rawConfirm, ...args);
