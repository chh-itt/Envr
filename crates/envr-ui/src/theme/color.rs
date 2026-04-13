/// sRGB color in linear 0–1 (`iced` uses the same convention for `Color::from_rgb`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Srgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Srgb {
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub const fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Parses `#RGB` or `#RRGGBB` (case-insensitive). Used for persisted accent strings.
    #[allow(clippy::result_unit_err)] // Opaque parse failure; callers only need ok/err
    pub fn from_hex(s: &str) -> Result<Self, ()> {
        fn nibble(h: u8) -> Result<u8, ()> {
            match h {
                b'0'..=b'9' => Ok(h - b'0'),
                b'a'..=b'f' => Ok(h - b'a' + 10),
                b'A'..=b'F' => Ok(h - b'A' + 10),
                _ => Err(()),
            }
        }

        let s = s.trim();
        let s = s.strip_prefix('#').unwrap_or(s);
        let b = s.as_bytes();
        let (r, g, b) = match b.len() {
            6 => {
                let r = u8::from_str_radix(std::str::from_utf8(&b[0..2]).map_err(|_| ())?, 16)
                    .map_err(|_| ())?;
                let g = u8::from_str_radix(std::str::from_utf8(&b[2..4]).map_err(|_| ())?, 16)
                    .map_err(|_| ())?;
                let bl = u8::from_str_radix(std::str::from_utf8(&b[4..6]).map_err(|_| ())?, 16)
                    .map_err(|_| ())?;
                (r, g, bl)
            }
            3 => {
                let r = nibble(b[0]).map_err(|_| ())? * 17;
                let g = nibble(b[1]).map_err(|_| ())? * 17;
                let bl = nibble(b[2]).map_err(|_| ())? * 17;
                (r, g, bl)
            }
            _ => return Err(()),
        };
        Ok(Self::new(
            f32::from(r) / 255.0,
            f32::from(g) / 255.0,
            f32::from(b) / 255.0,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::Srgb;

    #[test]
    fn from_hex_rrggbb_and_rgb() {
        let a = Srgb::from_hex("#0078D4").expect("hex");
        assert!((a.r - 0.0).abs() < 0.001);
        assert!((a.g - 120.0 / 255.0).abs() < 0.001);
        assert!((a.b - 212.0 / 255.0).abs() < 0.001);
        let b = Srgb::from_hex("#f00").expect("short");
        assert!((b.r - 1.0).abs() < 0.02);
    }
}
