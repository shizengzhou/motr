import {
  getCurrent,
  isRegistered,
  onOpenUrl,
  register,
  unregister,
} from '@tauri-apps/plugin-deep-link';
import { revealItemInDir } from '@tauri-apps/plugin-opener';

import is from '@/utils/is';

/// 能从系统递进来、并且能交给「新建任务」的 scheme（对齐原项目）
const RESOURCE_SCHEMES = ['ftp', 'http', 'https', 'magnet', 'thunder'];

/// 自有协议：不受设置页开关控制，永远注册
const OWN_SCHEMES = ['mo', 'motrix'];

/// mo:// / motrix:// 的 host -> 前端命令（对齐原项目 configs/protocol.js）
const OWN_COMMANDS = {
  'task-list': 'application:task-list',
  'new-task': 'application:new-task',
  'new-bt-task': 'application:new-bt-task',
  'pause-all-task': 'application:pause-all-task',
  'resume-all-task': 'application:resume-all-task',
  preferences: 'application:preferences',
  about: 'application:about',
};

const REVEAL_IN_FOLDER = 'reveal-in-folder';

/**
 * 下载协议 / Deep Link 管理（对应原项目 src/main/core/ProtocolManager.js）。
 *
 * - `apply()`：按 user.json 的 protocols 注册/取消注册 magnet、thunder；
 *   `mo` / `motrix` 永远注册。macOS 的协议由打包声明决定，运行时跳过。
 * - `init()`：接住运行中与冷启动的链接（`onOpenUrl` + `getCurrent`），
 *   翻译成前端命令；`Ipc.vue` 就绪后调用 `ready()` 补发。
 *
 * `.torrent` 文件关联仍由 Rust 处理（argv / RunEvent::Opened），
 * 这里只处理 URL scheme。
 */
export default class ProtocolManager {
  constructor(options = {}) {
    this.application = options.application || null;
    this.ready = false;
    this.pending = [];
    this.unlisten = null;
  }

  async init() {
    if (!is.isTauri() || this.unlisten) {
      return;
    }

    this.unlisten = await onOpenUrl((urls) => this.handleUrls(urls));

    // 冷启动就带着链接：插件在 setup 里已经收下了
    const current = await getCurrent().catch((err) => {
      console.warn('[Motr] get current deep link fail:', err);
      return null;
    });

    if (current && current.length > 0) {
      this.handleUrls(current);
    }
  }

  /** Ipc.vue 挂上 command 监听后调用：补发攒下的命令 */
  ready() {
    this.ready = true;

    const pending = this.pending;
    this.pending = [];

    for (const [command, args] of pending) {
      this.dispatch(command, args);
    }
  }

  /**
   * 让系统默认客户端与配置一致。
   * 与原版一致：dev 构建不动注册，macOS 由打包声明决定。
   */
  async apply(protocols = {}) {
    if (!is.isTauri() || is.dev() || is.macOS()) {
      return;
    }

    const wanted = [
      ['magnet', protocols.magnet === true],
      ['thunder', protocols.thunder === true],
      ...OWN_SCHEMES.map((scheme) => [scheme, true]),
    ];

    for (const [scheme, enabled] of wanted) {
      try {
        // eslint-disable-next-line no-await-in-loop
        const registered = await isRegistered(scheme);

        if (enabled && !registered) {
          // eslint-disable-next-line no-await-in-loop
          await register(scheme);
          console.info(`[Motr] \`${scheme}\` 已设为默认客户端`);
        } else if (!enabled && registered) {
          // eslint-disable-next-line no-await-in-loop
          await unregister(scheme);
          console.info(`[Motr] \`${scheme}\` 已取消默认客户端`);
        }
      } catch (err) {
        console.warn(`[Motr] apply protocol \`${scheme}\` fail:`, err);
      }
    }
  }

  handleUrls(urls) {
    for (const raw of urls) {
      this.handleUrl(raw);
    }
  }

  handleUrl(raw) {
    let url;

    try {
      url = new URL(raw);
    } catch (err) {
      console.warn(`[Motr] invalid deep link: ${raw}`, err);
      return;
    }

    // .torrent / 文件打开由 Rust 的 RunEvent::Opened 负责，避免处理两次
    if (url.protocol === 'file:') {
      return;
    }

    this.application?.windowManager?.showWindow?.();

    const scheme = url.protocol.replace(':', '');

    if (RESOURCE_SCHEMES.includes(scheme)) {
      this.send('application:new-task', {
        type: 'uri',
        uri: raw,
      });
      return;
    }

    if (!OWN_SCHEMES.includes(scheme)) {
      return;
    }

    const host = url.hostname;
    const args = Object.fromEntries(url.searchParams.entries());

    if (host === REVEAL_IN_FOLDER) {
      this.revealInFolder(args);
      return;
    }

    const command = OWN_COMMANDS[host];

    if (command) {
      this.send(command, args);
    }
  }

  revealInFolder({ path, gid }) {
    if (path) {
      revealItemInDir(path).catch((err) => {
        console.warn(`[Motr] reveal in folder fail: ${path}`, err);
      });
    }

    if (gid) {
      this.send('application:show-task-detail', { gid });
    }
  }

  send(command, args) {
    if (!command) {
      return;
    }

    if (!this.ready) {
      this.pending.push([command, args]);
      return;
    }

    this.dispatch(command, args);
  }

  dispatch(command, args) {
    this.application?.sendCommandToAll?.(command, args ?? {});
  }

  destroy() {
    if (this.unlisten) {
      this.unlisten();
      this.unlisten = null;
    }
  }
}
