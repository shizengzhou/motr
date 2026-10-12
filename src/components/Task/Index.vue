<template>
  <el-container class="main panel" direction="horizontal">
    <el-aside width="200px" class="subnav hidden-xs-only">
      <mo-task-subnav :current="status" />
    </el-aside>
    <el-container class="content panel" direction="vertical">
      <el-header class="panel-header" height="84">
        <h4 class="task-title hidden-xs-only">{{ title }}</h4>
        <mo-subnav-switcher
          :title="title"
          :subnavs="subnavs"
          class="hidden-sm-and-up"
        />
        <mo-task-actions />
      </el-header>
      <el-main class="panel-content">
        <mo-task-list />
      </el-main>
    </el-container>
  </el-container>
</template>

<script>
import { mapState } from 'vuex';

import { commands } from '@/components/CommandManager/instance';
import { ADD_TASK_TYPE } from '@shared/constants';
import TaskSubnav from '@/components/Subnav/TaskSubnav.vue';
import TaskActions from '@/components/Task/TaskActions.vue';
import TaskList from '@/components/Task/TaskList.vue';
import SubnavSwitcher from '@/components/Subnav/SubnavSwitcher.vue';
import { getTaskUri, parseHeader } from '@shared/utils';
import {
  delayDeleteTaskFiles,
  showItemInFolder,
  moveTaskFilesToTrash,
} from '@/utils/native';

// MessageBox 是单例（element-ui/packages/message-box/src/main.js 里的 let instance），
// 每次都把 message 塞进同一个实例的 $slots.default 渲染，所以同 key 的 VNode 会被就地复用、
// 子组件实例不重建。ConfirmCheckbox 要按每次弹窗的 defaultChecked 重新初始化，就必须换 key
let confirmCheckboxKey = 0;

// MessageBox 的 message 只接受 String / VNode，而传入的 VNode 会被挂到 MessageBox 自己的
// 实例上渲染，不会随本组件重渲染；而 el-checkbox 是受控组件（value 不变就不会切换外观），
// 所以用一个自带状态的包装组件承载它
const ConfirmCheckbox = {
  name: 'MoConfirmCheckbox',
  props: {
    label: {
      type: String,
      default: '',
    },
    defaultChecked: {
      type: Boolean,
      default: false,
    },
  },
  data() {
    return {
      checked: this.defaultChecked,
    };
  },
  render(h) {
    return h(
      'el-checkbox',
      {
        // MessageBox 挂在 body 上，组件的 scoped 样式对它无效，间距只能内联
        style: { marginTop: '12px' },
        props: { value: this.checked },
        on: {
          input: (val) => {
            this.checked = val;
            this.$emit('change', val);
          },
        },
      },
      [this.label]
    );
  },
};

export default {
  name: 'mo-content-task',
  components: {
    [TaskSubnav.name]: TaskSubnav,
    [TaskActions.name]: TaskActions,
    [TaskList.name]: TaskList,
    [SubnavSwitcher.name]: SubnavSwitcher,
  },
  props: {
    status: {
      type: String,
      default: 'active',
    },
  },
  computed: {
    ...mapState('task', {
      taskList: (state) => state.taskList,
      selectedGidList: (state) => state.selectedGidList,
      selectedGidListCount: (state) => state.selectedGidList.length,
    }),
    ...mapState('preference', {
      noConfirmBeforeDelete: (state) => state.config.noConfirmBeforeDeleteTask,
    }),
    subnavs() {
      return [
        {
          key: 'active',
          title: this.$t('task.active'),
          route: '/task/active',
        },
        {
          key: 'waiting',
          title: this.$t('task.waiting'),
          route: '/task/waiting',
        },
        {
          key: 'stopped',
          title: this.$t('task.stopped'),
          route: '/task/stopped',
        },
      ];
    },
    title() {
      const subnav = this.subnavs.find((item) => item.key === this.status);
      return subnav.title;
    },
  },
  watch: {
    status: 'onStatusChange',
  },
  methods: {
    onStatusChange() {
      this.changeCurrentList();
    },
    changeCurrentList() {
      this.$store.dispatch('task/changeCurrentList', this.status);
    },
    directAddTask(uri, options = {}) {
      const uris = [uri];
      const payload = {
        uris,
        options: {
          ...options,
        },
      };
      this.$store.dispatch('task/addUri', payload).catch((err) => {
        this.$msg.error(err.message);
      });
    },
    showAddTaskDialog(uri, options = {}) {
      const { header, ...rest } = options;
      console.log('[Motr] show add task dialog options: ', options);

      const headers = parseHeader(header);
      const newOptions = {
        ...rest,
        ...headers,
      };

      this.$store.dispatch('app/updateAddTaskUrl', uri);
      this.$store.dispatch('app/updateAddTaskOptions', newOptions);
      this.$store.dispatch('app/showAddTaskDialog', ADD_TASK_TYPE.URI);
    },
    async deleteTaskFiles(task) {
      try {
        const result = await moveTaskFilesToTrash(task);

        if (!result) {
          throw new Error('task.remove-task-file-fail');
        }
      } catch (err) {
        this.$msg.error(this.$t(err.message));
      }
    },
    removeTask(task, taskName, isRemoveWithFiles = false) {
      this.$store.dispatch('task/forcePauseTask', task).finally(async () => {
        if (isRemoveWithFiles) {
          await this.deleteTaskFiles(task);
        }

        return this.removeTaskItem(task, taskName);
      });
    },
    removeTaskRecord(task, taskName, isRemoveWithFiles = false) {
      this.$store.dispatch('task/forcePauseTask', task).finally(async () => {
        if (isRemoveWithFiles) {
          await this.deleteTaskFiles(task);
        }

        return this.removeTaskRecordItem(task, taskName);
      });
    },
    async removeTaskItem(task, taskName) {
      try {
        await this.$store.dispatch('task/removeTask', task);
        this.$msg.success(
          this.$t('task.delete-task-success', {
            taskName,
          })
        );
      } catch ({ code }) {
        if (code === 1) {
          this.$msg.error(
            this.$t('task.delete-task-fail', {
              taskName,
            })
          );
        }
      }
    },
    async removeTaskRecordItem(task, taskName) {
      try {
        await this.$store.dispatch('task/removeTaskRecord', task);
        this.$msg.success(
          this.$t('task.remove-record-success', {
            taskName,
          })
        );
      } catch ({ code }) {
        if (code === 1) {
          this.$msg.error(
            this.$t('task.remove-record-fail', {
              taskName,
            })
          );
        }
      }
    },
    removeTasks(taskList, isRemoveWithFiles = false) {
      const gids = taskList.map((task) => task.gid);
      this.$store.dispatch('task/batchForcePauseTask', gids).finally(() => {
        if (isRemoveWithFiles) {
          this.batchDeleteTaskFiles(taskList);
        }

        this.removeTaskItems(gids);
      });
    },
    batchDeleteTaskFiles(taskList) {
      const promises = taskList.map((task, index) =>
        delayDeleteTaskFiles(task, index * 200)
      );
      Promise.allSettled(promises).then((results) => {
        console.log('[Motr] batch delete task files: ', results);
      });
    },
    removeTaskItems(gids) {
      this.$store
        .dispatch('task/batchRemoveTask', gids)
        .then(() => {
          this.$msg.success(this.$t('task.batch-delete-task-success'));
        })
        .catch(({ code }) => {
          if (code === 1) {
            this.$msg.error(this.$t('task.batch-delete-task-fail'));
          }
        });
    },
    handlePauseTask(payload) {
      const { task, taskName } = payload;
      this.$msg.info(this.$t('task.download-pause-message', { taskName }));
      this.$store.dispatch('task/pauseTask', task).catch(({ code }) => {
        if (code === 1) {
          this.$msg.error(this.$t('task.pause-task-fail', { taskName }));
        }
      });
    },
    handleResumeTask(payload) {
      const { task, taskName } = payload;
      this.$store.dispatch('task/resumeTask', task).catch(({ code }) => {
        if (code === 1) {
          this.$msg.error(
            this.$t('task.resume-task-fail', {
              taskName,
            })
          );
        }
      });
    },
    handleStopTaskSeeding(payload) {
      const { task } = payload;
      this.$store.dispatch('task/stopSeeding', task);
      this.$msg.info({
        message: this.$t('task.bt-stopping-seeding-tip'),
        duration: 8000,
      });
    },
    handleRestartTask(payload) {
      const { task, taskName, showDialog } = payload;
      const { gid } = task;
      const uri = getTaskUri(task);

      this.$store.dispatch('task/getTaskOption', gid).then((data) => {
        console.log('[Motr] get task option:', data);
        const { dir, header, split } = data;
        const options = {
          dir,
          header,
          split,
          out: taskName,
        };

        if (showDialog) {
          this.showAddTaskDialog(uri, options);
        } else {
          this.directAddTask(uri, options);
          this.$store.dispatch('task/removeTaskRecord', task);
        }
      });
    },
    handleRevealInFolder(payload) {
      const { path } = payload;
      showItemInFolder(path, {
        errorMsg: this.$t('task.file-not-exist'),
      });
    },
    // 删除确认框，等价于原 dialog.showMessageBox；Element 的 MessageBox 不支持 checkbox，
    // 所以把 ConfirmCheckbox 作为 message 的一部分，返回 { confirmed, deleteWithFiles }
    confirmDeleteTask({ title, message, checkboxLabel, deleteWithFiles }) {
      const h = this.$createElement;
      const state = { deleteWithFiles: !!deleteWithFiles };

      const content = h('div', [
        h('p', message),
        h(ConfirmCheckbox, {
          // 换 key 强制重建实例，让勾选状态按本次的 defaultChecked 初始化
          key: (confirmCheckboxKey += 1),
          props: {
            label: checkboxLabel,
            defaultChecked: state.deleteWithFiles,
          },
          on: {
            change: (checked) => {
              state.deleteWithFiles = checked;
            },
          },
        }),
      ]);

      return this.$confirm(content, title, {
        type: 'warning',
        confirmButtonText: this.$t('app.yes'),
        cancelButtonText: this.$t('app.no'),
        // 与原 Electron 对话框一致：没有右上角关闭按钮，点遮罩不关闭（Esc 等同取消）
        showClose: false,
        closeOnClickModal: false,
      }).then(
        () => ({ confirmed: true, deleteWithFiles: state.deleteWithFiles }),
        () => ({ confirmed: false, deleteWithFiles: false })
      );
    },
    async handleDeleteTask(payload) {
      const { task, taskName, deleteWithFiles } = payload;

      if (this.noConfirmBeforeDelete) {
        this.removeTask(task, taskName, deleteWithFiles);
        return;
      }

      const { confirmed, deleteWithFiles: checked } =
        await this.confirmDeleteTask({
          title: this.$t('task.delete-task'),
          message: this.$t('task.delete-task-confirm', { taskName }),
          checkboxLabel: this.$t('task.delete-task-label'),
          deleteWithFiles,
        });

      if (confirmed) {
        this.removeTask(task, taskName, checked);
      }
    },
    async handleDeleteTaskRecord(payload) {
      const { task, taskName, deleteWithFiles } = payload;

      if (this.noConfirmBeforeDelete) {
        this.removeTaskRecord(task, taskName, deleteWithFiles);
        return;
      }

      const { confirmed, deleteWithFiles: checked } =
        await this.confirmDeleteTask({
          title: this.$t('task.remove-record'),
          message: this.$t('task.remove-record-confirm', { taskName }),
          checkboxLabel: this.$t('task.remove-record-label'),
          deleteWithFiles,
        });

      if (confirmed) {
        this.removeTaskRecord(task, taskName, checked);
      }
    },
    async handleBatchDeleteTask(payload) {
      const { deleteWithFiles } = payload;
      const {
        noConfirmBeforeDelete,
        selectedGidList,
        selectedGidListCount,
        taskList,
      } = this;
      if (selectedGidListCount === 0) {
        return;
      }

      const selectedTaskList = taskList.filter((task) => {
        return selectedGidList.includes(task.gid);
      });

      if (noConfirmBeforeDelete) {
        this.removeTasks(selectedTaskList, deleteWithFiles);
        return;
      }

      const count = `${selectedGidListCount}`;
      const { confirmed, deleteWithFiles: checked } =
        await this.confirmDeleteTask({
          title: this.$t('task.delete-selected-tasks'),
          message: this.$t('task.batch-delete-task-confirm', { count }),
          checkboxLabel: this.$t('task.delete-task-label'),
          deleteWithFiles,
        });

      if (confirmed) {
        this.removeTasks(selectedTaskList, checked);
      }
    },
    handleCopyTaskLink(payload) {
      const { task } = payload;
      const uri = getTaskUri(task);
      navigator.clipboard.writeText(uri).then(() => {
        this.$msg.success(this.$t('task.copy-link-success'));
      });
    },
    handleShowTaskInfo(payload) {
      const { task } = payload;
      this.$store.dispatch('task/showTaskDetail', task);
    },
  },
  created() {
    this.changeCurrentList();
  },
  mounted() {
    commands.on('pause-task', this.handlePauseTask);
    commands.on('resume-task', this.handleResumeTask);
    commands.on('stop-task-seeding', this.handleStopTaskSeeding);
    commands.on('restart-task', this.handleRestartTask);
    commands.on('reveal-in-folder', this.handleRevealInFolder);
    commands.on('delete-task', this.handleDeleteTask);
    commands.on('delete-task-record', this.handleDeleteTaskRecord);
    commands.on('batch-delete-task', this.handleBatchDeleteTask);
    commands.on('copy-task-link', this.handleCopyTaskLink);
    commands.on('show-task-info', this.handleShowTaskInfo);
  },
  destroyed() {
    commands.off('pause-task', this.handlePauseTask);
    commands.off('resume-task', this.handleResumeTask);
    commands.off('stop-task-seeding', this.handleStopTaskSeeding);
    commands.off('restart-task', this.handleRestartTask);
    commands.off('reveal-in-folder', this.handleRevealInFolder);
    commands.off('delete-task', this.handleDeleteTask);
    commands.off('delete-task-record', this.handleDeleteTaskRecord);
    commands.off('batch-delete-task', this.handleBatchDeleteTask);
    commands.off('copy-task-link', this.handleCopyTaskLink);
    commands.off('show-task-info', this.handleShowTaskInfo);
  },
};
</script>
