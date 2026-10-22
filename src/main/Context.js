import { appLogDir } from '@tauri-apps/api/path';
import { platform, arch } from '@tauri-apps/plugin-os';
import { getAria2ConfPath, getSessionPath } from '../utils';

export default class Context {
  async getLogPath() {
    return await appLogDir();
  }

  async init() {
    const [logPath, sessionPath, aria2ConfPath] = await Promise.all([
      this.getLogPath(),
      getSessionPath(),
      getAria2ConfPath(),
    ]);
    // The key of Context cannot be the same as that of userConfig and systemConfig.
    this.context = {
      platform: platform(),
      arch: arch(),
      'log-path': logPath,
      'session-path': sessionPath,
      'engine-path': '',
      'aria2-bin-path': '',
      'aria2-conf-path': aria2ConfPath,
    };

    console.info('[Motrix] Context.init===>', this.context);
  }

  get(key) {
    if (typeof key === 'undefined') {
      return this.context;
    }

    return this.context[key];
  }
}
