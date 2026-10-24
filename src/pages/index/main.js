import '@/utils/polyfill';
import Vue from 'vue';
import VueI18Next from '@panter/vue-i18next';
import { sync } from 'vuex-router-sync';
import Element, { Loading, Message } from 'element-ui';
import axios from 'axios';

import App from './App.vue';
import is from '@/utils/is';
import router from '@/router';
import store from '@/store';
import { getLocaleManager } from '@/components/Locale';
import Icon from '@/components/Icons/Icon.vue';
import Msg from '@/components/Msg';
import { commands } from '@/components/CommandManager/instance';
// 命令注册必须**同步**完成，所以这里是静态导入。
//
// 原来是 `import('./commands')`（没 await 的动态导入）：Vite 会把它拆成独立 chunk
// 异步加载，而 Ipc.vue 一挂上 `command` 监听就立刻 invoke('protocol_ready')，
// Rust 侧随即补发攒下的命令 —— 那一刻命令表还是空的，`execute()` 找不到处理函数、
// 静默什么都不做。表现就是"右键用 Motr 打开种子，弹窗不出现"（冷启动必现，
// 因为动态导入在 $mount 之后才被触发）。
import './commands';
import TrayWorker from '@/workers/tray.worker?worker';
import { APP_RUN_MODE } from '@shared/constants';
import Application from '../../main/Application';
import api from '../../api';

import '@/components/Theme/Index.scss';

// 速率计是否该生效：与 App.vue 的 enableTraySpeedometer 同一套判断。
// 关开关时组件会卸载（并恢复静态图标），但 worker 里可能还有一次「已画完」
// 的消息在路上；若照旧转发，刚恢复的静态图标又会被速率图盖回去，
// 表现出来就是「速率先消失又出现」，而且组件已卸载、没人再撤销它。
const traySpeedometerEnabled = () =>
  is.macOS() &&
  store.state.preference.config.traySpeedometer === true &&
  store.state.preference.config.runMode !== APP_RUN_MODE.HIDE_TRAY;

// 速率计：渲染进程把「图标 + 上下行速率」画成图（见 workers/tray.worker.js），
// 这里把它交给 Rust 设成托盘图标（等价原 Electron 版的 application:update-tray）
const updateTray = is.isTauri()
  ? async (payload) => {
      if (!traySpeedometerEnabled()) {
        return;
      }

      const { tray } = payload;
      if (!tray) {
        return;
      }

      const buffer = await tray.arrayBuffer();

      try {
        await window.application?.trayManager?.setSpeedometer(
          new Uint8Array(buffer)
        );
      } catch (err) {
        console.warn('[Motr] update tray speedometer fail:', err);
      }
    }
  : () => {};

function initTrayWorker() {
  const worker = new TrayWorker();

  worker.addEventListener('message', (event) => {
    const { type, payload } = event.data;

    switch (type) {
      case 'initialized':
      case 'log':
        console.log('[Motr] Log from Tray Worker: ', payload);
        break;
      case 'tray:drawed':
        updateTray(payload);
        break;
      default:
        console.warn(
          '[Motr] Tray Worker unhandled message type:',
          type,
          payload
        );
    }
  });

  return worker;
}

function init(config) {
  Vue.http = Vue.prototype.$http = axios;
  Vue.config.productionTip = false;

  const { locale } = config;
  const localeManager = getLocaleManager();
  localeManager.changeLanguageByLocale(locale);

  Vue.use(VueI18Next);
  const i18n = new VueI18Next(localeManager.getI18n());
  Vue.use(Element, {
    size: 'mini',
    i18n: (key, value) => i18n.t(key, value),
  });
  Vue.use(Msg, Message, {
    showClose: true,
  });
  Vue.component('mo-icon', Icon);

  const loading = Loading.service({
    fullscreen: true,
    background: 'rgba(0, 0, 0, 0.1)',
  });

  sync(store, router);

  const app = new Vue({
    components: { App },
    router,
    store,
    i18n,
    render: (h) => h(App),
  }).$mount('#app');

  app.commands = commands;

  app.trayWorker = initTrayWorker();

  // 挂到 window：组件里用 window.app 取这几个对象
  //（原 Electron 版渲染进程里 global === window，vue-electron 会把 app 挂上去）
  window.app = app;

  setTimeout(() => {
    loading.close();
  }, 400);
}

const application = new Application();
window.application = application;

application
  .init()
  .then(async () => {
    const config = await api.loadConfig();
    console.info('[Motr] load preference:', config);

    // 与原项目保持一致，首次即使是 Tray 模式也显示窗口
    // if (config.runMode !== APP_RUN_MODE.TRAY) {
    application.windowManager.showWindow();
    // }
    application.updateManager.check(config);

    store.dispatch('preference/updatePreference', config);
    init(config);
  })
  .catch((err) => {
    alert(err);
  });
