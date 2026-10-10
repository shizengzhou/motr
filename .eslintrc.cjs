/* eslint-env node */
require('@rushstack/eslint-patch/modern-module-resolution');

module.exports = {
  root: true,
  // lint 脚本用了 --ignore-path .gitignore，.eslintignore 不会生效，
  // 所以额外的忽略项写在这里（Rust 构建产物里有生成的 js，不该被 lint）
  ignorePatterns: ['src-tauri/target'],
  extends: [
    'plugin:vue/essential',
    'eslint:recommended',
    '@vue/eslint-config-prettier',
  ],
  // @vue/eslint-config-prettier 带来的是 ecmaVersion: 2020，
  // 识别不了类字段（ES2022，如 `_onmessage = (message) => {}`）等新语法，
  // 这里显式覆盖为 latest
  parserOptions: {
    ecmaVersion: 'latest',
    sourceType: 'module',
  },
  rules: {
    'prettier/prettier': [
      'error',
      {
        singleQuote: true,
      },
    ],
  },
};
