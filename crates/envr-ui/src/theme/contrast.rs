//! WCAG 2.1 contrast helpers (`tasks_gui.md` GUI-050).

use super::Srgb;

fn channel_linear(c: f32) -> f64 {
    let c = f64::from(c.clamp(0.0, 1.0));
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Relative luminance \(L\) in sRGB (WCAG).
pub fn relative_luminance(s: Srgb) -> f64 {
    let r = channel_linear(s.r);
    let g = channel_linear(s.g);
    let b = channel_linear(s.b);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Contrast ratio between two **opaque** sRGB colors (larger = more contrast).
pub fn contrast_ratio(a: Srgb, b: Srgb) -> f64 {
    let l1 = relative_luminance(a);
    let l2 = relative_luminance(b);
    let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (hi + 0.05) / (lo + 0.05)
}
