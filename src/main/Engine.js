import {
  exists,
  BaseDirectory,
  writeTextFile,
  remove,
} from '@tauri-apps/plugin-fs';
import { info, warn, error } from '@tauri-apps/plugin-log';
import { Command } from '@tauri-apps/plugin-shell';
import { getAria2ConfPath, getSessionPath, transformConfig } from '../utils';
import is from '../utils/is';

const enginePidFile = 'engine.pid';

export default class Engine {
  constructor(options = {}) {
    this.systemConfig = options.systemConfig;
    this.userConfig = options.userConfig;
  }

  async start() {
    if (this.instance) {
      return;
    }

    const args = await this.getStartArgs();
    // 名字必须和 tauri.conf.json 的 bundle.externalBin 条目完全一致
    //（那边写的是 "binaries/aria2c"，传 'aria2c' 会被插件判定为未配置）
    const command = Command.sidecar('binaries/aria2c', args);
    this.instance = await command.spawn();
    const pid = this.instance.pid.toString();
    this.writePidFile(pid);

    command.once('close', async () => {
      try {
        await remove(enginePidFile, { baseDir: BaseDirectory.AppData });
      } catch (err) {
        warn(`[Motr] Remove engine process pid file failed: ${err}`);
      }
    });

    if (is.dev()) {
      command.stdout.on('data', (data) => {
        info(`[Motr] engine stdout===>${data.toString()}`);
      });

      command.stderr.on('data', (data) => {
        info(`[Motr] engine stderr===>${data.toString()}`);
      });
    }
  }

  async stop() {
    info('[Motr] engine.stop.instance');
    if (this.instance) {
      try {
        await this.instance.kill();
        this.instance = null;
      } catch (err) {
        warn(`[Motr] engine.stop failed: ${err}`);
      }
    }
  }

  async restart() {
    await this.stop();
    await this.start();
  }

  writePidFile(pid) {
    writeTextFile(enginePidFile, pid, { baseDir: BaseDirectory.AppData }).catch(
      (err) => {
        error(`[Motr] Write engine process pid failed: ${err}`);
      }
    );
  }

  async getStartArgs() {
    const [confPath, sessionPath, sessionIsExist] = await Promise.all([
      getAria2ConfPath(),
      getSessionPath(),
      exists('download.session', { baseDir: BaseDirectory.AppData }),
    ]);

    let result = [`--conf-path=${confPath}`, `--save-session=${sessionPath}`];
    if (sessionIsExist) {
      result = [...result, `--input-file=${sessionPath}`];
    }

    const extraConfig = {
      ...this.systemConfig,
    };
    const keepSeeding = this.userConfig['keep-seeding'];
    const seedRatio = this.systemConfig['seed-ratio'];
    if (keepSeeding || seedRatio === 0) {
      extraConfig['seed-ratio'] = 0;
      delete extraConfig['seed-time'];
    }
    console.log('extraConfig===>', extraConfig);

    const extra = transformConfig(extraConfig);
    result = [...result, ...extra];

    return result;
  }
}
