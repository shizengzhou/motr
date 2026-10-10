import { appDataDir, downloadDir, resolve } from '@tauri-apps/api/path';

import {
  ENGINE_MAX_CONNECTION_PER_SERVER,
  IP_VERSION,
} from '../shared/constants';

export const getMaxConnectionPerServer = () => {
  return ENGINE_MAX_CONNECTION_PER_SERVER;
};

export const getDhtPath = async (protocol) => {
  const name = protocol === IP_VERSION.V6 ? 'dht6.dat' : 'dht.dat';
  return resolve(await appDataDir(), `./${name}`);
};

export const getUserDownloadsPath = () => {
  return downloadDir();
};

const ELECTRON_DEFAULT_LOCALE = 'en-US';

/**
 * Chromium/Electron 只对下列语言保留地区信息，
 * 其余语言（ja、ko、ru、th ...）一律返回裸语言代码
 */
const ELECTRON_LOCALES_WITH_REGION = [
  'ar',
  'de',
  'en',
  'es',
  'fr',
  'it',
  'pt',
  'zh',
];

/**
 * Chromium/Electron 把中文的脚本子标签折算成地区：
 * zh-Hans → zh-CN，zh-Hant → zh-TW
 */
const CHINESE_SCRIPT_TO_REGION = {
  hans: 'CN',
  hant: 'TW',
};

/**
 * 把 Tauri（plugin-os 的 locale()，底层是 sys-locale）返回的 locale
 * 转成 Electron app.getLocale() 的风格。
 *
 * - null / '' / 'C' / 'POSIX' → 'en-US'
 * - 'en_US.UTF-8' / 'en_US@euro' → 'en-US'（去掉编码与修饰符，分隔符统一为 '-'）
 * - 'zh-Hans-CN' / 'zh-Hans' → 'zh-CN'，'zh-Hant-TW' / 'zh-Hant' → 'zh-TW'
 * - 'ja-JP' → 'ja'、'ru-RU' → 'ru'（Electron 对这类语言不带地区）
 * - 'EN-us' → 'en-US'（语言小写、地区大写）
 * - 其余脚本子标签（如 sr-Latn-RS 的 Latn）会被丢弃
 */
export const toElectronLocale = (locale) => {
  if (typeof locale !== 'string') {
    return ELECTRON_DEFAULT_LOCALE;
  }

  const normalized = locale.trim().replace(/_/g, '-').split(/[.@]/)[0];
  if (!normalized || /^(c|posix)$/i.test(normalized)) {
    return ELECTRON_DEFAULT_LOCALE;
  }

  const [language, ...subtags] = normalized.split('-').filter(Boolean);
  if (!language) {
    return ELECTRON_DEFAULT_LOCALE;
  }

  const lang = language.toLowerCase();
  const lowerSubtags = subtags.map((subtag) => subtag.toLowerCase());
  // 4 位字母是脚本（Hans/Hant/Latn），2 位字母或 3 位数字是地区
  const script = lowerSubtags.find((subtag) => subtag.length === 4);
  let region =
    lowerSubtags.find(
      (subtag) => subtag.length === 2 || /^\d{3}$/.test(subtag)
    ) || '';

  // 中文没有地区时，用脚本子标签推断（macOS 常见 zh-Hant / zh-Hans）
  if (lang === 'zh' && !region) {
    region = CHINESE_SCRIPT_TO_REGION[script] || '';
  }

  if (!region || !ELECTRON_LOCALES_WITH_REGION.includes(lang)) {
    return lang;
  }

  return `${lang}-${region.toUpperCase()}`;
};
