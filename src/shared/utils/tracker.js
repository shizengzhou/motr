import { isEmpty } from 'lodash';
import { fetch } from '@tauri-apps/plugin-http';
import { MAX_BT_TRACKER_LENGTH, ONE_SECOND } from '@shared/constants';

/**
 * 从 tracker 源抓列表，返回各源的原始文本（去重、跳过失败的源）。
 *
 * 请求走 @tauri-apps/plugin-http：它在 Rust 侧用 reqwest 发出去，既不受 webview 的
 * CORS 限制，也能按请求指定代理。原 Electron 版用 axios 在渲染进程直接请求，
 * 代理交给 axios 的 `config.proxy` —— 那套只对 Node 生效。
 *
 * @param {string[]} source tracker 源 URL
 * @param {string|undefined} proxyUrl 这条请求要走的代理；undefined 表示交给系统代理
 */
export const fetchBtTrackerFromSource = async (source, proxyUrl) => {
  if (isEmpty(source)) {
    return [];
  }

  const now = Date.now();
  const options = {
    connectTimeout: 30 * ONE_SECOND,
    ...(proxyUrl ? { proxy: { all: proxyUrl } } : {}),
  };

  const promises = source.map(async (url) => {
    const response = await fetch(`${url}?t=${now}`, options);

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}`);
    }

    return response.text();
  });

  const results = await Promise.allSettled(promises);
  const values = results
    .filter((item) => item.status === 'fulfilled')
    .map((item) => item.value)
    .filter((value) => value);

  return [...new Set(values)];
};

export const convertTrackerDataToLine = (arr = []) => {
  const result = arr
    .join('\r\n')
    .replace(/^\s*[\r\n]/gm, '')
    .trim();
  return result;
};

export const convertTrackerDataToComma = (arr = []) => {
  const result = convertTrackerDataToLine(arr)
    .replace(/(?:\r\n|\r|\n)/g, ',')
    .trim();
  return result;
};

export const reduceTrackerString = (str = '') => {
  if (str.length <= MAX_BT_TRACKER_LENGTH) {
    return str;
  }

  const subStr = str.substring(0, MAX_BT_TRACKER_LENGTH);
  const index = subStr.lastIndexOf(',');
  if (index === -1) {
    return subStr;
  }

  const result = subStr.substring(0, index);
  return result;
};
