import {
  Menu,
  MenuItem,
  PredefinedMenuItem,
  Submenu,
} from '@tauri-apps/api/menu';
import { invoke } from '@tauri-apps/api/core';
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

    // command -> 菜单节点：buildItem 时填充，快捷键分发要走和菜单点击
    // 完全相同的 handleCommand（带 command-before / command-after / command-arg）
    this.nodesByCommand = {};
    this.shortcutsBound = false;
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
    this.nodesByCommand = {};
    const items = await this.buildItems(template.menu);
    const menu = await Menu.new({ items });

    // macOS 是全局菜单栏；Windows/Linux 的菜单挂在窗口上
    if (is.macOS()) {
      await menu.setAsAppMenu();
    } else {
      await menu.setAsWindowMenu(this.window);
    }

    this.bindShortcuts();
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

    if (node.command) {
      this.nodesByCommand[node.command] = node;
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

    // Tauri 的 PredefinedMenuItem 没有 Electron 的 toggledevtools，
    // 单独交给 Rust 命令切换主窗口的 WebView 开发者工具
    if (node.role === 'toggledevtools') {
      const item = await MenuItem.new({
        id: node.id,
        text,
        action: () => invoke('toggle_devtools'),
      });
      if (node.id) {
        this.items[node.id] = item;
      }
      return item;
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

  /// Win/Linux 的窗口菜单加速键到不了应用的消息循环：tauri 在 Windows 上靠
  /// msg hook 调 TranslateAcceleratorW（tauri app.rs），但窗口聚焦时键盘焦点
  /// 一直在 WebView2 子窗口上，hook 看到的 msg.hwnd 不是挂了口令表的顶层窗口，
  /// 加速键永远不触发（Linux/GTK 同理）。实测表现为「菜单项旁有快捷键字样、
  /// 点击菜单正常、按键无反应」。
  /// macOS 的 App 菜单是 NSMenu，key equivalent 由系统在 webview 之前处理，不受影响。
  /// 这里在渲染进程补一层 keydown，用同一份 keymap 查命令、走与菜单点击相同的
  /// handleCommand——效果与原 Electron 版的菜单加速键一致。
  bindShortcuts() {
    if (this.shortcutsBound || is.macOS()) {
      return;
    }

    // 捕获阶段 + preventDefault：和原生菜单加速键一样，按键不到页面
    window.addEventListener('keydown', this.handleShortcut, true);
    this.shortcutsBound = true;
  }

  handleShortcut = (event) => {
    const hit = this.findCommandForKeystroke(event);
    if (!hit) {
      return;
    }

    event.preventDefault();

    const { command, node } = hit;
    if (node) {
      handleCommand(node);
    } else {
      // keymap 里有、菜单里却没有独立节点的命令（如 quit 是 role 项，
      // 由系统的预定义菜单项实现）：没有 command-before/after/arg，
      // 直接分发命令即可（与点菜单等价）
      window.application?.sendCommandToAll(command);
    }
  };

  findCommandForKeystroke(event) {
    const parts = [];
    if (event.ctrlKey) {
      parts.push('ctrl');
    }
    if (event.metaKey) {
      parts.push('cmd');
    }
    if (event.shiftKey) {
      parts.push('shift');
    }
    if (event.altKey) {
      parts.push('alt');
    }

    const key = event.key && event.key.toLowerCase();
    if (!key) {
      return null;
    }
    parts.push(key);

    // keymap 用 cmdctrl 表示「平台主修饰键」（Windows 上就是 Ctrl）。
    // 同一次按键要同时试 "ctrl-..." 和 "cmdctrl-..." 两种拼写：
    // Ctrl+Shift+A 直接命中 ctrl-shift-a，Ctrl+Shift+N 命中 cmdctrl-shift-n
    const candidates = [parts.join('-')];
    if (parts[0] === 'ctrl' || parts[0] === 'cmd') {
      candidates.push(['cmdctrl', ...parts.slice(1)].join('-'));
    }

    for (const keystroke of candidates) {
      const command = keymap[keystroke];
      if (command) {
        return { command, node: this.nodesByCommand[command] };
      }
    }

    return null;
  }
}
