//! 「启用 UPnP」：在路由器上给 BT / DHT 的监听端口开洞（UPnP IGD）。
//!
//! 对齐原项目 `src/main/core/UPnPManager.js` + `@motrix/nat-api` 的那套行为：
//!
//! - 映射 `listen-port` 与 `dht-listen-port`，每个端口 **TCP 和 UDP 都映射**
//!   （原项目 `client.map(port)` 不指定协议时正是两种都建）
//! - 租约 7200 秒，到期前 600 秒续租（nat-api 的 `ttl` + `autoUpdate: true`）
//! - 开关关掉、端口改了、应用退出时，撤销不再需要的映射
//! - **拿不到网关不算错**：很多网络没有 UPnP（或路由器里关着），只记日志，
//!   不影响启动和下载
//!
//! 与原项目一样在**应用进程内**做，不交给 aria2 —— 随包的 aria2 构建没有
//! 编译 UPnP 支持，传 `--enable-upnp` 会以 "unrecognized option" 直接退出
//! （见 engine.rs 里那段注释）。
//!
//! 原项目其实只做 UPnP IGD：`new NatAPI({ autoUpdate: true })` 没开 `enablePMP`，
//! 所以这里同样不含 NAT-PMP。

use std::net::{SocketAddr, UdpSocket};
use std::sync::Mutex;
use std::time::Duration;

use igd_next::aio::Gateway;
use igd_next::aio::tokio::{search_gateway, Tokio};
use igd_next::{AddPortError, PortMappingProtocol, RemovePortError, SearchOptions};
use tauri::{AppHandle, Manager, Runtime};

use crate::config_manager;

/// 映射租约（秒）。对齐原项目：nat-api 的 `ttl` 默认 7200
const LEASE_DURATION: u32 = 7200;

/// 续租间隔：租约到期前 600 秒（nat-api 里就是 `ttl - 600`）
const RENEW_INTERVAL: Duration = Duration::from_secs(LEASE_DURATION as u64 - 600);

/// 单次网关发现的等待时长。igd-next 默认 10 秒，压短一点 ——
/// 反正发不出去会很快失败，而退出时那次撤销是同步等的（见 [`shutdown`]）
const SEARCH_TIMEOUT: Duration = Duration::from_secs(5);

/// 路由器映射列表里显示的名字（nat-api 的 `description`）
const DESCRIPTION: &str = "Motr";

/// 一条端口映射：某协议下的某个外部端口。
///
/// 外部端口与本地端口取同一个值，同原项目 `client.map(port)`。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Mapping {
    pub port: u16,
    pub protocol: PortMappingProtocol,
}

impl Mapping {
    /// 日志里的写法：`21301/TCP`
    fn label(self) -> String {
        format!("{}/{}", self.port, self.protocol)
    }
}

/// 进程内的映射记录，对应原项目 `UPnPManager` 里的 `mappingStatus` 与客户端缓存
#[derive(Default)]
pub struct UpnpState {
    /// 已经建立成功的映射。撤销时只动这里面有的 ——
    /// 原项目也是先查 `mappingStatus[port]`，不去动别人的映射
    mapped: Vec<Mapping>,
    /// 发现到的网关。建过映射就留着，续租和退出时不必重新发一次 SSDP
    gateway: Option<Gateway<Tokio>>,
}

/// 期望的映射集合：开关开着才有（对应原项目 `getUPnPMappingEnabled()`）。
///
/// 两个端口 × 两种协议，最多 4 条；配置里两个端口写成同一个值时去重。
pub fn desired_mappings(enabled: bool, ports: config_manager::ListenPorts) -> Vec<Mapping> {
    if !enabled {
        return Vec::new();
    }

    let mut mappings: Vec<Mapping> = Vec::new();

    for port in [ports.bt, ports.dht] {
        // 0 不是可用的监听端口（aria2 里 0 表示"随机"，我们拿不到实际值）
        if port == 0 {
            continue;
        }

        for protocol in [PortMappingProtocol::TCP, PortMappingProtocol::UDP] {
            let mapping = Mapping { port, protocol };
            if !mappings.contains(&mapping) {
                mappings.push(mapping);
            }
        }
    }

    mappings
}

/// 「当前已映射」到「期望」的差：要新建的、要撤销的
pub fn diff(mapped: &[Mapping], desired: &[Mapping]) -> (Vec<Mapping>, Vec<Mapping>) {
    let to_add = desired
        .iter()
        .filter(|mapping| !mapped.contains(*mapping))
        .copied()
        .collect();
    let to_remove = mapped
        .iter()
        .filter(|mapping| !desired.contains(*mapping))
        .copied()
        .collect();

    (to_add, to_remove)
}

/// 读配置算期望值：开关来自 user.json，两个端口来自 system.json
fn wanted<R: Runtime>(app: &AppHandle<R>) -> Vec<Mapping> {
    desired_mappings(
        config_manager::enable_upnp(app),
        config_manager::listen_ports(app),
    )
}

/// 带着锁做点状态读写。里面的闭包必须是同步的 —— 拿锁跨 `await` 会让 future 不再是 Send
fn with_state<R: Runtime, T>(app: &AppHandle<R>, action: impl FnOnce(&mut UpnpState) -> T) -> T {
    let state = app.state::<Mutex<UpnpState>>();
    let mut state = state.lock().unwrap();

    action(&mut state)
}

fn snapshot<R: Runtime>(app: &AppHandle<R>) -> Vec<Mapping> {
    with_state(app, |state| state.mapped.clone())
}

/// 记下「这条已经映射好了」；同样的映射不重复记
fn remember_mapping<R: Runtime>(app: &AppHandle<R>, mapping: Mapping) {
    with_state(app, |state| {
        if !state.mapped.contains(&mapping) {
            state.mapped.push(mapping);
        }
    });
}

/// 忘掉「这条已经映射好了」。**只在确定路由器上确实没有了**时才调
fn forget_mapping<R: Runtime>(app: &AppHandle<R>, mapping: Mapping) {
    with_state(app, |state| state.mapped.retain(|item| *item != mapping));
}

fn cached_gateway<R: Runtime>(app: &AppHandle<R>) -> Option<Gateway<Tokio>> {
    with_state(app, |state| state.gateway.clone())
}

fn store_gateway<R: Runtime>(app: &AppHandle<R>, gateway: Gateway<Tokio>) {
    with_state(app, |state| state.gateway = Some(gateway));
}

/// 丢掉缓存的网关：路由器重启 / 换网之后，缓存的那些 URL 就没用了，下次重新发现
fn forget_gateway<R: Runtime>(app: &AppHandle<R>) {
    with_state(app, |state| state.gateway = None);
}

/// 本机在「通往网关的那张网卡」上的地址：本地 IP + 要转发的端口。
///
/// 拿一个 UDP socket `connect` 到网关来问内核（UDP 的 connect 只设置默认目标，
/// 不发任何包）—— 比枚举网卡再猜一张可靠。
fn local_addr_towards(gateway: SocketAddr, port: u16) -> Option<SocketAddr> {
    let bind = if gateway.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };

    let socket = UdpSocket::bind(bind).ok()?;
    socket.connect(gateway).ok()?;
    let ip = socket.local_addr().ok()?.ip();

    Some(SocketAddr::new(ip, port))
}

/// 网关发现的重试次数。igd-next 每次 `search_gateway` 只发一个 M-SEARCH，
/// 单个包容易错过（路由器刚上电、wifi 刚连上），所以同步/续租时多试几次
const DISCOVERY_ATTEMPTS: u8 = 3;

/// 两次发现之间的间隔
const DISCOVERY_BACKOFF: Duration = Duration::from_secs(2);

/// 退出时撤销映射的总预算：超过就不等了，交给租约到期回收
const EXIT_BUDGET: Duration = Duration::from_secs(5);

/// 取网关：先用缓存的，没有就发 SSDP 发现。
///
/// `attempts` 是发现次数：同步/续租给 [`DISCOVERY_ATTEMPTS`] 次，
/// 退出路径给 1 次 —— 退出不能等。
async fn gateway<R: Runtime>(app: &AppHandle<R>, attempts: u8) -> Option<Gateway<Tokio>> {
    if let Some(gateway) = cached_gateway(app) {
        return Some(gateway);
    }

    for attempt in 1..=attempts {
        // `SearchOptions` 是 non_exhaustive 的，只能先拿默认值再改字段
        let mut options = SearchOptions::default();
        options.timeout = Some(SEARCH_TIMEOUT);

        match search_gateway(options).await {
            Ok(gateway) => {
                log::info!("[Motr] UPnP 网关: {}", gateway.addr);
                store_gateway(app, gateway.clone());

                return Some(gateway);
            }
            Err(err) if attempt == attempts => {
                log::warn!("[Motr] 没找到 UPnP 网关（网络或路由器不支持 UPnP？）: {err}");
            }
            Err(err) => {
                log::debug!("[Motr] 第 {attempt} 次没找到 UPnP 网关: {err}");
                tokio::time::sleep(DISCOVERY_BACKOFF).await;
            }
        }
    }

    None
}

/// 建一条映射
async fn establish<R: Runtime>(app: &AppHandle<R>, gateway: &Gateway<Tokio>, mapping: Mapping) {
    let Some(local_addr) = local_addr_towards(gateway.addr, mapping.port) else {
        log::warn!(
            "[Motr] 取不到本机地址，跳过 UPnP 映射 {}（网关 {}）",
            mapping.label(),
            gateway.addr
        );

        return;
    };

    let result = gateway
        .add_port(
            mapping.protocol,
            mapping.port,
            local_addr,
            LEASE_DURATION,
            DESCRIPTION,
        )
        .await;

    match result {
        Ok(()) => {
            log::info!(
                "[Motr] UPnP 映射 {}：路由器 {} -> 本机 {}（租约 {} 秒）",
                mapping.label(),
                mapping.port,
                local_addr,
                LEASE_DURATION
            );
            remember_mapping(app, mapping);
        }
        // 有些路由器只认"永久租约"（lease_duration = 0），按它的要求再来一次
        Err(AddPortError::OnlyPermanentLeasesSupported) => {
            log::info!(
                "[Motr] UPnP 网关只支持永久租约，改用永久映射 {}",
                mapping.label()
            );

            match gateway
                .add_port(mapping.protocol, mapping.port, local_addr, 0, DESCRIPTION)
                .await
            {
                Ok(()) => remember_mapping(app, mapping),
                Err(err) => {
                    log::warn!("[Motr] UPnP 永久映射 {} 也失败了: {err}", mapping.label());
                    forget_gateway(app);
                }
            }
        }
        Err(err) => {
            log::warn!("[Motr] UPnP 映射 {} 失败: {err}", mapping.label());
            forget_gateway(app);
        }
    }
}

/// 撤销一条映射
async fn release<R: Runtime>(app: &AppHandle<R>, gateway: &Gateway<Tokio>, mapping: Mapping) {
    match gateway.remove_port(mapping.protocol, mapping.port).await {
        Ok(()) => {
            log::info!("[Motr] UPnP 已撤销映射 {}", mapping.label());
            forget_mapping(app, mapping);
        }
        // 路由器上本来就没有，等于已经撤销了
        Err(RemovePortError::NoSuchPortMapping) => {
            log::info!("[Motr] UPnP 映射 {} 本来就不存在", mapping.label());
            forget_mapping(app, mapping);
        }
        Err(err) => {
            log::warn!("[Motr] UPnP 撤销映射 {} 失败: {err}", mapping.label());
            // 留着这条记录，下次同步时再撤销一次
            forget_gateway(app);
        }
    }
}

/// 让路由器上的映射与配置对齐：该建的建、该撤的撤。
///
/// 启动时与设置页保存后各调一次（原项目那套：启动 `startUPnPMapping`、
/// 开关变化 `watchUPnPEnabledChange`、端口变化 `watchUPnPPortsChange`）。
pub async fn sync<R: Runtime>(app: &AppHandle<R>) {
    let desired = wanted(app);
    let (to_add, to_remove) = diff(&snapshot(app), &desired);

    if to_add.is_empty() && to_remove.is_empty() {
        log::debug!("[Motr] UPnP 映射无需改动（{} 条）", desired.len());

        return;
    }

    log::info!(
        "[Motr] UPnP 同步：新建 {} 条、撤销 {} 条",
        to_add.len(),
        to_remove.len()
    );

    let Some(gateway) = gateway(app, DISCOVERY_ATTEMPTS).await else {
        // 撤不掉也留着记录：下次同步（或下次启动）再试
        return;
    };

    for mapping in to_remove {
        release(app, &gateway, mapping).await;
    }

    for mapping in to_add {
        establish(app, &gateway, mapping).await;
    }
}

/// 续租：把期望的映射重新建一遍，路由器会把租约往后推（nat-api 的 `autoUpdate`）。
/// 开关关着时 `desired` 为空，这里什么都不做。
async fn renew<R: Runtime>(app: &AppHandle<R>) {
    let desired = wanted(app);

    if desired.is_empty() {
        return;
    }

    let Some(gateway) = gateway(app, DISCOVERY_ATTEMPTS).await else {
        return;
    };

    log::debug!("[Motr] UPnP 续租 {} 条映射", desired.len());

    for mapping in desired {
        establish(app, &gateway, mapping).await;
    }
}

/// 与配置同步一次。启动时（lib.rs setup）和设置页保存后（commands::apply_upnp）调用。
///
/// 异步执行、立即返回：SSDP 发现最长要等 [`SEARCH_TIMEOUT`]，
/// 不能卡住启动或设置页的保存。
pub fn apply<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        sync(&app).await;
    });
}

/// 后台续租任务，启动时起一次。
///
/// 原项目的续租是 nat-api 在每次成功映射后挂的定时器（`autoUpdate`），
/// 这里用一个长驻任务统一做：每 [`RENEW_INTERVAL`] 重新建一遍映射。
pub fn spawn_renewal_task<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();

    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(RENEW_INTERVAL).await;
            renew(&app).await;
        }
    });
}

/// 退出前撤销所有映射（lib.rs 的 `RunEvent::Exit`）。
///
/// 退出路径没有等待的余地，所以同步跑完；撤销失败也没关系 ——
/// 映射带 2 小时租约，到期后路由器自己会清。
pub fn shutdown<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();

    // 兜一层：退出途中 panic 会连带影响别的退出处理（比如 window-state 落盘）
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tauri::async_runtime::block_on(async move {
            let mapped = snapshot(&app);

            if mapped.is_empty() {
                return;
            }

            // 给个总预算：路由器不响应时不能把退出流程拖住
            if tokio::time::timeout(EXIT_BUDGET, release_all(&app, mapped))
                .await
                .is_err()
            {
                log::warn!(
                    "[Motr] 撤销 UPnP 映射超时（{} 秒），交给租约到期回收",
                    EXIT_BUDGET.as_secs()
                );
            }
        });
    }));
}

/// 逐条撤销（包在 [`shutdown`] 的超时里）。只试一次发现 —— 退出不重试
async fn release_all<R: Runtime>(app: &AppHandle<R>, mappings: Vec<Mapping>) {
    let Some(gateway) = gateway(app, 1).await else {
        return;
    };

    log::info!("[Motr] 退出：撤销 {} 条 UPnP 映射", mappings.len());

    for mapping in mappings {
        release(app, &gateway, mapping).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试里快速构造一对端口（生产代码只从 system.json 读，没有这种用法）
    fn ports(bt: u16, dht: u16) -> config_manager::ListenPorts {
        config_manager::ListenPorts { bt, dht }
    }

    /// 开关关着就不该有任何映射（原项目关掉时会 stopUPnPMapping + closeClient）
    #[test]
    fn no_mapping_when_disabled() {
        assert!(desired_mappings(false, ports(21301, 26701)).is_empty());
    }

    /// 两个端口 × 两种协议 = 4 条，且两种协议都要（原项目 `map(port)` 就是这个语义）
    #[test]
    fn maps_both_protocols_for_both_ports() {
        let mappings = desired_mappings(true, ports(21301, 26701));

        assert_eq!(mappings.len(), 4);
        for (port, protocol) in [
            (21301, PortMappingProtocol::TCP),
            (21301, PortMappingProtocol::UDP),
            (26701, PortMappingProtocol::TCP),
            (26701, PortMappingProtocol::UDP),
        ] {
            assert!(
                mappings.contains(&Mapping { port, protocol }),
                "缺 {port}/{protocol}"
            );
        }
    }

    /// 端口为 0 时不映射（aria2 里 0 表示随机端口，拿不到实际值）
    #[test]
    fn skips_port_zero() {
        assert!(desired_mappings(true, ports(0, 0)).is_empty());
        assert_eq!(desired_mappings(true, ports(0, 26701)).len(), 2);
    }

    /// 两个端口配成同一个值时不去重映射同一条
    #[test]
    fn dedups_same_port() {
        assert_eq!(desired_mappings(true, ports(21301, 21301)).len(), 2);
    }

    /// 差集：缺的补、多的撤
    #[test]
    fn diff_reports_adds_and_removes() {
        let tcp = Mapping {
            port: 21301,
            protocol: PortMappingProtocol::TCP,
        };
        let udp = Mapping {
            port: 21301,
            protocol: PortMappingProtocol::UDP,
        };
        let stale = Mapping {
            port: 26701,
            protocol: PortMappingProtocol::UDP,
        };

        // 只有 tcp：要补 udp，stale 与期望无关（期望里没有它）
        let (to_add, to_remove) = diff(&[tcp, stale], &[tcp, udp]);
        assert_eq!(to_add, vec![udp]);
        assert_eq!(to_remove, vec![stale]);

        // 一致时两边都空 —— 相当于原项目 `enable()` 里先查再设
        let (to_add, to_remove) = diff(&[tcp, udp], &[tcp, udp]);
        assert!(to_add.is_empty() && to_remove.is_empty());

        // 关掉开关：现存的全撤
        let (to_add, to_remove) = diff(&[tcp, udp], &[]);
        assert!(to_add.is_empty());
        assert_eq!(to_remove.len(), 2);
    }

    /// 真机检查：这个网络里的 UPnP 网关能不能被发现。
    ///
    /// 要发 SSDP，所以默认跳过；想确认自己路由器支不支持，跑：
    ///
    /// ```text
    /// cargo test --lib -- --ignored --nocapture discovers_gateway_on_this_network
    /// ```
    #[test]
    #[ignore = "需要网络（会发 SSDP 找网关），默认不跑"]
    fn discovers_gateway_on_this_network() {
        let found = tauri::async_runtime::block_on(async {
            let mut options = SearchOptions::default();
            options.timeout = Some(SEARCH_TIMEOUT);

            search_gateway(options)
                .await
                .map(|gateway| gateway.addr)
                .map_err(|err| err.to_string())
        });

        match found {
            Ok(addr) => println!("找到 UPnP 网关: {addr}"),
            Err(err) => println!("没找到 UPnP 网关: {err}"),
        }
    }

    /// 拿本机地址：给一个回环网关也不能崩，地址族要跟网关一致
    #[test]
    fn local_addr_matches_gateway_family() {
        let addr = local_addr_towards("127.0.0.1:1900".parse().unwrap(), 21301)
            .expect("回环地址应该能取到本机地址");

        assert_eq!(addr.port(), 21301);
        assert!(addr.ip().is_ipv4());

        let addr = local_addr_towards("[::1]:1900".parse().unwrap(), 26701).expect("IPv6 同理");
        assert_eq!(addr.port(), 26701);
    }
}
