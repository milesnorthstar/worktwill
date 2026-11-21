//! 将当前 Wry 的平台坐标统一为前端约定的 WebView 物理像素。

pub fn physical_point(x: f64, y: f64, screen_scale: f64, macos: bool) -> (f64, f64) {
    // Wry 0.57 的 AppKit 路径直接上报逻辑点；Windows ScreenToClient 已是物理像素。
    let factor = if macos { screen_scale } else { 1.0 };
    (x * factor, y * factor)
}

#[cfg(test)]
mod tests {
    use super::physical_point;

    #[test]
    fn retina_macos_point_reaches_the_same_dom_position_after_frontend_conversion() {
        let (x, y) = physical_point(421.0, 526.0, 2.0, true);
        assert_eq!((x / 2.0, y / 2.0), (421.0, 526.0));
        assert_ne!((x / 2.0, y / 2.0), (210.5, 263.0));
    }

    #[test]
    fn macos_uses_current_display_scale_without_rounding_fractional_coordinates() {
        for scale in [1.0, 1.5, 2.0] {
            let point = physical_point(611.5, 454.25, scale, true);
            assert_eq!((point.0 / scale, point.1 / scale), (611.5, 454.25));
        }
        assert_eq!(physical_point(-3.5, -1.0, 2.0, true), (-7.0, -2.0));
    }

    #[test]
    fn already_physical_platform_coordinates_are_not_scaled_twice() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            assert_eq!(physical_point(842.0, 1052.0, scale, false), (842.0, 1052.0));
        }
    }
}
