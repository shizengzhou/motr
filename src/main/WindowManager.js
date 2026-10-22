import { show } from '@tauri-apps/api/app';
import { getCurrentWindow, primaryMonitor } from '@tauri-apps/api/window';
import { LogicalSize } from '@tauri-apps/api/dpi';
import {
  restoreStateCurrent,
  StateFlags,
} from '@tauri-apps/plugin-window-state';
import { EventEmitter } from 'events';
import { isDialogOpen } from '@/utils/dialog';
import { APP_RUN_MODE } from '../shared/constants';
import is from '@/utils/is';

const DEFAULT_WINDOW_WIDTH = 1024;
const DEFAULT_WINDOW_HEIGHT = 768;

export default class WindowManager extends EventEmitter {
  constructor(options = {}) {
    super();
    this.application = options.application || null;
    this.window = getCurrentWindow();

    this.onFocusChanged();
  }

  onFocusChanged() {
    this.window.onFocusChanged(async ({ payload }) => {
      // 打开原生文件/消息对话框时窗口同样会失焦；这时不能自动隐藏，
      // 否则窗口会藏到对话框后面，关掉对话框也不会自己回来
      if (isDialogOpen()) {
        return;
      }

      const userConfig = await this.application.configManager.getUserConfig();
      if (
        !payload &&
        (userConfig['auto-hide-window'] ||
          userConfig['run-mode'] === APP_RUN_MODE.TRAY)
      ) {
        await this.window.hide();
      }
    });
  }

  /**
   * 「恢复上次退出时的窗口大小和位置」。
   *
   * 只恢复尺寸和位置（对齐原项目 `window.getBounds()` 的 x/y/width/height），
   * 不恢复最大化 / 全屏 / 可见性。是否调用的开关在 Application.initWindowManager。
   */
  async restoreWindowState() {
    await restoreStateCurrent(StateFlags.SIZE | StateFlags.POSITION);
  }

  /**
   * 小屏幕适配：主显示器工作区不够大时把默认窗口缩一档。
   *
   * 对齐上游 Motrix 的 "Optimized for small screen users"：
   * 宽 / 高各自判断 —— 工作区 >= 1280 或 >= 800 的维度保持原尺寸，
   * 不够的维度缩到 87.5%（1024×768 → 896×672），只缩不放。
   *
   * 只在没开「恢复窗口状态」时调用：开了的话以上次保存的 bounds 为准。
   */
  async fitWindowToDisplay() {
    const monitor = await primaryMonitor();
    if (!monitor) {
      console.warn('[Motr] 找不到主显示器，跳过小屏幕窗口适配');
      return;
    }

    const scale = monitor.scaleFactor;
    const workWidth = monitor.workArea.size.width / scale;
    const workHeight = monitor.workArea.size.height / scale;

    const widthScale = workWidth >= 1280 ? 1 : 0.875;
    const heightScale = workHeight >= 800 ? 1 : 0.875;

    if (widthScale === 1 && heightScale === 1) {
      return;
    }

    const width = Math.round(DEFAULT_WINDOW_WIDTH * widthScale);
    const height = Math.round(DEFAULT_WINDOW_HEIGHT * heightScale);

    console.info(
      `[Motr] 屏幕工作区 ${workWidth}×${workHeight} 不足，窗口尺寸缩至 ${width}×${height}`
    );

    await this.window.setSize(new LogicalSize(width, height));
  }

  /**
   * 「隐藏菜单栏」：按 user.json 的 hide-app-menu 设置窗口装饰（原生标题栏）。
   *
   * 仅 Windows / Linux 生效 —— macOS 是 Overlay 标题栏 + 交通灯，不归这个开关管。
   * 勾选 = 去掉原生标题栏（无边框，前端自定义标题栏负责拖动/按钮），
   * 取消 = 恢复原生标题栏。两种状态下边缘拖拽缩放都由 Tauri 接管。
   */
  async hideAppMenu() {
    if (!is.windows() && !is.linux()) {
      return;
    }

    const hide = await this.application.configManager.getUserConfig(
      'hide-app-menu'
    );
    const decorations = !hide;

    // 没变化就不动：每次 setDecorations 都会重算窗口样式
    if ((await this.window.isDecorated()) === decorations) {
      return;
    }

    await this.window.setDecorations(decorations);
  }

  async showWindow() {
    await show();
    await this.window.show();
    await this.window.unminimize();
    await this.window.setFocus();
  }

  async hideWindow() {
    await this.window.hide();
  }
}
