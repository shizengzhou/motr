import { getCurrentWindow } from '@tauri-apps/api/window';
import { info } from '@tauri-apps/plugin-log';
import { EventEmitter } from 'events';
import { getSystemTheme } from '../utils/native';
import store from '../store';

export default class ThemeManager extends EventEmitter {
  constructor() {
    super();
  }

  async init() {
    this.systemTheme = await getSystemTheme();
    store.dispatch('app/updateSystemTheme', this.systemTheme);
    await this.handleEvents();
  }

  async handleEvents() {
    return await getCurrentWindow().onThemeChanged(({ payload: theme }) => {
      this.systemTheme = theme;
      info(`[Motr] nativeTheme updated===>${theme}`);
      // emit('command', {
      //   command: 'application:update-system-theme',
      //   args: [{ theme }],
      // });
      this.emit('system-theme-change', theme);
    });
  }
}
