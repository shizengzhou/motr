<template>
  <div id="app">
    <mo-title-bar v-if="isRenderer" :showActions="showWindowActions" />
    <router-view />
    <mo-engine-client :secret="rpcSecret" />
    <mo-ipc v-if="isRenderer" />
    <mo-dynamic-tray v-if="enableTraySpeedometer" />
  </div>
</template>

<script>
import { mapGetters, mapState } from 'vuex';
import { invoke } from '@tauri-apps/api/core';
import is from '@/utils/is';
import { APP_RUN_MODE, APP_THEME } from '@shared/constants';
import DynamicTray from '@/components/Native/DynamicTray.vue';
import EngineClient from '@/components/Native/EngineClient.vue';
import Ipc from '@/components/Native/Ipc.vue';
import TitleBar from '@/components/Native/TitleBar.vue';
import { getLanguage } from '@shared/locales';
import { getLocaleManager } from '@/components/Locale';

export default {
  name: 'motrix-app',
  components: {
    [DynamicTray.name]: DynamicTray,
    [EngineClient.name]: EngineClient,
    [Ipc.name]: Ipc,
    [TitleBar.name]: TitleBar,
  },
  computed: {
    isMac: () => is.macOS(),
    isRenderer: () => is.isTauri(),
    ...mapState('app', {
      systemTheme: (state) => state.systemTheme,
    }),
    ...mapState('preference', {
      showWindowActions: (state) => {
        return (is.windows() || is.linux()) && state.config.hideAppMenu;
      },
      runMode: (state) => state.config.runMode,
      traySpeedometer: (state) => state.config.traySpeedometer,
      rpcSecret: (state) => state.config.rpcSecret,
    }),
    ...mapGetters('preference', ['theme', 'locale', 'direction']),
    themeClass() {
      if (this.theme === APP_THEME.AUTO) {
        return `theme-${this.systemTheme}`;
      } else {
        return `theme-${this.theme}`;
      }
    },
    i18nClass() {
      return `i18n-${this.locale}`;
    },
    directionClass() {
      return `dir-${this.direction}`;
    },
    enableTraySpeedometer() {
      const { isMac, isRenderer, traySpeedometer, runMode } = this;
      return (
        isMac &&
        isRenderer &&
        traySpeedometer &&
        runMode !== APP_RUN_MODE.HIDE_TRAY
      );
    },
  },
  methods: {
    // 原生菜单（App 菜单 + 托盘菜单）的语言由 Rust 侧构建，
    // 这里只把语言值同步过去，让它按新语言重建菜单
    rebuildNativeMenus(locale) {
      if (!this.isRenderer) {
        return;
      }

      invoke('rebuild_menus', { locale }).catch((err) => {
        console.warn('[Motr] rebuild native menus fail:', err);
      });
    },
    updateRootClassName() {
      const { themeClass = '', i18nClass = '', directionClass = '' } = this;
      const className = `${themeClass} ${i18nClass} ${directionClass}`;
      document.documentElement.className = className;
    },
  },
  beforeMount() {
    this.updateRootClassName();
  },
  watch: {
    locale(val) {
      const lng = getLanguage(val);
      getLocaleManager().changeLanguage(lng);
      this.rebuildNativeMenus(lng);
    },
    themeClass() {
      this.updateRootClassName();
    },
    i18nClass() {
      this.updateRootClassName();
    },
    directionClass() {
      this.updateRootClassName();
    },
  },
};
</script>

<style></style>
