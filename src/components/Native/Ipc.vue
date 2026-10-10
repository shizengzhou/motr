<template>
  <div v-if="false"></div>
</template>

<script>
import { listen } from '@tauri-apps/api/event';
import { commands } from '@/components/CommandManager/instance';

export default {
  name: 'mo-ipc',
  methods: {
    async bindIpcEvents() {
      // this.$electron.ipcRenderer.on('command', (event, command, ...args) => {
      //   commands.execute(command, ...args);
      // });
      this.unlisten = await listen('command', (event) => {
        console.log('ipc', event);
        const { command, args } = event.payload;
        commands.execute(command, ...args);
      });
    },
    unbindIpcEvents() {
      if (this.unlisten) {
        this.unlisten();
        this.unlisten = null;
      }
    },
  },
  async created() {
    await this.bindIpcEvents();
  },
  destroyed() {
    this.unbindIpcEvents();
  },
};
</script>
