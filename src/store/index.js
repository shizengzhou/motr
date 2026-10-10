import Vue from 'vue';
import Vuex from 'vuex';

import modules from './modules';

Vue.use(Vuex);

export default new Vuex.Store({
  modules,
  // 仅开发环境开启 strict（生产环境开着会影响性能）
  strict: import.meta.env.DEV,
});
