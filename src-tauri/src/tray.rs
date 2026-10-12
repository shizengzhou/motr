use tauri::{
    AppHandle, Manager,
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

/// 菜单模块按这个 id 找回托盘来挂右键菜单（menu::apply）
pub const TRAY_ID: &str = "motr-tray";

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let icon = Image::from_bytes(include_bytes!("../icons/mo-tray-light-normal.png"))?;

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
                show_main_window(tray.app_handle());
            }
        })
        // 右键菜单由 menu 模块按 src-tauri/resources/menus/tray.json 构建后挂上
        .build(app)?;

    Ok(())
}

/// 静态托盘图标：速率计关闭 / 组件卸载时恢复成它
const STATIC_ICON: &[u8] = include_bytes!("../icons/mo-tray-light-normal.png");

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

/// 把渲染进程画好的「图标 + 上下行速率」PNG 换成托盘图标。
/// 和原 Electron 版一致：canvas 在渲染进程画，这边只负责 setImage。
#[tauri::command]
pub fn set_tray_speedometer(app: AppHandle, png: Vec<u8>) -> Result<(), String> {
    let content = Image::from_bytes(&png).map_err(|err| format!("解析托盘速率图失败: {err}"))?;
    apply_icon(&app, pad_for_tray(&content))
}

/// 恢复静态图标（关掉「托盘显示实时速度」、组件卸载时由前端调用）
#[tauri::command]
pub fn reset_tray_icon(app: AppHandle) -> Result<(), String> {
    let icon = Image::from_bytes(STATIC_ICON).map_err(|err| format!("解析托盘图标失败: {err}"))?;
    apply_icon(&app, icon)
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

/// 显示并聚焦主窗口（托盘左键、菜单里的 application:show 都用它）
pub(crate) fn show_main_window(app: &AppHandle) {
    // macOS: 应用被 hide 掉时窗口不会出现，需要先 unhide
    #[cfg(target_os = "macos")]
    let _ = app.show();

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
