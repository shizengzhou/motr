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
      // Vite 不做 Node 内置模块 polyfill（会把它们换成空对象）。
      // parse-torrent 解析 .torrent 时会用 path.join/path.sep，
      // 不映射成 path-browserify 就会报 "undefined is not an object (evaluating 'path.join.apply')"
      path: 'path-browserify',
    },
  },
});
