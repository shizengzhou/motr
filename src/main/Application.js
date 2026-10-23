import { exit, setDockVisibility } from '@tauri-apps/api/app';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import { basename } from '@tauri-apps/api/path';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { info, warn } from '@tauri-apps/plugin-log';
import { message, open } from '@/utils/dialog';
import { readFile, remove, BaseDirectory } from '@tauri-apps/plugin-fs';
import { openUrl } from '@tauri-apps/plugin-opener';
import { relaunch } from '@tauri-apps/plugin-process';
import { Buffer } from 'buffer';
import { EventEmitter } from 'events';
import ConfigManager from './ConfigManager';
import Context from './Context';
import { APP_RUN_MODE } from '../shared/constants';
import is from '@/utils/is';
import ThemeManager from './ThemeManager';
import MenuManager from './MenuManager';
import TrayManager from './TrayManager';
import UpdateManager from './UpdateManager';
import AutoLaunchManager from './AutoLaunchManager';
import ProtocolManager from './ProtocolManager';
import { getLocaleManager } from '../components/Locale';
import { getLanguage } from '@shared/locales';
import Engine from './Engine';
import WindowManager from './WindowManager';

const i18n = getLocaleManager().getI18n();

export default class Application extends EventEmitter {
  constructor() {
    super();
  }

  async init() {
    try {
      await Promise.all([
        this.initContext(),
        this.initConfigManager(),
        this.initThemeManager(),
      ]);

      await this.initWindowManager();

      // 协议链接可能冷启动就带进来，先接住（等 Ipc 就绪后再补发命令）
      await this.initProtocolManager();

      // 更新管理器要在菜单/托盘建好之前就绪（菜单里的「检查更新」会用到）
      this.initUpdateManager();

      // 开机自启：只在设置页保存时改写系统登录项
      this.initAutoLaunchManager();

      // 先注册菜单/托盘命令的本地处理，避免托盘刚建好就点击时走前端兜底
      this.handleCommands();

      // 托盘 / App 菜单都依赖 configManager 里的运行模式与语言
      await this.initTrayManager();
      await this.initMenuManager();

      this.startEngine();
      this.handleEvents();
    } catch (err) {
      console.error('[Motr] Application.init error:', err);
      throw err;
    }
  }

  handleCommands() {
    this.on('help:official-website', async () => {
      const url = 'https://motrix.app/';
      await openUrl(url);
    });

    this.on('help:manual', async () => {
      const url = 'https://motrix.app/manual';
      await openUrl(url);
    });

    this.on('help:release-notes', async () => {
      const url = 'https://motrix.app/release';
      await openUrl(url);
    });

    this.on('help:report-problem', async () => {
      const url = 'https://motrix.app/report';
      await openUrl(url);
    });

    this.on('application:open-file', async () => {
      const selected = await open({
        multiple: false,
        filters: [{ name: 'Torrent', extensions: ['torrent'] }],
      });

      if (!selected) {
        return;
      }

      const path = Array.isArray(selected) ? selected[0] : selected;
      const name = await basename(path);
      try {
        const data = await readFile(path);
        const dataURL = Buffer.from(data).toString('base64');
        this.sendCommandToAll('application:new-bt-task-with-file', {
          name,
          dataURL,
        });
      } catch (err) {
        warn(`[Motr] read file error: ${path} ${err.message}`);
      }
    });

    this.on('application:quit', async () => {
      // 退出流程开始：之后的关闭请求放行真关（对齐原项目 before-quit 里的 willQuit）
      this.windowManager.willQuit = true;
      await exit(0);
    });

    this.on('application:show', async ({ page }) => {
      await this.show(page);
    });
  }

  async show(page = 'index') {
    console.log(`show ${page}`);
    await this.windowManager.showWindow();
  }

  async initContext() {
    this.context = new Context();
    await this.context.init();
  }

  async initConfigManager() {
    this.configListeners = {};
    this.configManager = new ConfigManager();
    await this.configManager.init();
  }

  async initThemeManager() {
    this.themeManager = new ThemeManager();
    await this.themeManager.init();
    this.themeManager.on('system-theme-change', async (theme) => {
      // this.trayManager.handleSystemThemeChange(theme);
      await this.sendCommandToAll('application:update-system-theme', { theme });
    });
  }

  async initWindowManager() {
    this.windowManager = new WindowManager({ application: this });

    const keepWindowState = await this.configManager.getUserConfig(
      'keep-window-state'
    );

    if (keepWindowState) {
      // 以上次退出时的 bounds 为准，不再做小屏幕适配
      await this.windowManager.restoreWindowState();
    } else {
      await this.windowManager.fitWindowToDisplay();
    }

    // 窗口显示之前应用「隐藏菜单栏」，避免先闪一下原生标题栏
    await this.windowManager.hideAppMenu();
  }

  async initMenuManager() {
    this.menuManager = new MenuManager({ application: this });
    await this.menuManager.setup();
  }

  async initTrayManager() {
    const [runMode, theme, locale] = await Promise.all([
      this.configManager.getUserConfig('run-mode'),
      this.configManager.getUserConfig('tray-theme'),
      this.configManager.getLocale(),
    ]);

    // 托盘菜单文案在创建时用当前 i18n 翻译，而 Application.init 早于
    // main.js 里的 init(config)，此时语言还是默认 en-US；先按 user.json 切过来，
    // 否则 App 菜单是中文、托盘菜单是英文
    await getLocaleManager().changeLanguage(getLanguage(locale) || 'en-US');

    this.trayManager = new TrayManager({
      application: this,
      runMode,
      theme,
    });
    await this.trayManager.init();

    // 启动时按运行模式同步 Dock 图标（托盘本身由 TrayManager 创建）
    await this.applyDockVisibility(runMode);
  }

  initUpdateManager() {
    this.updateManager = new UpdateManager({ application: this });
  }

  initAutoLaunchManager() {
    this.autoLaunchManager = new AutoLaunchManager({ application: this });
  }

  async initProtocolManager() {
    this.protocolManager = new ProtocolManager({ application: this });
    await this.protocolManager.init();

    const protocols = await this.configManager.getUserConfig('protocols');
    await this.protocolManager.apply(protocols);
  }

  async applyDockVisibility(mode) {
    if (!is.macOS()) {
      return;
    }

    try {
      await setDockVisibility(mode !== APP_RUN_MODE.TRAY);
    } catch (err) {
      warn(`[Motr] set dock visibility fail: ${err}`);
    }
  }

  async handleEvents() {
    await Promise.all([
      this.watchThemeChange(),
      this.watchLocaleChange(),
      this.watchRunModeChange(),
    ]);
  }

  async stopEngine() {
    info('[Motr] stopEngine===>');
    try {
      // await this.engineClient.shutdown({ force: true });
      info('[Motr] stopEngine.setImmediate===>');
      setTimeout(() => {
        this.engine.stop();
      });
    } catch (err) {
      warn(`[Motr] shutdown engine fail: ${err.message}`);
    } finally {
      // no finally
    }
  }

  async startEngine() {
    try {
      const [systemConfig, userConfig] = await Promise.all([
        this.configManager.getSystemConfig(),
        this.configManager.getUserConfig(),
      ]);
      this.engine = new Engine({
        systemConfig,
        userConfig,
      });
      this.engine.start();
    } catch (err) {
      message(i18n.t('app.system-error-message', { message: err.message }), {
        title: i18n.t('app.system-error-title'),
        kind: 'error',
      }).then(() => {
        setTimeout(async () => {
          await exit(1);
        }, 100);
      });
    }
  }

  async watchRunModeChange() {
    const { userConfig } = this.configManager;
    const key = 'run-mode';
    this.configListeners[key] = await userConfig.onKeyChange(
      key,
      async (newValue) => {
        info(`[Motr] detected ${key} value change event: ${newValue}`);
        this.trayManager.handleRunModeChange(newValue);

        this.applyDockVisibility(newValue);
      }
    );
  }

  async watchLocaleChange() {
    const { userConfig } = this.configManager;
    const key = 'locale';
    this.configListeners[key] = await userConfig.onKeyChange(
      key,
      async (newValue) => {
        info(`[Motr] detected ${key} value change event: ${newValue}`);
        const localeManager = getLocaleManager();
        localeManager.changeLanguageByLocale(newValue).then(() => {
          // this.menuManager.handleLocaleChange(newValue);
          // this.trayManager.handleLocaleChange(newValue);
        });
        // this.sendCommandToAll('application:update-locale', {
        //   locale: newValue,
        // });
      }
    );
  }

  async watchThemeChange() {
    const { userConfig } = this.configManager;
    const key = 'theme';
    this.configListeners[key] = await userConfig.onKeyChange(
      key,
      async (newValue) => {
        info(`[Motr] detected ${key} value change event:`, newValue);
        const themeMap = {
          dark: 'dark',
          light: 'light',
          auto: null,
        };
        await getCurrentWebviewWindow().setTheme(themeMap[newValue]);
        emit('command', {
          command: 'application:update-theme',
          args: [{ theme: newValue }],
        });
      }
    );
  }

  async reset() {
    await this.configManager.reset();
    await this.relaunch();
  }

  async resetSession() {
    await this.stopEngine();

    setTimeout(() => {
      remove('download.session', {
        baseDir: BaseDirectory.AppData,
      }).catch((err) => {
        console.info('[Motr] Removed the download seesion file: ', err);
      });

      this.engine.start();
    }, 3000);
  }

  async relaunch() {
    await this.stopAllSettled();
    await relaunch();
  }

  stop() {
    try {
      const promises = [
        this.stopEngine(),
        this.shutdownUPnPManager(),
        // this.energyManager.stopPowerSaveBlocker(),
        this.trayManager.destroy(),
      ];

      return promises;
    } catch (err) {
      warn(`[Motr] stop error: ${err.message}`);
    }
  }

  async stopAllSettled() {
    await Promise.allSettled(this.stop());
  }

  async shutdownUPnPManager() {
    await invoke('shutdown_upnp');
  }

  async sendCommandToAll(command, ...args) {
    if (!this.emit(command, ...args)) {
      await emit('command', { command, args });
    }
  }
}
