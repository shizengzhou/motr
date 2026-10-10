/* global globalThis */
import { Buffer } from 'buffer';

/**
 * 浏览器环境没有 Buffer。
 * 一些 CommonJS 依赖会在模块求值阶段就读取它（如 parse-torrent 里的
 * `Buffer.alloc(0)`），所以必须在应用入口最前面执行本模块。
 */
globalThis.Buffer = globalThis.Buffer || Buffer;
