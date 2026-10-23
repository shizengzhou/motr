import { isTauri } from '@tauri-apps/api/core';
import { type } from '@tauri-apps/plugin-os';

const osType = type();

export default {
  isTauri,
  dev: () => import.meta.env.DEV,
  // Tauri 打包不区分 Mac App Store 版本
  mas: () => false,
  macOS: () => osType === 'macos',
  windows: () => osType === 'windows',
  linux: () => osType === 'linux',
};
