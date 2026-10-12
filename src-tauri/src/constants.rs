use serde::{Deserialize, Serialize};

/// 代理生效范围，对应 JS 的 `PROXY_SCOPES`
///
/// 与 JS 的差异：Rust 没有「常量对象」，用 enum 表达；
/// 反序列化走 `from`/`into = "String"`，因此
/// - `UpdateApp` 序列化成 `"update-app"`（与 user.json 一致）
/// - 遇到未知字符串只会落到 `Other`，不会让整个 UserConfig 反序列化失败
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum ProxyScope {
    Download,
    UpdateApp,
    UpdateTrackers,
    /// 兜底：前端将来新增范围或配置里出现未知值时保留原值
    Other(String),
}

impl ProxyScope {
    pub const DOWNLOAD: &'static str = "download";
    pub const UPDATE_APP: &'static str = "update-app";
    pub const UPDATE_TRACKERS: &'static str = "update-trackers";

    /// 对应 JS 的 `PROXY_SCOPE_OPTIONS`
    pub const OPTIONS: [ProxyScope; 3] = [
        ProxyScope::Download,
        ProxyScope::UpdateApp,
        ProxyScope::UpdateTrackers,
    ];

    pub fn as_str(&self) -> &str {
        match self {
            ProxyScope::Download => Self::DOWNLOAD,
            ProxyScope::UpdateApp => Self::UPDATE_APP,
            ProxyScope::UpdateTrackers => Self::UPDATE_TRACKERS,
            ProxyScope::Other(value) => value,
        }
    }
}

impl From<String> for ProxyScope {
    fn from(value: String) -> Self {
        match value.as_str() {
            Self::DOWNLOAD => Self::Download,
            Self::UPDATE_APP => Self::UpdateApp,
            Self::UPDATE_TRACKERS => Self::UpdateTrackers,
            _ => Self::Other(value),
        }
    }
}

impl From<ProxyScope> for String {
    fn from(scope: ProxyScope) -> Self {
        scope.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_matches_js_values() {
        assert_eq!(
            serde_json::to_string(&ProxyScope::Download).unwrap(),
            "\"download\""
        );
        assert_eq!(
            serde_json::to_string(&ProxyScope::UpdateApp).unwrap(),
            "\"update-app\""
        );
        assert_eq!(
            serde_json::to_string(&ProxyScope::UpdateTrackers).unwrap(),
            "\"update-trackers\""
        );
    }

    #[test]
    fn deserialize_known_and_unknown() {
        assert_eq!(
            serde_json::from_str::<ProxyScope>("\"update-app\"").unwrap(),
            ProxyScope::UpdateApp
        );
        assert_eq!(
            serde_json::from_str::<ProxyScope>("\"whatever\"").unwrap(),
            ProxyScope::Other("whatever".to_string())
        );
        assert_eq!(ProxyScope::OPTIONS.len(), 3);
        assert_eq!(
            serde_json::to_string(&ProxyScope::OPTIONS.to_vec()).unwrap(),
            "[\"download\",\"update-app\",\"update-trackers\"]"
        );
    }
}
