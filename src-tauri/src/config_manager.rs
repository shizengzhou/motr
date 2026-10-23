use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

/// BT 监听端口默认值（system.json 的 listen-port，与前端 `@shared/constants.js` 一致）
pub const ENGINE_BT_PORT: u16 = 21301;

/// DHT 监听端口默认值（system.json 的 dht-listen-port）
pub const ENGINE_DHT_PORT: u16 = 26701;

/// 从配置文件里直接取一个键再反序列化。
///
/// Rust 侧剩下的几个开关（日志等级、UPnP、监听端口）都是「现读现用」的，
/// 且都由前端 `ConfigManager`（JS 侧 LazyStore）负责写入 —— 这里只读不写。
/// 单键读取比整份反序列化便宜，也不会因为某个无关的键类型不对就全盘失败；
/// 缺键或类型不对时返回 `None`，由调用方决定默认值。
fn get_value<T, R>(app_handle: &AppHandle<R>, file: &str, key: &str) -> Option<T>
where
    T: serde::de::DeserializeOwned,
    R: Runtime,
{
    app_handle
        .store(file)
        .ok()
        .and_then(|store| store.get(key))
        .and_then(|value| serde_json::from_value(value).ok())
}

/// 「启用 UPnP」是否开启：读 user.json，缺键或读不到时按默认值（true，与原项目一致）
///
/// 开关能在运行中改（见 upnp.rs），所以这里是现读现用，不是只在启动时读一次。
pub fn enable_upnp<R: Runtime>(app_handle: &AppHandle<R>) -> bool {
    get_value(app_handle, "user.json", "enable-upnp").unwrap_or(true)
}

/// 需要映射到路由器的两个监听端口（BT / DHT）：读 system.json，缺键或读不到时按默认值
pub fn listen_ports<R: Runtime>(app_handle: &AppHandle<R>) -> (u16, u16) {
    (
        get_value(app_handle, "system.json", "listen-port").unwrap_or(ENGINE_BT_PORT),
        get_value(app_handle, "system.json", "dht-listen-port").unwrap_or(ENGINE_DHT_PORT),
    )
}

/// 设置页里的日志等级：读 user.json，缺键或读不到时按默认值（warn）
///
/// 只在启动时用一次（logging::init），所以设置页保存后下次启动生效。
pub fn log_level<R: Runtime>(app_handle: &AppHandle<R>) -> String {
    get_value(app_handle, "user.json", "log-level").unwrap_or_else(|| "warn".to_string())
}
