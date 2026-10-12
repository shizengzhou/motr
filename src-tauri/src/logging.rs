use tauri::AppHandle;
use tauri_plugin_log::Builder;

use crate::config_manager;

/// 设置页「日志等级」下拉框里的取值（前端 `@shared/constants.js` 的 `LOG_LEVELS`）。
///
/// 顺序是"从安静到啰嗦"：error < warn < info < debug ——
/// 用来校验映射没把粗细搞反（见 tests）。
const LEVEL_NAMES: [&str; 4] = ["error", "warn", "info", "debug"];

/// 名字认不出来时用的等级（也是 user.json 里 log-level 的默认值）
const DEFAULT_LEVEL: log::LevelFilter = log::LevelFilter::Warn;

/// user.json 里的等级名 -> Rust 的 `LevelFilter`。
///
/// 四个名字与下拉框一一对应。老配置里若还留着 `verbose` / `silly`
/// （原项目带过这两级），会走默认值并记一条 warn 日志。
fn level_filter(level: &str) -> log::LevelFilter {
    match level {
        "error" => log::LevelFilter::Error,
        "warn" => log::LevelFilter::Warn,
        "info" => log::LevelFilter::Info,
        "debug" => log::LevelFilter::Debug,
        _ => DEFAULT_LEVEL,
    }
}

/// 按 user.json 里的 `log-level` 建日志插件。
///
/// 对应原项目 `Application.setupLogger` 里那句 `logger.transports.file.level = logLevel`，
/// 差别是：electron-log 运行中就能改，而这里的等级是构建期的 fern 过滤器，
/// 所以**改完要下次启动才生效**（设置页那边也只是把它写进 user.json）。
pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let configured = config_manager::log_level(app);
    let level = level_filter(&configured);

    app.plugin(
        Builder::default()
            .level(level)
            // 单个日志文件最大 1M（1_000_000 字节），超出后轮转
            .max_file_size(1_000_000)
            .build(),
    )?;

    // 装好之后再打，否则这条日志自己就被过滤器丢掉了
    if LEVEL_NAMES.contains(&configured.as_str()) {
        log::info!("[Motr] 日志等级: {configured} -> {level}");
    } else {
        log::warn!("[Motr] 未知的日志等级 {configured:?}，按 {DEFAULT_LEVEL} 处理");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 下拉框里的四个名字都有对应等级
    #[test]
    fn maps_frontend_level_names() {
        assert_eq!(level_filter("error"), log::LevelFilter::Error);
        assert_eq!(level_filter("warn"), log::LevelFilter::Warn);
        assert_eq!(level_filter("info"), log::LevelFilter::Info);
        assert_eq!(level_filter("debug"), log::LevelFilter::Debug);
    }

    /// 认不出来的名字（user.json 是可以手改的）按默认值处理，不 panic。
    /// `verbose` / `silly` 是原项目有、移植版已去掉的两级，老配置里可能还留着。
    #[test]
    fn falls_back_to_default_for_unknown_level() {
        assert_eq!(level_filter(""), DEFAULT_LEVEL);
        assert_eq!(level_filter("Verbose"), DEFAULT_LEVEL, "大小写不匹配也算未知");
        assert_eq!(level_filter("verbose"), DEFAULT_LEVEL);
        assert_eq!(level_filter("silly"), DEFAULT_LEVEL);
        // trace 是 Rust 的等级名，但不在前端的下拉框里
        assert_eq!(level_filter("trace"), DEFAULT_LEVEL);
        assert_eq!(level_filter("silly!"), DEFAULT_LEVEL);
    }

    /// 等级映射不能把粗细搞反：LEVEL_NAMES 越靠后，过滤器必须越宽松
    #[test]
    fn mapping_is_monotonic() {
        let filters = LEVEL_NAMES.map(level_filter);

        for pair in filters.windows(2) {
            assert!(
                pair[1] >= pair[0],
                "{} 应该比 {} 更啰嗦",
                pair[1],
                pair[0]
            );
        }

        // 最安静的一级只留 error，最啰嗦的一级是 debug
        assert_eq!(filters[0], log::LevelFilter::Error);
        assert_eq!(filters[LEVEL_NAMES.len() - 1], log::LevelFilter::Debug);
    }
}
