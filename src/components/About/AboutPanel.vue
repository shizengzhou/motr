<template>
  <el-dialog
    custom-class="app-about-dialog"
    width="61.8vw"
    :visible="visible"
    @open="handleOpen"
    :before-close="handleClose"
    @closed="handleClosed"
  >
    <mo-app-info :version="version" :engine="engineInfo" />
    <mo-copyright slot="footer" />
  </el-dialog>
</template>

<script>
import { mapState } from 'vuex';
import AppInfo from '@/components/About/AppInfo.vue';
import Copyright from '@/components/About/Copyright.vue';
import { getVersion } from '@tauri-apps/api/app';

export default {
  name: 'mo-about-panel',
  components: {
    [AppInfo.name]: AppInfo,
    [Copyright.name]: Copyright,
  },
  props: {
    visible: {
      type: Boolean,
      default: false,
    },
  },
  data() {
    return {
      version: '',
    };
  },
  computed: {
    ...mapState('app', {
      engineInfo: (state) => state.engineInfo,
    }),
  },
  created() {
    getVersion().then((version) => {
      this.version = version;
    });
  },
  methods: {
    handleOpen() {
      this.$store.dispatch('app/fetchEngineInfo');
    },
    handleClose() {
      this.$store.dispatch('app/hideAboutPanel');
    },
    handleClosed() {},
  },
};
</script>

<style lang="scss">
.app-about-dialog {
  max-width: 632px;
  min-width: 380px;
  .el-dialog__header {
    padding-top: 0;
    padding-bottom: 0;
  }
}
</style>
