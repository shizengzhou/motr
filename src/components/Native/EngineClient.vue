<template>
  <div v-if="false"></div>
</template>

<script>
import { mapState } from 'vuex';
import { getCurrentWindow, ProgressBarStatus } from '@tauri-apps/api/window';
import is from '@/utils/is';
import api from '@/api';
import { showNotification } from '@/utils/native';
import { bytesToSize, checkTaskIsBT, getTaskName } from '@shared/utils';

// app store 里的 progress 直接喂给窗口 API，数值语义与原 Electron 版一致：
// -1（没有活跃任务）→ 清除；>1（原版的 2，进度未知）→ Windows 不确定态、其它平台 0；0~1 → 汇总进度百分比
const progressBarState = (progress) => {
  if (progress < 0) {
    return { status: ProgressBarStatus.None };
  }

  if (progress > 1) {
    return {
      progress: 0,
      status: is.windows()
        ? ProgressBarStatus.Indeterminate
        : ProgressBarStatus.Normal,
    };
  }

  return {
    progress: Math.round(progress * 100),
    status: ProgressBarStatus.Normal,
  };
};

export default {
  name: 'mo-engine-client',
  computed: {
    isRenderer: () => is.isTauri(),
    ...mapState('app', {
      uploadSpeed: (state) => state.stat.uploadSpeed,
      downloadSpeed: (state) => state.stat.downloadSpeed,
      speed: (state) => state.stat.uploadSpeed + state.stat.downloadSpeed,
      interval: (state) => state.interval,
      downloading: (state) => state.stat.numActive > 0,
      progress: (state) => state.progress,
    }),
    ...mapState('task', {
      messages: (state) => state.messages,
      seedingList: (state) => state.seedingList,
      taskDetailVisible: (state) => state.taskDetailVisible,
      enabledFetchPeers: (state) => state.enabledFetchPeers,
      currentTaskGid: (state) => state.currentTaskGid,
      currentTaskItem: (state) => state.currentTaskItem,
    }),
    ...mapState('preference', {
      taskNotification: (state) => state.config.taskNotification,
      showProgressBar: (state) => state.config.showProgressBar,
      resumeAllWhenAppLaunched: (state) =>
        state.config.resumeAllWhenAppLaunched,
    }),
    currentTaskIsBT() {
      return checkTaskIsBT(this.currentTaskItem);
    },
  },
  watch: {
    downloading(val, oldVal) {
      // 原版发 'download-status-change' 事件给主进程（TrayManager 切换
      // normal/active 图标）。托盘现在由前端管理，直接调 TrayManager。
      if (val !== oldVal && this.isRenderer) {
        window.application?.trayManager?.setActive(val).catch((err) => {
          console.warn('[Motr] update tray active icon fail:', err);
        });

        // 有下载时阻止系统休眠（原版在同一事件里调 EnergyManager）：
        // 只阻止空闲休眠，不阻止熄屏，下载全部结束后立即释放
        const energyManager = window.application?.energyManager;
        if (val) {
          energyManager?.startPowerSaveBlocker();
        } else {
          energyManager?.stopPowerSaveBlocker();
        }
      }
    },
    speed() {
      // Dock 上显示下载速率（对齐原 Electron 版 DockManager.handleSpeedChange：
      // 只有 macOS 显示，速率为 0 时清空 badge）
      if (!is.macOS()) {
        return;
      }

      const { downloadSpeed } = this;
      const label =
        downloadSpeed > 0 ? `${bytesToSize(downloadSpeed)}/s` : undefined;

      getCurrentWindow()
        .setBadgeLabel(label)
        .catch((err) => {
          console.warn('[Motr] update dock speed fail:', err);
        });
    },
    progress(val) {
      // 进度条：原版由主进程按 show-progress-bar 决定是否响应，这里在唯一的出口判断
      if (!this.showProgressBar) {
        return;
      }

      getCurrentWindow()
        .setProgressBar(progressBarState(val))
        .catch((err) => {
          console.warn('[Motr] set task progress fail:', err);
        });
    },
    showProgressBar(val) {
      // 关掉开关时立即清除（对应原版 unbindProgressChange 里的 setProgressBar(-1)）
      if (val) {
        return;
      }

      getCurrentWindow()
        .setProgressBar({ status: ProgressBarStatus.None })
        .catch((err) => {
          console.warn('[Motr] clear task progress fail:', err);
        });
    },
  },
  methods: {
    async fetchTaskItem({ gid }) {
      return api.fetchTaskItem({ gid }).catch((e) => {
        console.warn(`fetchTaskItem fail: ${e.message}`);
      });
    },
    onDownloadStart(event) {
      this.$store.dispatch('task/fetchList');
      this.$store.dispatch('app/resetInterval');
      this.$store.dispatch('task/saveSession');
      const [{ gid }] = event;
      const { seedingList } = this;
      if (seedingList.includes(gid)) {
        return;
      }

      this.fetchTaskItem({ gid }).then((task) => {
        const { dir } = task;
        this.$store.dispatch('preference/recordHistoryDirectory', dir);
        const taskName = getTaskName(task);
        const message = this.$t('task.download-start-message', { taskName });
        this.$msg.info(message);
      });
    },
    onDownloadPause(event) {
      const [{ gid }] = event;
      const { seedingList } = this;
      if (seedingList.includes(gid)) {
        return;
      }

      this.fetchTaskItem({ gid }).then((task) => {
        const taskName = getTaskName(task);
        const message = this.$t('task.download-pause-message', { taskName });
        this.$msg.info(message);
      });
    },
    onDownloadStop(event) {
      const [{ gid }] = event;
      this.fetchTaskItem({ gid }).then((task) => {
        const taskName = getTaskName(task);
        const message = this.$t('task.download-stop-message', { taskName });
        this.$msg.info(message);
      });
    },
    onDownloadError(event) {
      const [{ gid }] = event;
      this.fetchTaskItem({ gid }).then((task) => {
        const taskName = getTaskName(task);
        const { errorCode, errorMessage } = task;
        console.error(
          `[Motr] download error gid: ${gid}, #${errorCode}, ${errorMessage}`
        );
        const message = this.$t('task.download-error-message', { taskName });
        const link = `<a target="_blank" href="https://github.com/agalwood/Motr/wiki/Error#${errorCode}" rel="noopener noreferrer">${errorCode}</a>`;
        this.$msg({
          type: 'error',
          showClose: true,
          duration: 5000,
          dangerouslyUseHTMLString: true,
          message: `${message} ${link}`,
        });
      });
    },
    onDownloadComplete(event) {
      this.$store.dispatch('task/fetchList');
      const [{ gid }] = event;
      this.$store.dispatch('task/removeFromSeedingList', gid);

      this.fetchTaskItem({ gid }).then((task) => {
        this.handleDownloadComplete(task, false);
      });
    },
    onBtDownloadComplete(event) {
      this.$store.dispatch('task/fetchList');
      const [{ gid }] = event;
      const { seedingList } = this;
      if (seedingList.includes(gid)) {
        return;
      }

      this.$store.dispatch('task/addToSeedingList', gid);

      this.fetchTaskItem({ gid }).then((task) => {
        this.handleDownloadComplete(task, true);
      });
    },
    handleDownloadComplete(task, isBT) {
      this.$store.dispatch('task/saveSession');

      this.showTaskCompleteNotify(task, isBT);

      // 原 Electron 版这里会把 task-download-complete 发给主进程（做 addRecentDocument
      // 和 Dock 动画），这两件事暂时都没做
    },
    showTaskCompleteNotify(task, isBT) {
      const taskName = getTaskName(task);
      const message = isBT
        ? this.$t('task.bt-download-complete-message', { taskName })
        : this.$t('task.download-complete-message', { taskName });
      const tips = isBT ? '\n' + this.$t('task.bt-download-complete-tips') : '';

      this.$msg.success(`${message}${tips}`);

      if (!this.taskNotification) {
        return;
      }

      const notifyMessage = isBT
        ? this.$t('task.bt-download-complete-notify')
        : this.$t('task.download-complete-notify');

      showNotification({
        title: notifyMessage,
        body: `${taskName}${tips}`,
      });
    },
    showTaskErrorNotify(task) {
      const taskName = getTaskName(task);

      const message = this.$t('task.download-fail-message', { taskName });
      this.$msg.success(message);

      if (!this.taskNotification) {
        return;
      }

      showNotification({
        title: this.$t('task.download-fail-notify'),
        body: taskName,
      });
    },
    bindEngineEvents() {
      api.client.on('onDownloadStart', this.onDownloadStart);
      // api.client.on('onDownloadPause', this.onDownloadPause)
      api.client.on('onDownloadStop', this.onDownloadStop);
      api.client.on('onDownloadComplete', this.onDownloadComplete);
      api.client.on('onDownloadError', this.onDownloadError);
      api.client.on('onBtDownloadComplete', this.onBtDownloadComplete);
    },
    unbindEngineEvents() {
      api.client.removeListener('onDownloadStart', this.onDownloadStart);
      // api.client.removeListener('onDownloadPause', this.onDownloadPause)
      api.client.removeListener('onDownloadStop', this.onDownloadStop);
      api.client.removeListener('onDownloadComplete', this.onDownloadComplete);
      api.client.removeListener('onDownloadError', this.onDownloadError);
      api.client.removeListener(
        'onBtDownloadComplete',
        this.onBtDownloadComplete
      );
    },
    startPolling() {
      this.timer = setTimeout(() => {
        this.polling();
        this.startPolling();
      }, this.interval);
    },
    polling() {
      this.$store.dispatch('app/fetchGlobalStat');
      this.$store.dispatch('app/fetchProgress');
      this.$store.dispatch('task/fetchList');

      if (this.taskDetailVisible && this.currentTaskGid) {
        if (this.currentTaskIsBT && this.enabledFetchPeers) {
          this.$store.dispatch('task/fetchItemWithPeers', this.currentTaskGid);
        } else {
          this.$store.dispatch('task/fetchItem', this.currentTaskGid);
        }
      }
    },
    stopPolling() {
      clearTimeout(this.timer);
      this.timer = null;
    },
  },
  created() {
    this.bindEngineEvents();
  },
  mounted() {
    setTimeout(() => {
      this.$store.dispatch('app/fetchEngineInfo');
      this.$store.dispatch('app/fetchEngineOptions');

      this.startPolling();
    }, 100);
  },
  destroyed() {
    this.$store.dispatch('task/saveSession');

    this.unbindEngineEvents();

    this.stopPolling();
  },
};
</script>
