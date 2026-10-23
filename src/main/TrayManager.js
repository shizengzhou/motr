import {
  Menu,
  MenuItem,
  PredefinedMenuItem,
  Submenu,
} from '@tauri-apps/api/menu';
import { TrayIcon } from '@tauri-apps/api/tray';
import { Image } from '@tauri-apps/api/image';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { warn } from '@tauri-apps/plugin-log';

import { getLocaleManager } from '@/components/Locale';
import { getLanguage } from '@shared/locales';
import { APP_RUN_MODE, APP_THEME } from '@shared/constants';
import { acceleratorFor, handleCommand, translate } from '@/utils/menu';
import is from '@/utils/is';
import keymap from '@shared/keymap.json';
import trayTemplate from './menus/tray.json';

// ?inline 让 Vite 直接给出 data URL，运行时不用再 fetch 一次
import lightNormalIcon from '@/assets/icons/tray/mo-tray-light-normal.png?inline';
import lightActiveIcon from '@/assets/icons/tray/mo-tray-light-active.png?inline';
import darkNormalIcon from '@/assets/icons/tray/mo-tray-dark-normal.png?inline';
import darkActiveIcon from '@/assets/icons/tray/mo-tray-dark-active.png?inline';
import colorfulNormalIcon from '@/assets/icons/tray/mo-tray-colorful-normal.png?inline';
import colorfulActiveIcon from '@/assets/icons/tray/mo-tray-colorful-active.png?inline';

const TRAY_ID = 'motr-tray';

/// 托盘图标 -> 构建期内联的 data URL
const ICON_DATA_URLS = {
  'mo-tray-light-normal': lightNormalIcon,
  'mo-tray-light-active': lightActiveIcon,
  'mo-tray-dark-normal': darkNormalIcon,
  'mo-tray-dark-active': darkActiveIcon,
  'mo-tray-colorful-normal': colorfulNormalIcon,
  'mo-tray-colorful-active': colorfulActiveIcon,
};

/// data:image/png;base64,xxx -> Uint8Array（Image.fromBytes 用）
const dataUrlToBytes = (dataUrl) => {
  const binary = atob(dataUrl.slice(dataUrl.indexOf(',') + 1));
  const bytes = new Uint8Array(binary.length);

  for (let i = 0; i < binary.length; i += 1) {
    bytes[i] = binary.charCodeAt(i);
  }

  return bytes;
};

/**
 * 托盘管理器（对应原项目 src/main/ui/TrayManager.js）。
 *
 * 图标和右键菜单都在前端创建：
 * - 菜单模板见 src/main/menus/tray.json，文案用前端 i18n；
 * - 图标在 src/assets/icons/tray 下，通过 Vite 静态导入拿 URL（打包带 hash，
 *   不再依赖 public 的绝对路径）；速率计的动态图标由 worker 画好后通过
 *   setSpeedometer 直接换图标（不再经过 Rust 命令）。
 */
export default class TrayManager {
  constructor(options = {}) {
    this.application = options.application || null;
    this.theme = options.theme || APP_THEME.AUTO;
    this.runMode = options.runMode || APP_RUN_MODE.STANDARD;
    this.speedometer = options.speedometer === true;

    this.window = getCurrentWindow();
    this.tray = null;
    this.items = {};
    this.active = false;
    this.iconCache = {};

    // command -> 快捷键（keymap 是 keystroke -> command）
    this.keystrokes = Object.fromEntries(
      Object.entries(keymap).map(([keystroke, command]) => [command, keystroke])
    );
  }

  get i18n() {
    return getLocaleManager().getI18n();
  }

  async init() {
    if (!is.isTauri() || this.runMode === APP_RUN_MODE.HIDE_TRAY) {
      return;
    }

    if (this.tray) {
      return;
    }

    // 运行模式从 HideTray 切回来时 Rust 侧的旧托盘已经不在了，
    // 这里如果发现同 id 的残留（比如热重载），先收掉
    const existing = await TrayIcon.getById(TRAY_ID);
    if (existing) {
      await existing.close();
    }

    const menu = await this.buildMenu();

    this.tray = await TrayIcon.new({
      id: TRAY_ID,
      icon: await this.staticIcon(),
      iconAsTemplate: is.macOS(),
      tooltip: 'Motr',
      menu,
      // 左键留给「显示窗口」，右键由系统弹菜单
      showMenuOnLeftClick: false,
      action: (event) => this.handleTrayEvent(event),
    });
  }

  handleTrayEvent(event) {
    if (
      event.type === 'Click' &&
      event.button === 'Left' &&
      event.buttonState === 'Up'
    ) {
      this.application?.show?.('index');
    }
  }

  async buildMenu() {
    this.items = {};
    const items = await this.buildItems(trayTemplate);
    return Menu.new({ items });
  }

  async buildItems(nodes) {
    const result = [];
    for (const node of nodes) {
      // eslint-disable-next-line no-await-in-loop
      result.push(await this.buildItem(node));
    }
    return result;
  }

  async buildItem(node) {
    if (node.type === 'separator') {
      return PredefinedMenuItem.new({ item: 'Separator' });
    }

    const text = translate(node, this.i18n);

    if (node.submenu) {
      return Submenu.new({
        text,
        items: await this.buildItems(node.submenu),
      });
    }

    // Tauri 的 menu 插件按 MenuId 注册 action 通道（全局 HashMap<MenuId, Channel>），
    // App 菜单和托盘菜单里同 id 的项（如 app.task-list）会互相覆盖：后创建的那个
    // 抢到通道，点托盘实际执行的是 App 菜单里的节点。给托盘的 id 加前缀隔离。
    const item = await MenuItem.new({
      id: node.id ? `tray:${node.id}` : undefined,
      text,
      accelerator: acceleratorFor(node.command, this.keystrokes),
      action: () => handleCommand(node),
    });

    if (node.id) {
      this.items[node.id] = item;
    }

    return item;
  }

  async handleLocaleChange(locale) {
    if (!this.tray) {
      return;
    }

    await getLocaleManager().changeLanguage(getLanguage(locale) || 'en-US');

    const menu = await this.buildMenu();
    await this.tray.setMenu(menu);
  }

  async handleRunModeChange(mode) {
    this.runMode = mode;

    if (mode === APP_RUN_MODE.HIDE_TRAY) {
      await this.destroy();
      return;
    }

    await this.init();
  }

  async setActive(active) {
    if (this.active === active || is.macOS()) {
      this.active = active;
      return;
    }

    this.active = active;
    await this.refreshIcon();
  }

  /** 速率计：worker 画好的 PNG 直接当托盘图标（macOS 用 template 图自动反色） */
  async setSpeedometer(png) {
    if (!this.tray) {
      return;
    }

    try {
      const image = await Image.fromBytes(png);
      await this.tray.setIconWithAsTemplate(image, is.macOS());
    } catch (err) {
      warn(`[Motr] update tray speedometer fail: ${err}`);
    }
  }

  /** 关掉速率计 / 组件卸载时恢复静态图标 */
  async resetIcon() {
    if (!this.tray) {
      return;
    }

    await this.refreshIcon();
  }

  async refreshIcon() {
    if (!this.tray) {
      return;
    }

    const icon = await this.staticIcon();
    await this.tray.setIconWithAsTemplate(icon, is.macOS());
  }

  /** 当前该用的静态图标（对齐原 Rust tray.rs 的平台差异） */
  async staticIcon() {
    let name;

    if (is.macOS()) {
      // template 图只有一种状态，深浅色由系统反色
      name = 'mo-tray-light-normal';
    } else if (is.windows()) {
      name = this.active
        ? 'mo-tray-colorful-active'
        : 'mo-tray-colorful-normal';
    } else {
      // Linux：auto 与未知值都按 dark（多数桌面托盘是深色的）
      const palette = this.theme === APP_THEME.LIGHT ? 'light' : 'dark';
      name = `mo-tray-${palette}-${this.active ? 'active' : 'normal'}`;
    }

    return this.loadIcon(name);
  }

  async loadIcon(name) {
    if (this.iconCache[name]) {
      return this.iconCache[name];
    }

    const dataUrl = ICON_DATA_URLS[name];
    if (!dataUrl) {
      throw new Error(`unknown tray icon: ${name}`);
    }

    const image = await Image.fromBytes(dataUrlToBytes(dataUrl));
    this.iconCache[name] = image;

    return image;
  }

  async updateMenuStates(visibleStates, enabledStates, checkedStates) {
    if (visibleStates && Object.keys(visibleStates).length > 0) {
      warn('[Motr] tray menu item visible state is not supported yet');
    }

    if (checkedStates && Object.keys(checkedStates).length > 0) {
      warn('[Motr] tray menu item checked state is not supported yet');
    }

    await Promise.all(
      Object.entries(enabledStates || {}).map(([id, enabled]) =>
        this.items[id]?.setEnabled(enabled)
      )
    );
  }

  updateMenuItemVisibleState(id, flag) {
    return this.updateMenuStates({ [id]: flag }, null, null);
  }

  updateMenuItemEnabledState(id, flag) {
    return this.updateMenuStates(null, { [id]: flag }, null);
  }

  async destroy() {
    if (!this.tray) {
      return;
    }

    await this.tray.close().catch((err) => {
      warn(`[Motr] close tray fail: ${err}`);
    });
    this.tray = null;
    this.items = {};
  }
}
