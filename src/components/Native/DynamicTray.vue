<template>
  <div style="display: none">
    <img id="tray-icon-light-normal" src="static/mo-tray-light-normal@2x.png" />
    <img id="tray-icon-light-active" src="static/mo-tray-light-active@2x.png" />
    <img id="tray-icon-dark-normal" src="static/mo-tray-dark-normal@2x.png" />
    <img id="tray-icon-dark-active" src="static/mo-tray-dark-active@2x.png" />
  </div>
</template>

<script>
import { mapState } from 'vuex';
import { invoke } from '@tauri-apps/api/core';

import { getInverseTheme } from '@shared/utils';
import { APP_THEME } from '@shared/constants';

const cache = {};

export default {
  name: 'mo-dynamic-tray',
  computed: {
    ...mapState('app', {
      iconStatus: (state) => (state.stat.numActive > 0 ? 'active' : 'normal'),
      theme: (state) => state.systemTheme,
      focused: (state) => state.trayFocused,
      uploadSpeed: (state) => state.stat.uploadSpeed,
      downloadSpeed: (state) => state.stat.downloadSpeed,
      speed: (state) => state.stat.uploadSpeed + state.stat.downloadSpeed,
    }),
    scale() {
      return 2;
    },
    currentTheme() {
      const { theme, focused } = this;
      if (theme === APP_THEME.DARK) {
        return theme;
      }

      return focused ? getInverseTheme(theme) : theme;
    },
    iconKey() {
      const { bigSur, iconStatus, currentTheme } = this;
      return bigSur
        ? 'tray-icon-light-normal'
        : `tray-icon-${currentTheme}-${iconStatus}`;
    },
  },
  watch: {
    async speed() {
      await this.drawTray();
    },
    async iconKey() {
      await this.drawTray();
    },
  },
  mounted() {
    setTimeout(async () => {
      await this.drawTray();
    }, 200);
  },
  destroyed() {
    // 关掉「托盘显示实时速度」或退出时，把托盘恢复成静态图标
    // （原 Electron 版对应 TrayManager.handleSpeedometerEnableChange -> renderTray）
    invoke('reset_tray_icon').catch((err) => {
      console.warn('[Motr] reset tray icon fail:', err);
    });
  },
  methods: {
    async getIcon(key) {
      if (cache[key]) {
        return cache[key];
      }

      const iconImage = document.getElementById(key);
      const result = await createImageBitmap(iconImage);
      cache[key] = result;

      return result;
    },
    async drawTray() {
      const {
        currentTheme: theme,
        uploadSpeed,
        downloadSpeed,
        scale,
        iconKey,
      } = this;

      const icon = await this.getIcon(iconKey);

      // worker 由 pages/index/main.js 初始化后挂到 window.app 上
      const worker = window.app && window.app.trayWorker;
      if (!worker) {
        console.warn('[Motr] tray worker 未就绪，跳过本次托盘绘制');
        return;
      }

      worker.postMessage({
        type: 'tray:draw',
        payload: {
          theme,
          icon,
          uploadSpeed,
          downloadSpeed,
          scale,
        },
      });
    },
  },
};
</script>
