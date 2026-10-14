import { PROXY_SCOPES } from '@shared/constants';

// 设置页允许省略协议（placeholder 是 [http://][USER:PASSWORD@]HOST[:PORT]），
// 但按标准 URL 解析的地方（更新检查）必须先补上协议，否则 HOST:PORT 会被当成长度为 4 的 scheme
const withScheme = (server) => {
  if (!server) {
    return '';
  }

  return /^[a-z][a-z0-9+.-]*:\/\//i.test(server) ? server : `http://${server}`;
};

const scopeEnabled = (proxy = {}, scope) =>
  Boolean(proxy.enable && proxy.server && (proxy.scope || []).includes(scope));

/**
 * aria2 用的两个系统配置项（键名与 system.json 一致，都是 systemKeys）。
 *
 * 对齐原项目 `ConfigManager.fixSystemConfig` / `Application.watchProxyChange`：
 * 只有 scope 勾了「下载」时才把代理交给引擎，否则写空串
 *（原项目此处不清理旧值，关掉代理后 aria2 还会继续走代理；Rust 侧的
 * src-tauri/src/proxy.rs 是同一套规则，保持一致）。
 */
export const aria2ProxyOptions = (proxy = {}) => {
  const enabled = scopeEnabled(proxy, PROXY_SCOPES.DOWNLOAD);

  return {
    'all-proxy': enabled ? proxy.server : '',
    'no-proxy': enabled ? proxy.bypass || '' : '',
  };
};

/**
 * 某个 scope 下要用的代理地址。
 * 没勾这个 scope（或没填服务器）时返回 undefined，让调用方回落到默认行为（系统代理）。
 */
export const proxyUrlForScope = (proxy = {}, scope) => {
  if (!scopeEnabled(proxy, scope)) {
    return undefined;
  }

  return withScheme(proxy.server);
};
