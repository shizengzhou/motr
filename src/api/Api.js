import { isEmpty, clone } from 'lodash';
import { invoke } from '@tauri-apps/api/core';
import { emit } from '@tauri-apps/api/event';
import is from '@/utils/is';
import { aria2ProxyOptions } from '@/utils/proxy';
import { Aria2 } from '@shared/aria2';
import {
  separateConfig,
  compactUndefined,
  formatOptionsForEngine,
  mergeTaskResult,
  changeKeysToCamelCase,
  changeKeysToKebabCase,
} from '@shared/utils';
import { toElectronLocale } from '../utils';
import { ENGINE_RPC_HOST } from '../shared/constants';

export default class Api {
  constructor(options = {}) {
    this.options = options;

    this.init();
  }

  async init() {
    this.config = await this.loadConfig();

    this.client = this.initClient();
    this.client.open();
  }

  loadConfigFromLocalStorage() {
    // TODO
    const result = {};
    return result;
  }

  async loadConfigFromNativeStore() {
    const result = await invoke('get_app_config');
    return { ...result, locale: toElectronLocale(result.locale) };
  }

  async loadConfig() {
    let result = is.isTauri()
      ? await this.loadConfigFromNativeStore()
      : this.loadConfigFromLocalStorage();

    result = changeKeysToCamelCase(result);
    return result;
  }

  initClient() {
    const { rpcListenPort: port, rpcSecret: secret } = this.config;
    const host = ENGINE_RPC_HOST;
    return new Aria2({
      host,
      port,
      secret,
    });
  }

  closeClient() {
    this.client
      .close()
      .then(() => {
        this.client = null;
      })
      .catch((err) => {
        console.log('engine client close fail', err);
      });
  }

  fetchPreference() {
    return new Promise((resolve) => {
      this.config = this.loadConfig();
      resolve(this.config);
    });
  }

  savePreference(params = {}) {
    const kebabParams = changeKeysToKebabCase(params);
    if (is.isTauri()) {
      return this.savePreferenceToNativeStore(kebabParams);
    } else {
      return this.savePreferenceToLocalStorage(kebabParams);
    }
  }

  savePreferenceToLocalStorage() {
    // TODO
  }

  async savePreferenceToNativeStore(params = {}) {
    // 代理：把 proxy 映射成 aria2 的 all-proxy/no-proxy 一起下发
    //（原项目 Application.watchProxyChange 做的是同一件事），这样运行中的引擎立刻跟着变，
    // system.json 里也留下同一份值
    if (params.proxy) {
      Object.assign(params, aria2ProxyOptions(params.proxy));
    }

    const { user, system, others } = separateConfig(params);

    if (!isEmpty(user)) {
      console.info('[Motr] save user config: ', user);
    }

    if (!isEmpty(system)) {
      console.info('[Motr] save system config: ', system);
      this.updateActiveTaskOption(system);
    }

    if (!isEmpty(others)) {
      console.info('[Motr] save config found illegal key: ', others);
    }

    // 落盘统一交给 Rust（它是 user.json / system.json 的唯一写入者）：
    // 前端直接用 LazyStore 写文件的话，Rust 侧的 store 缓存会变旧 —— 读不到新值，
    // 而且退出时 tauri-plugin-store 会把旧缓存写回文件（"改完设置要重启才生效"
    // 和 "last-check-update-time 推不动" 都是这么来的，见 config_manager::write_keys）
    await invoke('save_config', { user, system });

    if (!isEmpty(system)) {
      this.changeGlobalOption(system);
    }

    if (!isEmpty(system) || !isEmpty(user)) {
      emit('command', {
        command: 'application:update-preference-config',
      });
    }
  }

  getVersion() {
    return this.client.call('getVersion');
  }

  changeGlobalOption(options) {
    const args = formatOptionsForEngine(options);

    return this.client.call('changeGlobalOption', args);
  }

  getGlobalOption() {
    return new Promise((resolve) => {
      this.client.call('getGlobalOption').then((data) => {
        resolve(changeKeysToCamelCase(data));
      });
    });
  }

  getOption(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);

    return new Promise((resolve) => {
      this.client.call('getOption', ...args).then((data) => {
        resolve(changeKeysToCamelCase(data));
      });
    });
  }

  updateActiveTaskOption(options) {
    this.fetchTaskList({ type: 'active' }).then((data) => {
      if (isEmpty(data)) {
        return;
      }

      const gids = data.map((task) => task.gid);
      this.batchChangeOption({ gids, options });
    });
  }

  changeOption(params = {}) {
    const { gid, options = {} } = params;

    const engineOptions = formatOptionsForEngine(options);
    const args = compactUndefined([gid, engineOptions]);

    return this.client.call('changeOption', ...args);
  }

  getGlobalStat() {
    return this.client.call('getGlobalStat');
  }

  addUri(params) {
    const { uris, outs, options } = params;
    const tasks = uris.map((uri, index) => {
      const engineOptions = formatOptionsForEngine(options);
      if (outs && outs[index]) {
        engineOptions.out = outs[index];
      }
      const args = compactUndefined([[uri], engineOptions]);
      return ['aria2.addUri', ...args];
    });
    return this.client.multicall(tasks);
  }

  addTorrent(params) {
    const { torrent, options } = params;
    const engineOptions = formatOptionsForEngine(options);
    const args = compactUndefined([torrent, [], engineOptions]);
    return this.client.call('addTorrent', ...args);
  }

  addMetalink(params) {
    const { metalink, options } = params;
    const engineOptions = formatOptionsForEngine(options);
    const args = compactUndefined([metalink, engineOptions]);
    return this.client.call('addMetalink', ...args);
  }

  fetchDownloadingTaskList(params = {}) {
    const { offset = 0, num = 20, keys } = params;
    const activeArgs = compactUndefined([keys]);
    const waitingArgs = compactUndefined([offset, num, keys]);
    return new Promise((resolve, reject) => {
      this.client
        .multicall([
          ['aria2.tellActive', ...activeArgs],
          ['aria2.tellWaiting', ...waitingArgs],
        ])
        .then((data) => {
          console.log('[Motr] fetch downloading task list data:', data);
          const result = mergeTaskResult(data);
          resolve(result);
        })
        .catch((err) => {
          console.log('[Motr] fetch downloading task list fail:', err);
          reject(err);
        });
    });
  }

  fetchWaitingTaskList(params = {}) {
    const { offset = 0, num = 20, keys } = params;
    const args = compactUndefined([offset, num, keys]);
    return this.client.call('tellWaiting', ...args);
  }

  fetchStoppedTaskList(params = {}) {
    const { offset = 0, num = 20, keys } = params;
    const args = compactUndefined([offset, num, keys]);
    return this.client.call('tellStopped', ...args);
  }

  fetchActiveTaskList(params = {}) {
    const { keys } = params;
    const args = compactUndefined([keys]);
    return this.client.call('tellActive', ...args);
  }

  fetchTaskList(params = {}) {
    const { type } = params;
    switch (type) {
      case 'active':
        return this.fetchDownloadingTaskList(params);
      case 'waiting':
        return this.fetchWaitingTaskList(params);
      case 'stopped':
        return this.fetchStoppedTaskList(params);
      default:
        return this.fetchDownloadingTaskList(params);
    }
  }

  fetchTaskItem(params = {}) {
    const { gid, keys } = params;
    const args = compactUndefined([gid, keys]);
    return this.client.call('tellStatus', ...args);
  }

  fetchTaskItemWithPeers(params = {}) {
    const { gid, keys } = params;
    const statusArgs = compactUndefined([gid, keys]);
    const peersArgs = compactUndefined([gid]);
    return new Promise((resolve, reject) => {
      this.client
        .multicall([
          ['aria2.tellStatus', ...statusArgs],
          ['aria2.getPeers', ...peersArgs],
        ])
        .then((data) => {
          console.log('[Motr] fetchTaskItemWithPeers:', data);
          const result = data[0] && data[0][0];
          const peers = data[1] && data[1][0];
          result.peers = peers || [];
          console.log('[Motr] fetchTaskItemWithPeers.result:', result);
          console.log('[Motr] fetchTaskItemWithPeers.peers:', peers);

          resolve(result);
        })
        .catch((err) => {
          console.log('[Motr] fetch downloading task list fail:', err);
          reject(err);
        });
    });
  }

  fetchTaskItemPeers(params = {}) {
    const { gid, keys } = params;
    const args = compactUndefined([gid, keys]);
    return this.client.call('getPeers', ...args);
  }

  pauseTask(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);
    return this.client.call('pause', ...args);
  }

  pauseAllTask() {
    return this.client.call('pauseAll');
  }

  forcePauseTask(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);
    return this.client.call('forcePause', ...args);
  }

  forcePauseAllTask() {
    return this.client.call('forcePauseAll');
  }

  resumeTask(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);
    return this.client.call('unpause', ...args);
  }

  resumeAllTask() {
    return this.client.call('unpauseAll');
  }

  removeTask(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);
    return this.client.call('remove', ...args);
  }

  forceRemoveTask(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);
    return this.client.call('forceRemove', ...args);
  }

  saveSession() {
    return this.client.call('saveSession');
  }

  purgeTaskRecord() {
    return this.client.call('purgeDownloadResult');
  }

  removeTaskRecord(params = {}) {
    const { gid } = params;
    const args = compactUndefined([gid]);
    return this.client.call('removeDownloadResult', ...args);
  }

  multicall(method, params = {}) {
    let { gids, options = {} } = params;
    options = formatOptionsForEngine(options);

    const data = gids.map((gid) => {
      const _options = clone(options);
      const args = compactUndefined([gid, _options]);
      return [method, ...args];
    });
    return this.client.multicall(data);
  }

  batchChangeOption(params = {}) {
    return this.multicall('aria2.changeOption', params);
  }

  batchRemoveTask(params = {}) {
    return this.multicall('aria2.remove', params);
  }

  batchResumeTask(params = {}) {
    return this.multicall('aria2.unpause', params);
  }

  batchPauseTask(params = {}) {
    return this.multicall('aria2.pause', params);
  }

  batchForcePauseTask(params = {}) {
    return this.multicall('aria2.forcePause', params);
  }
}
