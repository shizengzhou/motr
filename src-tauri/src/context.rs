use serde::Serialize;

/// 运行时环境信息，对应 JS `loadConfigFromNativeStore()` 里的 context
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Context {
    pub platform: String,
    pub arch: String,
    pub log_path: String,
    pub session_path: String,
    pub engine_path: String,
    pub aria2_bin_path: String,
    pub aria2_conf_path: String,
}
