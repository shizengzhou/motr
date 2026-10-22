import {
  Menu,
  MenuItem,
  PredefinedMenuItem,
  Submenu,
} from '@tauri-apps/api/menu';
import { getCurrentWindow } from '@tauri-apps/api/window';

import { getLocaleManager } from '@/components/Locale';
import { getLanguage } from '@shared/locales';
import is from '@/utils/is';
import keymap from '@shared/keymap.json';
import darwinTemplate from './menus/darwin.json';
import win32Template from './menus/win32.json';
import linuxTemplate from './menus/linux.json';
import { acceleratorFor, handleCommand, translate } from '../utils/menu';

/// Electron role -> Tauri 的预定义菜单项（系统自己实现行为）
const ROLE_ITEMS = {
  undo: 'Undo',
  redo: 'Redo',
  cut: 'Cut',
  copy: 'Copy',
  paste: 'Paste',
  selectall: 'SelectAll',
  hide: 'Hide',
  hideothers: 'HideOthers',
  unhide: 'ShowAll',
  quit: 'Quit',
  minimize: 'Minimize',
  close: 'CloseWindow',
  zoom: 'Maximize',
  togglefullscreen: 'Fullscreen',
  front: 'BringAllToFront',
};

export default class MenuManager {
  constructor(options = {}) {
    this.application = options.application || null;
    this.window = getCurrentWindow();

    // command -> 快捷键（keymap 是 keystroke -> command）
    this.keystrokes = Object.fromEntries(
      Object.entries(keymap).map(([keystroke, command]) => [command, keystroke])
    );
  }

  get i18n() {
    return getLocaleManager().getI18n();
  }

  async getLocale() {
    const locale = await this.application?.configManager?.getLocale?.();
    return locale || navigator.language;
  }

  /** 创建并设置 App 菜单；locale 缺省时用 user.json 里的语言 */
  async setup(locale) {
    if (!is.isTauri()) {
      return;
    }

    const target = locale || (await this.getLocale());
    await getLocaleManager().changeLanguage(getLanguage(target) || 'en-US');

    this.items = {};

    const template = this.getTemplate();
    const items = await this.buildItems(template.menu);
    const menu = await Menu.new({ items });

    // macOS 是全局菜单栏；Windows/Linux 的菜单挂在窗口上
    if (is.macOS()) {
      await menu.setAsAppMenu();
    } else {
      await menu.setAsWindowMenu(this.window);
    }
  }

  async handleLocaleChange(locale) {
    await this.setup(locale);
  }

  getTemplate() {
    if (is.macOS()) {
      return darwinTemplate;
    }
    if (is.windows()) {
      return win32Template;
    }
    return linuxTemplate;
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
      const submenu = await Submenu.new({
        text,
        items: await this.buildItems(node.submenu),
      });
      if (node.id) {
        this.items[node.id] = submenu;
      }
      return submenu;
    }

    const predefined = ROLE_ITEMS[node.role];
    if (predefined) {
      // 系统预定义项（复制/粘贴/退出…）由系统处理，不需要 action
      return PredefinedMenuItem.new({ item: predefined, text });
    }

    // 其余项（包括 Tauri 没有预定义项的 delete 等 role）按普通菜单项创建
    const item = await MenuItem.new({
      id: node.id,
      text,
      accelerator: acceleratorFor(node.command, this.keystrokes),
      action: () => handleCommand(node),
    });
    if (node.id) {
      this.items[node.id] = item;
    }
    return item;
  }
}
