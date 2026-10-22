import { LazyStore } from '@tauri-apps/plugin-store';
import { isEnabled } from '@tauri-apps/plugin-autostart';
import is from '../utils/is';

import {
  getDhtPath,
  getMaxConnectionPerServer,
  getUserDownloadsPath,
} from '../utils/index';
import {
  APP_RUN_MODE,
  APP_THEME,
  EMPTY_STRING,
  ENGINE_BT_PORT,
  ENGINE_DHT_PORT,
  ENGINE_RPC_PORT,
  IP_VERSION,
  NGOSANG_TRACKERS_BEST_IP_URL_CDN,
  NGOSANG_TRACKERS_BEST_URL_CDN,
} from '@shared/constants';
import { CHROME_UA } from '@shared/ua';
import { separateConfig } from '@shared/utils';
import { reduceTrackerString } from '@shared/utils/tracker';

const defaultSystemConfig = {
  'all-proxy': EMPTY_STRING,
  'allow-overwrite': false,
  'auto-file-renaming': true,
  'bt-exclude-tracker': EMPTY_STRING,
  'bt-force-encryption': false,
  'bt-load-saved-metadata': true,
  'bt-save-metadata': true,
  'bt-tracker': EMPTY_STRING,
  continue: true,
  'dht-listen-port': ENGINE_DHT_PORT,
  'enable-dht6': true,
  'follow-metalink': true,
  'follow-torrent': true,
  'listen-port': ENGINE_BT_PORT,
  'max-concurrent-downloads': 5,
  'max-connection-per-server': getMaxConnectionPerServer(),
  'max-download-limit': 0,
  'max-overall-download-limit': 0,
  'max-overall-upload-limit': 0,
  'no-proxy': EMPTY_STRING,
  'pause-metadata': false,
  pause: true,
  'rpc-listen-port': ENGINE_RPC_PORT,
  'rpc-secret': EMPTY_STRING,
  'seed-ratio': 2,
  'seed-time': 2880,
  split: getMaxConnectionPerServer(),
  'user-agent': CHROME_UA,
};

const defaultUserConfig = {
  'all-proxy-backup': EMPTY_STRING,
  'auto-check-update': is.macOS(),
  'auto-hide-window': false,
  'auto-sync-tracker': true,
  'enable-upnp': true,
  'engine-max-connection-per-server': getMaxConnectionPerServer(),
  'favorite-directories': [],
  'hide-app-menu': is.windows() || is.linux(),
  'history-directories': [],
  'keep-seeding': false,
  'keep-window-state': false,
  'last-check-update-time': 0,
  'last-sync-tracker-time': 0,
  locale: navigator.language,
  'log-level': 'warn',
  'new-task-show-downloading': true,
  'no-confirm-before-delete-task': false,
  'open-at-login': false,
  protocols: { magnet: true, thunder: false },
  'resume-all-when-app-launched': false,
  'run-mode': APP_RUN_MODE.STANDARD,
  'show-progress-bar': true,
  'task-notification': true,
  theme: APP_THEME.AUTO,
  'tracker-source': [
    NGOSANG_TRACKERS_BEST_IP_URL_CDN,
    NGOSANG_TRACKERS_BEST_URL_CDN,
  ],
  'tray-theme': APP_THEME.AUTO,
  'tray-speedometer': is.macOS(),
  'update-channel': 'latest',
  'use-proxy': false,
  'window-state': {},
};

export default class ConfigManager {
  constructor() {
    this.systemConfig = {};
    this.systemDefaults = {};
    this.userConfig = {};
    this.userDefaults = {};
  }

  async init() {
    await this.initUserConfig();
    await this.initSystemConfig();

    // 先补默认值，再做 fixUserConfig / fixSystemConfig 的修正：
    // 反过来的话，残缺的 user.json 会在 fixUserConfig 里先抛
    //（tracker-source 为 undefined 时取 .length）
    await this.syncDefaultConfig();
    await this.fixUserConfig();
    await this.fixSystemConfig();
  }

  /**
   * 把 defaults 里缺失的键补进 user.json / system.json。
   *
   *  rust 侧先创建 Store 的话，js 侧的 defaults 会失效。不补的话文件会停留在
   * 「只被写过几个键」的残缺状态（`Store.set` 的自动保存是整份 cache 覆盖写盘）。
   *
   * 已存在的键一律不动，只补缺。
   */
  async syncDefaultConfig() {
    await this.syncStoreDefaults(
      'user.json',
      this.userConfig,
      this.userDefaults
    );
    await this.syncStoreDefaults(
      'system.json',
      this.systemConfig,
      this.systemDefaults
    );
  }

  async syncStoreDefaults(file, store, defaults) {
    const entries = Object.fromEntries(await store.entries());
    const missingKeys = Object.keys(defaults).filter(
      (key) => entries[key] === undefined
    );

    if (missingKeys.length === 0) {
      return;
    }

    for (const key of missingKeys) {
      // eslint-disable-next-line no-await-in-loop
      await store.set(key, defaults[key]);
    }

    // set 只改内存 cache（默认还有 100ms 防抖写盘），显式存一次，
    // 免得刚补齐默认值就被退出流程吞掉
    await store.save();

    console.info(`[Motr] ${file} 补齐缺失的默认键:`, missingKeys);
  }

  /**
   * Aria2 Configuration Priority
   * system.json > built-in aria2.conf
   * https://aria2.github.io/manual/en/html/aria2c.html
   *
   */
  async initSystemConfig() {
    // dht 路径 / 下载目录要靠 Tauri path API 现算，不能把 Promise
    // 塞进 defaults（序列化后会变成 {}）
    const [dhtPath, dhtPath6, downloadDir] = await Promise.all([
      getDhtPath(IP_VERSION.V4),
      getDhtPath(IP_VERSION.V6),
      getUserDownloadsPath(),
    ]);

    this.systemDefaults = {
      ...defaultSystemConfig,
      'dht-file-path': dhtPath,
      'dht-file-path6': dhtPath6,
      dir: downloadDir,
    };

    this.systemConfig = new LazyStore('system.json', {
      defaults: this.systemDefaults,
    });
  }

  async initUserConfig() {
    this.userDefaults = defaultUserConfig;
    this.userConfig = new LazyStore('user.json', {
      defaults: this.userDefaults,
    });
  }

  async fixSystemConfig() {
    // Remove aria2c unrecognized options
    const { others } = separateConfig(
      Object.fromEntries(await this.systemConfig.entries())
    );
    if (others && Object.keys(others).length > 0) {
      await Promise.all(
        Object.keys(others).map((key) => this.systemConfig.delete(key))
      );
    }

    // 代理不再从 user.json 派生：与原版一致，设置页保存时直接把
    // all-proxy / no-proxy 写进 system.json

    // Fix spawn ENAMETOOLONG on Windows
    const tracker = reduceTrackerString(
      await this.systemConfig.get('bt-tracker')
    );
    await this.setSystemConfig('bt-tracker', tracker);
  }

  async fixUserConfig() {
    // Fix the value of open-at-login when the user delete
    // the Motrix self-starting item through startup management.
    const openAtLogin = await isEnabled();
    if ((await this.getUserConfig('open-at-login')) !== openAtLogin) {
      await this.setUserConfig('open-at-login', openAtLogin);
    }

    const trackerSource = await this.getUserConfig('tracker-source', []);

    if (!Array.isArray(trackerSource) || trackerSource.length === 0) {
      await this.setUserConfig('tracker-source', [
        NGOSANG_TRACKERS_BEST_IP_URL_CDN,
        NGOSANG_TRACKERS_BEST_URL_CDN,
      ]);
    }
  }

  async getSystemConfig(key, defaultValue) {
    if (typeof key === 'undefined' && typeof defaultValue === 'undefined') {
      return Object.fromEntries(await this.systemConfig.entries());
    }

    const value = await this.systemConfig.get(key);

    if (value == undefined) {
      return defaultValue;
    }

    return value;
  }

  async getUserConfig(key, defaultValue) {
    if (typeof key === 'undefined' && typeof defaultValue === 'undefined') {
      return Object.fromEntries(await this.userConfig.entries());
    }

    const value = await this.userConfig.get(key);

    if (value == undefined) {
      return defaultValue;
    }

    return value;
  }

  async getLocale() {
    return (await this.getUserConfig('locale')) || navigator.language;
  }

  async setSystemConfig(key, value) {
    await this.systemConfig.set(key, value);
  }

  async setUserConfig(key, value) {
    await this.userConfig.set(key, value);
  }

  async reset() {
    await this.systemConfig.reset();
    await this.userConfig.reset();
  }
}
