<template>
  <el-container class="content panel" direction="vertical">
    <el-header class="panel-header" height="84">
      <h4 class="hidden-xs-only">{{ title }}</h4>
      <mo-subnav-switcher
        :title="title"
        :subnavs="subnavs"
        class="hidden-sm-and-up"
      />
    </el-header>
    <el-main class="panel-content">
      <el-form
        class="form-preference"
        ref="basicForm"
        label-position="right"
        size="mini"
        :model="form"
        :rules="rules"
      >
        <el-form-item
          :label="`${$t('preferences.appearance')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            <mo-theme-switcher
              v-model="form.theme"
              @change="handleThemeChange"
              ref="themeSwitcher"
            />
          </el-col>
          <el-col v-if="showHideAppMenuOption" class="form-item-sub" :span="16">
            <el-checkbox v-model="form.hideAppMenu">
              {{ $t('preferences.hide-app-menu') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="16">
            <el-checkbox v-model="form.autoHideWindow">
              {{ $t('preferences.auto-hide-window') }}
            </el-checkbox>
          </el-col>
          <el-col v-if="isMac" class="form-item-sub" :span="16">
            <el-checkbox v-model="form.traySpeedometer">
              {{ $t('preferences.tray-speedometer') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="16">
            <el-checkbox v-model="form.showProgressBar">
              {{ $t('preferences.show-progress-bar') }}
            </el-checkbox>
          </el-col>
        </el-form-item>
        <el-form-item
          v-if="isMac"
          :label="`${$t('preferences.run-mode')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            <el-select v-model="form.runMode">
              <el-option
                v-for="item in runModes"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              >
              </el-option>
            </el-select>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.language')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="16">
            <el-select
              v-model="form.locale"
              :placeholder="$t('preferences.change-language')"
            >
              <el-option
                v-for="item in locales"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              >
              </el-option>
            </el-select>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.startup')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24" v-if="!isLinux">
            <el-checkbox v-model="form.openAtLogin">
              {{ $t('preferences.open-at-login') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.keepWindowState">
              {{ $t('preferences.keep-window-state') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.resumeAllWhenAppLaunched">
              {{ $t('preferences.auto-resume-all') }}
            </el-checkbox>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.default-dir')}: `"
          :label-width="formLabelWidth"
        >
          <el-input placeholder="" v-model="form.dir" :readonly="isMas">
            <mo-history-directory
              slot="prepend"
              @selected="handleHistoryDirectorySelected"
            />
            <mo-select-directory
              v-if="isRenderer"
              slot="append"
              @selected="handleNativeDirectorySelected"
            />
          </el-input>
          <div class="el-form-item__info" v-if="isMas" style="margin-top: 8px">
            {{ $t('preferences.mas-default-dir-tips') }}
          </div>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.transfer-settings')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.transfer-speed-upload') }}
            <el-input-number
              v-model="maxOverallUploadLimitParsed"
              controls-position="right"
              :min="0"
              :max="65535"
              :step="1"
              :label="$t('preferences.transfer-speed-download')"
            >
            </el-input-number>
            <el-select
              style="width: 100px"
              v-model="uploadUnit"
              @change="handleUploadChange"
            >
              <el-option
                v-for="item in speedUnits"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              >
              </el-option>
            </el-select>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.transfer-speed-download') }}
            <el-input-number
              v-model="maxOverallDownloadLimitParsed"
              controls-position="right"
              :min="0"
              :max="65535"
              :step="1"
              :label="$t('preferences.transfer-speed-download')"
            >
            </el-input-number>
            <el-select
              style="width: 100px"
              v-model="downloadUnit"
              @change="handleDownloadChange"
            >
              <el-option
                v-for="item in speedUnits"
                :key="item.value"
                :label="item.label"
                :value="item.value"
              >
              </el-option>
            </el-select>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.bt-settings')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.btSaveMetadata">
              {{ $t('preferences.bt-save-metadata') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.btAutoDownloadContent">
              {{ $t('preferences.bt-auto-download-content') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.btForceEncryption">
              {{ $t('preferences.bt-force-encryption') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-switch
              v-model="form.keepSeeding"
              :active-text="$t('preferences.keep-seeding')"
              @change="onKeepSeedingChange"
            >
            </el-switch>
          </el-col>
          <el-col class="form-item-sub" :span="24" v-if="!form.keepSeeding">
            {{ $t('preferences.seed-ratio') }}
            <el-input-number
              v-model="form.seedRatio"
              controls-position="right"
              :min="1"
              :max="100"
              :step="0.1"
              :label="$t('preferences.seed-ratio')"
            >
            </el-input-number>
          </el-col>
          <el-col class="form-item-sub" :span="24" v-if="!form.keepSeeding">
            {{ $t('preferences.seed-time') }}
            ({{ $t('preferences.seed-time-unit') }})
            <el-input-number
              v-model="form.seedTime"
              controls-position="right"
              :min="60"
              :max="525600"
              :step="1"
              :label="$t('preferences.seed-time')"
            >
            </el-input-number>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.task-manage')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.max-concurrent-downloads') }}
            <el-input-number
              v-model="form.maxConcurrentDownloads"
              controls-position="right"
              :min="1"
              :max="maxConcurrentDownloads"
              :label="$t('preferences.max-concurrent-downloads')"
            >
            </el-input-number>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.max-connection-per-server') }}
            <el-input-number
              v-model="form.maxConnectionPerServer"
              controls-position="right"
              :min="1"
              :max="form.engineMaxConnectionPerServer"
              :label="$t('preferences.max-connection-per-server')"
            >
            </el-input-number>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.continue">
              {{ $t('preferences.continue') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.newTaskShowDownloading">
              {{ $t('preferences.new-task-show-downloading') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.taskNotification">
              {{ $t('preferences.task-completed-notify') }}
            </el-checkbox>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.noConfirmBeforeDeleteTask">
              {{ $t('preferences.no-confirm-before-delete-task') }}
            </el-checkbox>
          </el-col>
        </el-form-item>
      </el-form>
      <div class="form-actions">
        <el-button type="primary" @click="submitForm('basicForm')">
          {{ $t('preferences.save') }}
        </el-button>
        <el-button @click="resetForm('basicForm')">
          {{ $t('preferences.discard') }}
        </el-button>
      </div>
    </el-main>
  </el-container>
</template>

<script>
import is from '@/utils/is';
import { confirm } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { mapState } from 'vuex';
import { cloneDeep, extend, isEmpty } from 'lodash';
import SubnavSwitcher from '@/components/Subnav/SubnavSwitcher.vue';
import HistoryDirectory from '@/components/Preference/HistoryDirectory.vue';
import SelectDirectory from '@/components/Native/SelectDirectory.vue';
import ThemeSwitcher from '@/components/Preference/ThemeSwitcher.vue';
import { availableLanguages, getLanguage } from '@shared/locales';
import { getLocaleManager } from '@/components/Locale';
import {
  calcFormLabelWidth,
  changedConfig,
  checkIsNeedRestart,
  convertLineToComma,
  diffConfig,
  extractSpeedUnit,
} from '@shared/utils';
import {
  APP_RUN_MODE,
  EMPTY_STRING,
  ENGINE_MAX_CONCURRENT_DOWNLOADS,
  ENGINE_RPC_PORT,
} from '@shared/constants';
import { reduceTrackerString } from '@shared/utils/tracker';
import { restartApp } from '@/utils/native';

const initForm = (config) => {
  const {
    autoHideWindow,
    btForceEncryption,
    btSaveMetadata,
    dir,
    engineMaxConnectionPerServer,
    followMetalink,
    followTorrent,
    hideAppMenu,
    keepSeeding,
    keepWindowState,
    locale,
    maxConcurrentDownloads,
    maxConnectionPerServer,
    maxOverallDownloadLimit,
    maxOverallUploadLimit,
    newTaskShowDownloading,
    noConfirmBeforeDeleteTask,
    openAtLogin,
    pauseMetadata,
    resumeAllWhenAppLaunched,
    runMode,
    seedRatio,
    seedTime,
    showProgressBar,
    taskNotification,
    theme,
    traySpeedometer,
  } = config;

  const btAutoDownloadContent =
    followTorrent && followMetalink && !pauseMetadata;

  const result = {
    autoHideWindow,
    btAutoDownloadContent,
    btForceEncryption,
    btSaveMetadata,
    continue: config.continue,
    dir,
    engineMaxConnectionPerServer,
    followMetalink,
    followTorrent,
    hideAppMenu,
    keepSeeding,
    keepWindowState,
    locale,
    maxConcurrentDownloads,
    maxConnectionPerServer,
    maxOverallDownloadLimit,
    maxOverallUploadLimit,
    newTaskShowDownloading,
    noConfirmBeforeDeleteTask,
    openAtLogin,
    pauseMetadata,
    resumeAllWhenAppLaunched,
    runMode,
    seedRatio,
    seedTime,
    showProgressBar,
    taskNotification,
    theme,
    traySpeedometer,
  };
  return result;
};

export default {
  name: 'mo-preference-basic',
  components: {
    [SubnavSwitcher.name]: SubnavSwitcher,
    [HistoryDirectory.name]: HistoryDirectory,
    [SelectDirectory.name]: SelectDirectory,
    [ThemeSwitcher.name]: ThemeSwitcher,
  },
  data() {
    const { locale } = this.$store.state.preference.config;
    const formOriginal = initForm(this.$store.state.preference.config);
    let form = {};
    form = initForm(extend(form, formOriginal, changedConfig.basic));

    return {
      form,
      formLabelWidth: calcFormLabelWidth(locale),
      formOriginal,
      locales: availableLanguages,
      rules: {},
    };
  },
  computed: {
    isRenderer: () => is.isTauri(),
    isMac: () => is.macOS(),
    isMas: () => is.mas(),
    isLinux() {
      return is.linux();
    },
    title() {
      return this.$t('preferences.basic');
    },
    maxConcurrentDownloads() {
      return ENGINE_MAX_CONCURRENT_DOWNLOADS;
    },
    maxOverallDownloadLimitParsed: {
      get() {
        return parseInt(this.form.maxOverallDownloadLimit);
      },
      set(value) {
        const limit = value > 0 ? `${value}${this.downloadUnit}` : 0;
        this.form.maxOverallDownloadLimit = limit;
      },
    },
    maxOverallUploadLimitParsed: {
      get() {
        return parseInt(this.form.maxOverallUploadLimit);
      },
      set(value) {
        const limit = value > 0 ? `${value}${this.uploadUnit}` : 0;
        this.form.maxOverallUploadLimit = limit;
      },
    },
    downloadUnit: {
      get() {
        const { maxOverallDownloadLimit } = this.form;
        return extractSpeedUnit(maxOverallDownloadLimit);
      },
      set(value) {
        return value;
      },
    },
    uploadUnit: {
      get() {
        const { maxOverallUploadLimit } = this.form;
        return extractSpeedUnit(maxOverallUploadLimit);
      },
      set(value) {
        return value;
      },
    },
    runModes() {
      let result = [
        {
          label: this.$t('preferences.run-mode-standard'),
          value: APP_RUN_MODE.STANDARD,
        },
        {
          label: this.$t('preferences.run-mode-tray'),
          value: APP_RUN_MODE.TRAY,
        },
      ];

      if (this.isMac) {
        result = [
          ...result,
          {
            label: this.$t('preferences.run-mode-hide-tray'),
            value: APP_RUN_MODE.HIDE_TRAY,
          },
        ];
      }

      return result;
    },
    speedUnits() {
      return [
        {
          label: 'KB/s',
          value: 'K',
        },
        {
          label: 'MB/s',
          value: 'M',
        },
      ];
    },
    subnavs() {
      return [
        {
          key: 'basic',
          title: this.$t('preferences.basic'),
          route: '/preference/basic',
        },
        {
          key: 'advanced',
          title: this.$t('preferences.advanced'),
          route: '/preference/advanced',
        },
        {
          key: 'lab',
          title: this.$t('preferences.lab'),
          route: '/preference/lab',
        },
      ];
    },
    showHideAppMenuOption() {
      return is.windows() || is.linux();
    },
    rpcDefaultPort() {
      return ENGINE_RPC_PORT;
    },
    ...mapState('preference', {
      config: (state) => state.config,
    }),
  },
  methods: {
    handleLocaleChange(locale) {
      const lng = getLanguage(locale);
      getLocaleManager().changeLanguage(lng);
    },
    handleThemeChange(theme) {
      this.form.theme = theme;
    },
    handleDownloadChange(value) {
      const speedLimit = parseInt(this.form.maxOverallDownloadLimit, 10);
      this.downloadUnit = value;
      const limit = speedLimit > 0 ? `${speedLimit}${value}` : 0;
      this.form.maxOverallDownloadLimit = limit;
    },
    handleUploadChange(value) {
      const speedLimit = parseInt(this.form.maxOverallUploadLimit, 10);
      this.uploadUnit = value;
      const limit = speedLimit > 0 ? `${speedLimit}${value}` : 0;
      this.form.maxOverallUploadLimit = limit;
    },
    onKeepSeedingChange(enable) {
      this.form.seedRatio = enable ? 0 : 1;
      this.form.seedTime = enable ? 525600 : 60;
    },
    handleHistoryDirectorySelected(dir) {
      this.form.dir = dir;
    },
    handleNativeDirectorySelected(dir) {
      this.form.dir = dir;
      this.$store.dispatch('preference/recordHistoryDirectory', dir);
    },
    onDirectorySelected(dir) {
      this.form.dir = dir;
    },
    syncFormConfig() {
      this.$store.dispatch('preference/fetchPreference').then((config) => {
        this.form = initForm(config);
        this.formOriginal = cloneDeep(this.form);
      });
    },
    submitForm(formName) {
      this.$refs[formName].validate((valid) => {
        if (!valid) {
          console.error('[Motr] preference form valid:', valid);
          return false;
        }

        const data = {
          ...diffConfig(this.formOriginal, this.form),
          ...changedConfig.advanced,
        };

        const { btAutoDownloadContent, btTracker, rpcListenPort } = data;

        if ('btAutoDownloadContent' in data) {
          data.followTorrent = btAutoDownloadContent;
          data.followMetalink = btAutoDownloadContent;
          data.pauseMetadata = !btAutoDownloadContent;
        }

        if (btTracker) {
          data.btTracker = reduceTrackerString(convertLineToComma(btTracker));
        }

        if (rpcListenPort === EMPTY_STRING) {
          data.rpcListenPort = this.rpcDefaultPort;
        }

        console.log('[Motr] preference changed data:', data);

        this.$store
          .dispatch('preference/save', data)
          .then(() => {
            // 运行模式（托盘是否创建 / Dock 图标是否显示）由 Rust 侧读 user.json 再应用，
            // 所以必须等配置落盘之后再调（见 src-tauri/src/run_mode.rs）
            if ('runMode' in data) {
              invoke('apply_run_mode').catch((err) => {
                console.warn('[Motr] apply run mode fail:', err);
              });
            }

            // 开机自启同理：由 Rust 侧读 user.json 后改写系统登录项
            //（见 src-tauri/src/auto_launch.rs）
            if ('openAtLogin' in data) {
              invoke('apply_auto_launch').catch((err) => {
                console.warn('[Motr] apply auto launch fail:', err);
              });
            }

            // 「隐藏菜单栏」控制窗口装饰（原生标题栏）：勾选=无边框，取消=恢复原生标题栏。
            // 由 Rust 侧读 user.json 后 setDecorations（见 src-tauri/src/window.rs 的
            // apply_hide_app_menu）；前端自定义标题栏上的窗口按钮走 Vue 响应式，自己会更新
            if ('hideAppMenu' in data) {
              invoke('apply_hide_app_menu').catch((err) => {
                console.warn('[Motr] apply hide app menu fail:', err);
              });
            }

            this.$store.dispatch('app/fetchEngineOptions');
            this.syncFormConfig();
            this.$msg.success(this.$t('preferences.save-success-message'));
          })
          .catch((err) => {
            console.log('[Motr] preference save error:', err);
            this.$msg.success(this.$t('preferences.save-fail-message'));
          });

        changedConfig.basic = {};
        changedConfig.advanced = {};

        if (this.isRenderer) {
          // 「自动隐藏窗口」不需要通知主进程：Rust 侧在窗口失焦时现读 user.json
          //（见 src-tauri/src/window.rs），保存后立即生效

          if (checkIsNeedRestart(data)) {
            restartApp();
          }
        }
      });
    },
    resetForm() {
      this.syncFormConfig();
    },
  },
  beforeRouteLeave(to, from, next) {
    changedConfig.basic = diffConfig(this.formOriginal, this.form);
    if (to.path === '/preference/advanced') {
      next();
    } else {
      if (isEmpty(changedConfig.basic) && isEmpty(changedConfig.advanced)) {
        next();
      } else {
        confirm(this.$t('preferences.not-saved-confirm'), {
          title: this.$t('preferences.not-saved'),
          kind: 'warning',
          okLabel: this.$t('app.yes'),
          cancelLabel: this.$t('app.no'),
        }).then((okClicked) => {
          if (okClicked) {
            changedConfig.basic = {};
            changedConfig.advanced = {};
            next();
          }
        });
      }
    }
  },
};
</script>
