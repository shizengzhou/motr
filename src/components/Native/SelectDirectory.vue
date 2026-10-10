<template>
  <el-button class="select-directory" @click.stop="onFolderClick">
    <mo-icon name="folder" width="10" height="10" />
  </el-button>
</template>

<script>
import { open } from '@tauri-apps/plugin-dialog';
import '@/components/Icons/folder';

export default {
  name: 'mo-select-directory',
  props: {},
  methods: {
    async onFolderClick() {
      const path = await open({
        directory: true,
        multiple: false,
        canCreateDirectories: true,
      });

      if (!path) {
        return;
      }

      this.$emit('selected', path);
    },
  },
};
</script>
