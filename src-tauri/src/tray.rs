use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    AppHandle,
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::config_manager;

/// 菜单模块按这个 id 找回托盘来挂右键菜单（menu::apply）
pub const TRAY_ID: &str = "motr-tray";

/// 当前是否显示「下载中」图标：reset_icon 恢复静态图标时要看它，
/// 免得把下载中的 active 图标冲掉（仅 Win/Linux 有意义，macOS 永远 normal）
static SHOW_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 各平台的静态托盘图标，对齐原项目 TrayManager.loadImages*：
/// - macOS：只有 light-normal 一个状态（template image，深浅色由系统自动反色）
/// - Windows：colorful 的 normal / active 两个状态
/// - Linux：跟随托盘主题的 light / dark × normal / active
#[cfg(target_os = "macos")]
const MACOS_NORMAL_ICON: &[u8] = include_bytes!("../icons/mo-tray-light-normal.png");

#[cfg(target_os = "windows")]
const WINDOWS_NORMAL_ICON: &[u8] = include_bytes!("../icons/mo-tray-colorful-normal.png");
#[cfg(target_os = "windows")]
const WINDOWS_ACTIVE_ICON: &[u8] = include_bytes!("../icons/mo-tray-colorful-active.png");

#[cfg(target_os = "linux")]
const LINUX_LIGHT_NORMAL_ICON: &[u8] = include_bytes!("../icons/mo-tray-light-normal.png");
#[cfg(target_os = "linux")]
const LINUX_LIGHT_ACTIVE_ICON: &[u8] = include_bytes!("../icons/mo-tray-light-active.png");
#[cfg(target_os = "linux")]
const LINUX_DARK_NORMAL_ICON: &[u8] = include_bytes!("../icons/mo-tray-dark-normal.png");
#[cfg(target_os = "linux")]
const LINUX_DARK_ACTIVE_ICON: &[u8] = include_bytes!("../icons/mo-tray-dark-active.png");

/// 当前该显示哪个静态图标。macOS 无视 active（template 图只有一个状态，
/// 且开启速率计后图标由前端画图覆盖，见 set_speedometer）
fn static_icon(app: &AppHandle, active: bool) -> &'static [u8] {
    #[cfg(target_os = "macos")]
    {
        let _ = (app, active);
        MACOS_NORMAL_ICON
    }

    #[cfg(target_os = "windows")]
    {
        let _ = app;
        if active {
            WINDOWS_ACTIVE_ICON
        } else {
            WINDOWS_NORMAL_ICON
        }
    }

    #[cfg(target_os = "linux")]
    {
        // 主题取 user.json 的 tray-theme（与原项目同源）。auto 和未知值都按 dark：
        // 多数 Linux 桌面托盘是深色的，浅色图标会看不见
        if config_manager::tray_theme(app) == "light" {
            if active {
                LINUX_LIGHT_ACTIVE_ICON
            } else {
                LINUX_LIGHT_NORMAL_ICON
            }
        } else if active {
            LINUX_DARK_ACTIVE_ICON
        } else {
            LINUX_DARK_NORMAL_ICON
        }
    }
}

/// 加载内置托盘图标（编译期嵌入，文件损坏才是编程错误）
fn load_icon(bytes: &[u8]) -> Image<'static> {
    Image::from_bytes(bytes).expect("内置托盘图标应可解析")
}

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    // 带上当前 active 状态：启动时它必然是 false，而运行模式从 HideTray 切回来
    // 重建托盘时（run_mode::apply_tray），下载中的 active 图标不会跳回 normal
    let icon = load_icon(static_icon(app, SHOW_ACTIVE.load(Ordering::Relaxed)));

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        // macOS 菜单栏图标用 template image，深浅色主题下由系统自动反色
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("Motr")
        // 左键留给"聚焦窗口"，所以不让左键弹菜单；右键弹菜单由系统直接处理
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::window::show_main_window(tray.app_handle());
            }
        })
        // 右键菜单由 menu 模块按 src-tauri/resources/menus/tray.json 构建后挂上
        .build(app)?;

    Ok(())
}

/// 「下载中」状态变化（等价原项目 TrayManager.handleDownloadStatusChange）：
/// Win/Linux 在 normal / active 图标间切换；macOS 无视——template 图只有一个状态，
/// 开启速率计后托盘由前端画的速率图接管
pub(crate) fn set_active(app: &AppHandle, active: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = (app, active);
        return;
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        if SHOW_ACTIVE.swap(active, Ordering::Relaxed) == active {
            return;
        }

        let _ = apply_icon(app, load_icon(static_icon(app, active)));
    }
}

/// 恢复静态托盘图标（关掉「托盘显示实时速度」、组件卸载时由前端调用）
pub(crate) fn reset_icon(app: &AppHandle) -> Result<(), String> {
    let icon = load_icon(static_icon(app, SHOW_ACTIVE.load(Ordering::Relaxed)));
    apply_icon(app, icon)
}

/// macOS 的 tray-icon 会把图标**高度**固定缩放到 18pt（宽度按比例，见
/// tray-icon/src/platform_impl/macos/mod.rs 的 set_icon_for_ns_status_item_button），
/// 而速率图的内容是按 16pt 画的（原 Electron 版靠 @2x + scaleFactor 2 做到）。
/// 所以上下各补 h/16 像素：内容高度 = h × 18 / (h + 2·h/16) = 16pt。
fn pad_for_tray(content: &Image<'_>) -> Image<'static> {
    let width = content.width();
    let height = content.height();
    let pad = (height / 16).max(1) as usize;
    let row_bytes = (width * 4) as usize;

    let mut rgba = vec![0u8; row_bytes * (height as usize + pad * 2)];
    let src = content.rgba();
    for y in 0..height as usize {
        let src_start = y * row_bytes;
        let dst_start = (y + pad) * row_bytes;
        rgba[dst_start..dst_start + row_bytes]
            .copy_from_slice(&src[src_start..src_start + row_bytes]);
    }

    Image::new_owned(rgba, width, height + pad as u32 * 2)
}

/// 把渲染进程画好的「图标 + 上下行速率」PNG 换成托盘图标（命令在 commands.rs）。
/// 和原 Electron 版一致：canvas 在渲染进程画，这边只负责 setImage。
pub(crate) fn set_speedometer(app: &AppHandle, png: &[u8]) -> Result<(), String> {
    let content = Image::from_bytes(png).map_err(|err| format!("解析托盘速率图失败: {err}"))?;
    apply_icon(app, pad_for_tray(&content))
}

fn apply_icon(app: &AppHandle, icon: Image<'_>) -> Result<(), String> {
    let tray = app
        .tray_by_id(TRAY_ID)
        .ok_or_else(|| format!("找不到 id 为 {TRAY_ID} 的托盘"))?;

    tray.set_icon(Some(icon)).map_err(|err| err.to_string())?;

    // tray-icon 的 set_icon 会把 template 标记重置成 false，
    // 深浅色菜单栏自动反色全靠它，所以重新设回去
    #[cfg(target_os = "macos")]
    tray.set_icon_as_template(true)
        .map_err(|err| err.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 六个平台的托盘 PNG 都必须能解码（防止替换资源时放错文件或损坏）。
    /// 这里用 include_bytes! 直接引文件、不经过按平台 cfg 的常量，
    /// 所以无论在哪个系统上跑测试都会全部检查
    #[test]
    fn all_tray_icons_decode() {
        for (name, bytes) in [
            ("light-normal", &include_bytes!("../icons/mo-tray-light-normal.png")[..]),
            ("light-active", &include_bytes!("../icons/mo-tray-light-active.png")[..]),
            ("colorful-normal", &include_bytes!("../icons/mo-tray-colorful-normal.png")[..]),
            ("colorful-active", &include_bytes!("../icons/mo-tray-colorful-active.png")[..]),
            ("dark-normal", &include_bytes!("../icons/mo-tray-dark-normal.png")[..]),
            ("dark-active", &include_bytes!("../icons/mo-tray-dark-active.png")[..]),
        ] {
            assert!(
                Image::from_bytes(bytes).is_ok(),
                "托盘图标 mo-tray-{name}.png 解析失败"
            );
        }
    }

    /// 速率图要按「画布 18pt、内容 16pt」补上下边：132×32 -> 132×36，内容下移 2px
    #[test]
    fn speedometer_icon_is_padded_to_16pt() {
        // 造一张 132×32 的图，每行填 y 值，便于验证拷贝位置
        let (width, height) = (132u32, 32u32);
        let row_bytes = (width * 4) as usize;
        let mut rgba = vec![0u8; row_bytes * height as usize];
        for y in 0..height as usize {
            for x in 0..width as usize {
                let i = y * row_bytes + x * 4;
                rgba[i] = y as u8;
                rgba[i + 3] = 255;
            }
        }

        let padded = pad_for_tray(&Image::new_owned(rgba, width, height));
        assert_eq!((padded.width(), padded.height()), (132, 36));

        let data = padded.rgba();
        let pixel = |x: usize, y: usize| data[y * row_bytes + x * 4];

        // 补出来的上下各 2 行必须是全透明
        for y in [0, 1, 34, 35] {
            assert!(
                (0..width as usize).all(|x| data[y * row_bytes + x * 4 + 3] == 0),
                "第 {y} 行应该是空的"
            );
        }

        // 内容整体下移 2px，且宽度方向不动
        assert_eq!(pixel(0, 2), 0);
        assert_eq!(pixel(0, 33), 31);
        assert_eq!(pixel(131, 2), 0);
        assert_eq!(pixel(131, 33), 31);

        // 18pt 画布下，内容渲染出来正好是 16pt
        let content_pt = 18.0 * height as f64 / padded.height() as f64;
        assert!(
            (content_pt - 16.0).abs() < 0.001,
            "内容应渲染成 16pt，实际 {content_pt}pt"
        );
    }

    /// 托盘图标的尺寸与留白：tray-icon 在 macOS 上固定把「画布」缩放到 18pt
    /// （不管源图像素、也不认 @2x），所以图里必须留白，让实际图形渲染出来是 16pt
    /// （与原 Electron 版一致）。删掉留白会让图标比别人大一圈。
    #[test]
    fn tray_icon_renders_at_16pt() {
        let icon = Image::from_bytes(include_bytes!("../icons/mo-tray-light-normal.png")).unwrap();
        let (width, height) = (icon.width(), icon.height());
        assert_eq!((width, height), (36, 36));

        let rgba = icon.rgba();
        let (mut x0, mut y0, mut x1, mut y1) = (width, height, 0, 0);
        for y in 0..height {
            for x in 0..width {
                if rgba[((y * width + x) * 4 + 3) as usize] > 32 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                }
            }
        }

        // 图形应居中且四周各留 2px
        assert_eq!((x0, y0, x1, y1), (2, 2, width - 3, height - 3));

        // 18pt 画布下，图形高度换算出来应约等于 16pt
        let rendered_pt = 18.0 * (y1 - y0 + 1) as f64 / height as f64;
        assert!(
            (rendered_pt - 16.0).abs() < 0.5,
            "图形实际渲染高度 {rendered_pt}pt，应为 16pt"
        );
    }
}
