import '@/utils/polyfill';
import Vue from 'vue';
import VueI18Next from '@panter/vue-i18next';
import { sync } from 'vuex-router-sync';
import Element, { Loading, Message } from 'element-ui';
import axios from 'axios';

import { invoke } from '@tauri-apps/api/core';

import App from './App.vue';
import is from '@/utils/is';
import router from '@/router';
import store from '@/store';
import { getLocaleManager } from '@/components/Locale';
import Icon from '@/components/Icons/Icon.vue';
import Msg from '@/components/Msg';
import { commands } from '@/components/CommandManager/instance';
import TrayWorker from '@/workers/tray.worker?worker';

import '@/components/Theme/Index.scss';

// 速率计：渲染进程把「图标 + 上下行速率」画成图（见 workers/tray.worker.js），
// 这里把它交给 Rust 设成托盘图标（等价原 Electron 版的 application:update-tray）
const updateTray = is.isTauri()
  ? async (payload) => {
      const { tray } = payload;
      if (!tray) {
        return;
      }

      const buffer = await tray.arrayBuffer();

      try {
        await invoke('set_tray_speedometer', {
          png: Array.from(new Uint8Array(buffer)),
        });
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
  // if (is.renderer()) {
  //   Vue.use(require('vue-electron'));
  // }

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
  import('./commands');

  app.trayWorker = initTrayWorker();

  // 挂到 window：组件里用 window.app 取这几个对象
  //（原 Electron 版渲染进程里 global === window，vue-electron 会把 app 挂上去）
  window.app = app;

  setTimeout(() => {
    loading.close();
  }, 400);
}

Promise.all([
  store.dispatch('preference/fetchPreference'),
  // 读一次系统主题 + 订阅后续变化（theme=auto 时跟随系统）
  store.dispatch('app/initSystemTheme'),
])
  .then(([config]) => {
    console.info('[Motr] load preference:', config);
    init(config);
  })
  .catch((err) => {
    alert(err);
  });
