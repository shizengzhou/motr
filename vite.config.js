import { fileURLToPath, URL } from 'node:url';

import { defineConfig } from 'vite';
import legacy from '@vitejs/plugin-legacy';
import vue2 from '@vitejs/plugin-vue2';

// https://vitejs.dev/config/
export default defineConfig({
  server: {
    watch: {
      ignored: ['**/src-tauri/**'],
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        // 等价于原 webpack 里 sass-loader 的 additionalData：
        // 每个 scss / <style lang="scss"> 入口都注入主题变量与 mixin
        additionalData: `@import "@/components/Theme/Variables.scss";\n`,
      },
    },
  },
  plugins: [
    vue2(),
    legacy({
      targets: ['ie >= 11'],
      additionalLegacyPolyfills: ['regenerator-runtime/runtime'],
    }),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
      '@shared': fileURLToPath(new URL('./src/shared', import.meta.url)),
      // 必须让 element-ui 内部的 require('vue') 与应用的 import 'vue' 落到同一份运行时。
      // vue 2.7 的条件导出会把它拆成两份：
      //   应用侧（import）      → vue/dist/vue.runtime.esm.js
      //   element-ui（CJS require）→ vue/dist/vue.runtime.common.js
      // 两套响应式系统互不可见 ⇒ element-ui 内部 new Vue 创建的 store 触发的更新
      // 不会让组件的 render watcher 失效。表现就是 el-table 的列注册之后表头/表体
      // 不再重渲染（表格空白），生产构建下还会引发"测量↔重排"死循环把界面卡死。
      // 注意不能用 resolve.dedupe：它按版本去重，而这两份是同一版本、不同导出条件。
      vue: 'vue/dist/vue.runtime.esm.js',
      // Vite 不做 Node 内置模块 polyfill（会把它们换成空对象）。
      // parse-torrent 解析 .torrent 时会用 path.join/path.sep，
      // 不映射成 path-browserify 就会报 "undefined is not an object (evaluating 'path.join.apply')"
      path: 'path-browserify',
    },
  },
});
