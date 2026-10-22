<template>
  <div v-if="false"></div>
</template>

<script>
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { commands } from '@/components/CommandManager/instance';

export default {
  name: 'mo-ipc',
  methods: {
    async bindIpcEvents() {
      this.unlisten = await listen('command', (event) => {
        console.log('ipc', event);
        const { command, args } = event.payload;
        if (Array.isArray(args)) {
          commands.execute(command, ...args);
        } else {
          commands.execute(command);
        }
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

    // 监听已就位：前端 ProtocolManager 补发冷启动攒下的 deep link 命令；
    // Rust 侧只负责 .torrent 文件关联，也保留自己的队列
    window.application?.protocolManager?.onReady();

    invoke('protocol_ready').catch((err) => {
      console.warn('[Motr] protocol ready fail:', err);
    });
  },
  destroyed() {
    this.unbindIpcEvents();
  },
};
</script>
