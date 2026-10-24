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
        ref="advancedForm"
        label-position="right"
        size="mini"
        :model="form"
        :rules="rules"
      >
        <el-form-item
          :label="`${$t('preferences.auto-update')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            <el-checkbox v-model="form.autoCheckUpdate">
              {{ $t('preferences.auto-check-update') }}
            </el-checkbox>
            <div
              class="el-form-item__info"
              style="margin-top: 8px"
              v-if="form.lastCheckUpdateTime !== 0"
            >
              {{
                $t('preferences.last-check-update-time') +
                ': ' +
                (form.lastCheckUpdateTime !== 0
                  ? new Date(form.lastCheckUpdateTime).toLocaleString()
                  : new Date().toLocaleString())
              }}
              <span class="action-link" @click.prevent="onCheckUpdateClick">
                {{ $t('app.check-updates-now') }}
              </span>
            </div>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.proxy')}: `"
          :label-width="formLabelWidth"
        >
          <el-switch
            v-model="form.useProxy"
            :active-text="$t('preferences.enable-proxy')"
            @change="onUseProxyChange"
          >
          </el-switch>
        </el-form-item>
        <el-form-item
          :label-width="formLabelWidth"
          v-if="form.useProxy"
          style="margin-top: -16px"
        >
          <el-col class="form-item-sub" :xs="24" :sm="20" :md="16" :lg="16">
            <el-input
              placeholder="[http://][USER:PASSWORD@]HOST[:PORT]"
              @change="onAllProxyBackupChange"
              v-model="form.allProxyBackup"
            >
            </el-input>
          </el-col>
          <el-col class="form-item-sub" :xs="24" :sm="24" :md="20" :lg="20">
            <el-input
              type="textarea"
              rows="2"
              auto-complete="off"
              :placeholder="`${$t('preferences.proxy-bypass-input-tips')}`"
              v-model="form.noProxy"
            >
            </el-input>
            <div class="el-form-item__info" style="margin-top: 8px">
              <a
                target="_blank"
                href="https://github.com/agalwood/Motr/wiki/Proxy"
                rel="noopener noreferrer"
              >
                {{ $t('preferences.proxy-tips') }}
                <mo-icon name="link" width="12" height="12" />
              </a>
            </div>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.bt-tracker')}: `"
          :label-width="formLabelWidth"
        >
          <div class="form-item-sub bt-tracker">
            <el-row :gutter="10" style="line-height: 0">
              <el-col :span="20">
                <div class="track-source">
                  <el-select
                    class="select-track-source"
                    v-model="form.trackerSource"
                    allow-create
                    filterable
                    multiple
                  >
                    <el-option-group
                      v-for="group in trackerSourceOptions"
                      :key="group.label"
                      :label="group.label"
                    >
                      <el-option
                        v-for="item in group.options"
                        :key="item.value"
                        :label="item.label"
                        :value="item.value"
                      >
                        <span style="float: left">{{ item.label }}</span>
                        <span style="float: right; margin-right: 24px">
                          <el-tag type="success" size="mini" v-if="item.cdn">
                            CDN
                          </el-tag>
                        </span>
                      </el-option>
                    </el-option-group>
                  </el-select>
                </div>
              </el-col>
              <el-col :span="3">
                <div class="sync-tracker">
                  <el-tooltip
                    class="item"
                    effect="dark"
                    :content="$t('preferences.sync-tracker-tips')"
                    placement="bottom"
                  >
                    <el-button
                      @click="syncTrackerFromSource"
                      class="sync-tracker-btn"
                    >
                      <mo-icon
                        name="refresh"
                        width="12"
                        height="12"
                        :spin="true"
                        v-if="trackerSyncing"
                      />
                      <mo-icon name="sync" width="12" height="12" v-else />
                    </el-button>
                  </el-tooltip>
                </div>
              </el-col>
            </el-row>
            <el-input
              type="textarea"
              rows="3"
              auto-complete="off"
              :placeholder="`${$t('preferences.bt-tracker-input-tips')}`"
              v-model="form.btTracker"
            >
            </el-input>
            <div class="el-form-item__info" style="margin-top: 8px">
              {{ $t('preferences.bt-tracker-tips') }}
              <a
                target="_blank"
                href="https://github.com/ngosang/trackerslist"
                rel="noopener noreferrer"
              >
                ngosang/trackerslist
                <mo-icon name="link" width="12" height="12" />
              </a>
              <a
                target="_blank"
                href="https://github.com/XIU2/TrackersListCollection"
                rel="noopener noreferrer"
              >
                XIU2/TrackersListCollection
                <mo-icon name="link" width="12" height="12" />
              </a>
            </div>
          </div>
          <div class="form-item-sub">
            <el-checkbox v-model="form.autoSyncTracker">
              {{ $t('preferences.auto-sync-tracker') }}
            </el-checkbox>
            <div
              class="el-form-item__info"
              style="margin-top: 8px"
              v-if="form.lastSyncTrackerTime > 0"
            >
              {{ new Date(form.lastSyncTrackerTime).toLocaleString() }}
            </div>
          </div>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.rpc')}: `"
          :label-width="formLabelWidth"
        >
          <el-row style="margin-bottom: 8px">
            <el-col class="form-item-sub" :xs="24" :sm="18" :md="10" :lg="10">
              {{ $t('preferences.rpc-listen-port') }}
              <el-input
                :placeholder="rpcDefaultPort"
                :maxlength="8"
                v-model="form.rpcListenPort"
                @change="onRpcListenPortChange"
              >
                <i slot="append" @click.prevent="onRpcPortDiceClick">
                  <mo-icon name="dice" width="12" height="12" />
                </i>
              </el-input>
            </el-col>
          </el-row>
          <el-row style="margin-bottom: 8px">
            <el-col class="form-item-sub" :xs="24" :sm="18" :md="18" :lg="18">
              {{ $t('preferences.rpc-secret') }}
              <el-input
                :show-password="hideRpcSecret"
                placeholder="RPC Secret"
                :maxlength="64"
                v-model="form.rpcSecret"
              >
                <i slot="append" @click.prevent="onRpcSecretDiceClick">
                  <mo-icon name="dice" width="12" height="12" />
                </i>
              </el-input>
              <div class="el-form-item__info" style="margin-top: 8px">
                <a
                  target="_blank"
                  href="https://github.com/agalwood/Motr/wiki/RPC"
                  rel="noopener noreferrer"
                >
                  {{ $t('preferences.rpc-secret-tips') }}
                  <mo-icon name="link" width="12" height="12" />
                </a>
              </div>
            </el-col>
          </el-row>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.port')}: `"
          :label-width="formLabelWidth"
        >
          <el-row style="margin-bottom: 8px">
            <el-col class="form-item-sub" :xs="24" :sm="18" :md="12" :lg="12">
              <el-switch v-model="form.enableUpnp" active-text="UPnP/NAT-PMP">
              </el-switch>
            </el-col>
          </el-row>
          <el-row style="margin-bottom: 8px">
            <el-col class="form-item-sub" :xs="24" :sm="18" :md="10" :lg="10">
              {{ $t('preferences.bt-port') }}
              <el-input
                placeholder="BT Port"
                :maxlength="8"
                v-model="form.listenPort"
                @change="onBtPortChange"
              >
                <i slot="append" @click.prevent="onBtPortDiceClick">
                  <mo-icon name="dice" width="12" height="12" />
                </i>
              </el-input>
            </el-col>
          </el-row>
          <el-row>
            <el-col class="form-item-sub" :xs="24" :sm="18" :md="10" :lg="10">
              {{ $t('preferences.dht-port') }}
              <el-input
                placeholder="DHT Port"
                :maxlength="8"
                v-model="form.dhtListenPort"
                @change="onDhtPortChange"
              >
                <i slot="append" @click.prevent="onDhtPortDiceClick">
                  <mo-icon name="dice" width="12" height="12" />
                </i>
              </el-input>
            </el-col>
          </el-row>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.download-protocol')}: `"
          :label-width="formLabelWidth"
        >
          {{ $t('preferences.protocols-default-client') }}
          <el-col class="form-item-sub" :span="24">
            <el-switch
              v-model="form.protocols.magnet"
              :active-text="$t('preferences.protocols-magnet')"
              @change="(val) => onProtocolsChange('magnet', val)"
            >
            </el-switch>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-switch
              v-model="form.protocols.thunder"
              :active-text="$t('preferences.protocols-thunder')"
              @change="(val) => onProtocolsChange('thunder', val)"
            >
            </el-switch>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.user-agent')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.mock-user-agent') }}
            <el-input
              type="textarea"
              rows="2"
              auto-complete="off"
              placeholder="User-Agent"
              v-model="form.userAgent"
            >
            </el-input>
            <el-button-group class="ua-group">
              <el-button @click="() => changeUA('aria2')">Aria2</el-button>
              <el-button @click="() => changeUA('transmission')"
                >Transmission</el-button
              >
              <el-button @click="() => changeUA('chrome')">Chrome</el-button>
              <el-button @click="() => changeUA('du')">du</el-button>
            </el-button-group>
          </el-col>
        </el-form-item>
        <el-form-item
          :label="`${$t('preferences.developer')}: `"
          :label-width="formLabelWidth"
        >
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.aria2-conf-path') }}
            <el-input placeholder="" disabled v-model="aria2ConfPath">
              <mo-show-in-folder
                slot="append"
                v-if="isRenderer"
                :path="aria2ConfPath"
              />
            </el-input>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.download-session-path') }}
            <el-input placeholder="" disabled v-model="sessionPath">
              <mo-show-in-folder
                slot="append"
                v-if="isRenderer"
                :path="sessionPath"
              />
            </el-input>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            {{ $t('preferences.app-log-path') }}
            <el-row :gutter="16">
              <el-col :span="18">
                <el-input placeholder="" disabled v-model="logPath">
                  <mo-show-in-folder
                    slot="append"
                    v-if="isRenderer"
                    :path="logPath"
                  />
                </el-input>
              </el-col>
              <el-col :span="6">
                <el-select v-model="form.logLevel">
                  <el-option
                    v-for="item in logLevels"
                    :key="item"
                    :label="item"
                    :value="item"
                  >
                  </el-option>
                </el-select>
              </el-col>
            </el-row>
          </el-col>
          <el-col class="form-item-sub" :span="24">
            <el-button
              plain
              type="warning"
              @click="() => onSessionResetClick()"
            >
              {{ $t('preferences.session-reset') }}
            </el-button>
            <el-button plain type="danger" @click="() => onFactoryResetClick()">
              {{ $t('preferences.factory-reset') }}
            </el-button>
          </el-col>
        </el-form-item>
      </el-form>
      <div class="form-actions">
        <el-button type="primary" @click="submitForm('advancedForm')">
          {{ $t('preferences.save') }}
        </el-button>
        <!-- <el-button @click="resetForm('advancedForm')">
          {{ $t('preferences.discard') }}
        </el-button> -->
      </div>
    </el-main>
  </el-container>
</template>

<script>
import is from '@/utils/is';
import { confirm } from '@/utils/dialog';
import { mapState } from 'vuex';
import { cloneDeep, extend, isEmpty } from 'lodash';
import randomize from 'randomatic';
import ShowInFolder from '@/components/Native/ShowInFolder.vue';
import SubnavSwitcher from '@/components/Subnav/SubnavSwitcher.vue';
import userAgentMap from '@shared/ua';
import {
  ENGINE_BT_PORT,
  ENGINE_DHT_PORT,
  ENGINE_RPC_PORT,
  LOG_LEVELS,
  TRACKER_SOURCE_OPTIONS,
} from '@shared/constants';
import {
  buildRpcUrl,
  calcFormLabelWidth,
  changedConfig,
  checkIsNeedRestart,
  convertCommaToLine,
  convertLineToComma,
  diffConfig,
  generateRandomInt,
  normalizePort,
} from '@shared/utils';
import {
  convertTrackerDataToLine,
  reduceTrackerString,
} from '@shared/utils/tracker';
import '@/components/Icons/dice';
import '@/components/Icons/sync';
import '@/components/Icons/refresh';
import { getLanguage } from '@shared/locales';
import { getLocaleManager } from '@/components/Locale';
import { restartApp } from '@/utils/native';

const initForm = (config) => {
  const {
    allProxy,
    allProxyBackup,
    autoCheckUpdate,
    autoSyncTracker,
    btTracker,
    dhtListenPort,
    enableUpnp,
    hideAppMenu,
    lastCheckUpdateTime,
    lastSyncTrackerTime,
    listenPort,
    logLevel,
    noProxy,
    protocols,
    rpcListenPort,
    rpcSecret,
    trackerSource,
    useProxy,
    userAgent,
  } = config;
  const result = {
    // 全局代理：all-proxy / no-proxy 在 system.json，use-proxy /
    // all-proxy-backup 在 user.json（与原版一致）
    allProxy,
    allProxyBackup,
    autoCheckUpdate,
    autoSyncTracker,
    btTracker: convertCommaToLine(btTracker),
    dhtListenPort,
    enableUpnp,
    hideAppMenu,
    lastCheckUpdateTime,
    lastSyncTrackerTime,
    listenPort,
    logLevel,
    noProxy: convertCommaToLine(noProxy),
    protocols: { ...protocols },
    rpcListenPort,
    rpcSecret,
    trackerSource,
    useProxy,
    userAgent,
  };
  return result;
};

export default {
  name: 'mo-preference-advanced',
  components: {
    [SubnavSwitcher.name]: SubnavSwitcher,
    [ShowInFolder.name]: ShowInFolder,
  },
  data() {
    const { locale } = this.$store.state.preference.config;
    const formOriginal = initForm(this.$store.state.preference.config);
    let form = {};
    form = initForm(extend(form, formOriginal));

    return {
      form,
      formLabelWidth: calcFormLabelWidth(locale),
      formOriginal,
      hideRpcSecret: true,
      rules: {},
      trackerSourceOptions: TRACKER_SOURCE_OPTIONS,
      trackerSyncing: false,
    };
  },
  computed: {
    isRenderer: () => is.isTauri(),
    title() {
      return this.$t('preferences.advanced');
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
    rpcDefaultPort() {
      return ENGINE_RPC_PORT;
    },
    logLevels() {
      return LOG_LEVELS;
    },
    ...mapState('preference', {
      config: (state) => state.config,
      aria2ConfPath: (state) => state.config.aria2ConfPath,
      logPath: (state) => state.config.logPath,
      sessionPath: (state) => state.config.sessionPath,
    }),
  },
  watch: {
    'form.rpcListenPort'(val) {
      const url = buildRpcUrl({
        port: this.form.rpcListenPort,
        secret: val,
      });
      navigator.clipboard.writeText(url);
    },
    'form.rpcSecret'(val) {
      const url = buildRpcUrl({
        port: this.form.rpcListenPort,
        secret: val,
      });
      navigator.clipboard.writeText(url);
    },
  },
  methods: {
    handleLocaleChange(locale) {
      const lng = getLanguage(locale);
      getLocaleManager().changeLanguage(lng);
    },
    async onCheckUpdateClick() {
      this.$msg.info(this.$t('app.checking-for-updates'));

      await window.application?.updateManager?.checkForUpdates({
        userCheck: true,
      });

      // 检查流程会写 last-check-update-time，回来后再读一次填到表单
      const config = await this.$store.dispatch('preference/fetchPreference');
      this.form.lastCheckUpdateTime = config.lastCheckUpdateTime;
    },
    syncTrackerFromSource() {
      this.trackerSyncing = true;
      const { trackerSource } = this.form;
      this.$store
        .dispatch('preference/fetchBtTracker', trackerSource)
        .then((data) => {
          const tracker = convertTrackerDataToLine(data);
          this.form.lastSyncTrackerTime = Date.now();
          this.form.btTracker = tracker;
          this.trackerSyncing = false;
        })
        .catch(() => {
          this.trackerSyncing = false;
        });
    },
    onProtocolsChange(protocol, enabled) {
      const { protocols } = this.form;
      this.form.protocols = {
        ...protocols,
        [protocol]: enabled,
      };
    },
    // 开关只改 system.json 里的 all-proxy：关掉时清空，备份留在 user.json
    onUseProxyChange(flag) {
      this.form.allProxy = flag ? this.form.allProxyBackup : '';
    },
    // 编辑代理地址时同步更新真正生效的 all-proxy
    onAllProxyBackupChange(value) {
      this.form.allProxy = value;
    },
    changeUA(type) {
      const ua = userAgentMap[type];
      if (!ua) {
        return;
      }
      this.form.userAgent = ua;
    },
    onBtPortDiceClick() {
      const port = generateRandomInt(20000, 24999);
      this.form.listenPort = port;
    },
    onDhtPortDiceClick() {
      const port = generateRandomInt(25000, 29999);
      this.form.dhtListenPort = port;
    },
    onBtPortChange(value) {
      this.form.listenPort = normalizePort(
        value,
        this.formOriginal.listenPort || ENGINE_BT_PORT
      );
    },
    onDhtPortChange(value) {
      this.form.dhtListenPort = normalizePort(
        value,
        this.formOriginal.dhtListenPort || ENGINE_DHT_PORT
      );
    },
    onRpcListenPortChange(value) {
      this.form.rpcListenPort = normalizePort(
        value,
        this.formOriginal.rpcListenPort || this.rpcDefaultPort
      );
    },
    onRpcPortDiceClick() {
      const port = generateRandomInt(ENGINE_RPC_PORT, 20000);
      this.form.rpcListenPort = port;
    },
    onRpcSecretDiceClick() {
      this.hideRpcSecret = false;
      const rpcSecret = randomize('Aa0', 16);
      this.form.rpcSecret = rpcSecret;

      setTimeout(() => {
        this.hideRpcSecret = true;
      }, 2000);
    },
    onSessionResetClick() {
      confirm(this.$t('preferences.session-reset-confirm'), {
        title: this.$t('preferences.session-reset'),
        kind: 'warning',
        okLabel: this.$t('app.yes'),
        cancelLabel: this.$t('app.no'),
      }).then((okClicked) => {
        if (okClicked) {
          this.$store.dispatch('task/purgeTaskRecord');
          // 用 finally 而不是 then：暂停失败也不能把重置吞掉 ——
          // 重置本身会把引擎连同任务一起清掉，暂停只是顺手做的
          this.$store.dispatch('task/pauseAllTask').finally(async () => {
            // 停引擎 → 删掉 download.session → 重新起引擎
            await window.application?.resetSession();
          });
        }
      });
    },
    onFactoryResetClick() {
      confirm(this.$t('preferences.factory-reset-confirm'), {
        title: this.$t('preferences.factory-reset'),
        kind: 'warning',
        okLabel: this.$t('app.yes'),
        cancelLabel: this.$t('app.no'),
      }).then((okClicked) => {
        if (okClicked) {
          // 先把 aria2 会话落盘（未完成的任务不该被清掉，只清配置），
          // 再让主进程清空 user.json / system.json 并重启
          this.$store.dispatch('task/saveSession').finally(async () => {
            await window.application?.reset();
          });
        }
      });
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
          ...changedConfig.basic,
        };

        const { btAutoDownloadContent, btTracker, noProxy } = data;

        if ('btAutoDownloadContent' in data) {
          data.followTorrent = btAutoDownloadContent;
          data.followMetalink = btAutoDownloadContent;
          data.pauseMetadata = !btAutoDownloadContent;
        }

        if (btTracker) {
          data.btTracker = reduceTrackerString(convertLineToComma(btTracker));
        }

        // 代理白名单：表单里一行一个，store/aria2 要逗号分隔
        if (noProxy) {
          data.noProxy = convertLineToComma(noProxy);
        }

        // 三个监听端口都过一遍：空值/越界/非数字会让 Rust 侧的 u16 反序列化失败，
        // 那种情况下整份 system.json 会回落默认值（见 @shared/utils 的 normalizePort）
        if ('rpcListenPort' in data) {
          data.rpcListenPort = normalizePort(
            data.rpcListenPort,
            this.formOriginal.rpcListenPort || this.rpcDefaultPort
          );
        }

        if ('listenPort' in data) {
          data.listenPort = normalizePort(
            data.listenPort,
            this.formOriginal.listenPort || ENGINE_BT_PORT
          );
        }

        if ('dhtListenPort' in data) {
          data.dhtListenPort = normalizePort(
            data.dhtListenPort,
            this.formOriginal.dhtListenPort || ENGINE_DHT_PORT
          );
        }

        console.log('[Motr] preference changed data:', data);

        this.$store
          .dispatch('preference/save', data)
          .then(() => {
            this.$store.dispatch('app/fetchEngineOptions');
            this.syncFormConfig();
            this.$msg.success(this.$t('preferences.save-success-message'));

            // 下载协议同理：Rust 侧把 Motr 注册成 magnet: / thunder: 的
            // 默认客户端，或取消注册（见 src-tauri/src/protocol.rs）。
            // 这个开关不在 needRestartKeys 里，所以要即时通知
            if ('protocols' in data) {
              window.application?.protocolManager?.apply(data.protocols);
            }
          })
          .catch(() => {
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
    changedConfig.advanced = diffConfig(this.formOriginal, this.form);
    if (to.path === '/preference/basic') {
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

<style lang="scss">
.bt-tracker {
  position: relative;
  .sync-tracker-btn {
    line-height: 0;
  }
  .track-source {
    margin-bottom: 16px;
    .select-track-source {
      width: 100%;
    }
    .el-select__tags {
      overflow-x: auto;
    }
  }
}
.ua-group {
  margin-top: 8px;
}
</style>
