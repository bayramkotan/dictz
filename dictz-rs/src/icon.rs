//! The dictz icon (an open book on a blue rounded square), drawn in code —
//! the same pixels as the Python edition's `dictz/ui/icons.py::app_icon_rgba`.

/// `n`×`n` RGBA pixels (designed for n = 64).
pub fn app_icon_rgba(n: usize) -> Vec<u8> {
    let mut rgba = vec![0u8; n * n * 4];
    let r = 12.0f32;
    let inside = |x: f32, y: f32| {
        let cx = x.clamp(r, n as f32 - r);
        let cy = y.clamp(r, n as f32 - r);
        (x - cx).powi(2) + (y - cy).powi(2) <= r * r
    };
    for y in 0..n {
        for x in 0..n {
            if !inside(x as f32 + 0.5, y as f32 + 0.5) {
                continue;
            }
            let mut c = [0x25u8, 0x63, 0xEB, 0xFF];
            let page = (14..=30).contains(&x) || (33..=49).contains(&x);
            let top = 18
                + if x < 32 {
                    (30 - x.min(30)) / 6
                } else {
                    (x.max(33) - 33) / 6
                };
            if page && (top..=46).contains(&y) {
                c = [0xFF, 0xFF, 0xFF, 0xFF];
                let line_row = y > top + 4 && (y - top) % 6 == 0 && y < 44;
                let line_col = (17..=27).contains(&x) || (36..=46).contains(&x);
                if line_row && line_col {
                    c = [0x93, 0xB4, 0xF5, 0xFF];
                }
            }
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&c);
        }
    }
    rgba
}

/// FNV-1a 64 of the icon pixels (parity check with the Python edition).
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in data {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}
