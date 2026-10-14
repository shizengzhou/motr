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

    // 监听已就位，可以收命令了。通知 Rust 侧一声：冷启动时系统递过来的
    // 协议链接（magnet: 等）在 src-tauri/src/protocol.rs 里先攒着，这里补发
    invoke('protocol_ready').catch((err) => {
      console.warn('[Motr] protocol ready fail:', err);
    });
  },
  destroyed() {
    this.unbindIpcEvents();
  },
};
</script>
