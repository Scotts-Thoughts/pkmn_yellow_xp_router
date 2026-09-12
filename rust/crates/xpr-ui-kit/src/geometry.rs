//! The tkinter-format window geometry string the config stores
//! (`"WxH+X+Y"`, negative offsets written as `-X`) and the window state
//! (`"zoomed"` / `"normal"` / `"iconic"`).

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub width: i32,
    pub height: i32,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

impl Geometry {
    /// `_restore_geometry`'s parse: `"2000x1200+100+50"`; returns `None` on
    /// anything malformed (the window then uses the 2000x1200 default).
    pub fn parse(geo: &str) -> Option<Geometry> {
        let geo = geo.trim();
        if geo.is_empty() {
            return None;
        }
        let spaced = geo.replace('+', " +").replace('-', " -");
        let parts: Vec<&str> = spaced.split_whitespace().collect();
        let wh: Vec<&str> = parts.first()?.split('x').collect();
        if wh.len() != 2 {
            return None;
        }
        let width: i32 = wh[0].parse().ok()?;
        let height: i32 = wh[1].parse().ok()?;
        let (x, y) = if parts.len() >= 3 {
            (Some(parts[1].parse::<i32>().ok()?), Some(parts[2].parse::<i32>().ok()?))
        } else {
            (None, None)
        };
        Some(Geometry { width, height, x, y })
    }

    /// `_save_geometry`'s format (`f"{w}x{h}+{x}+{y}"`).
    pub fn format(&self) -> String {
        format!("{}x{}+{}+{}", self.width, self.height, self.x.unwrap_or(0), self.y.unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let g = Geometry::parse("2000x1200+100+50").unwrap();
        assert_eq!(g, Geometry { width: 2000, height: 1200, x: Some(100), y: Some(50) });
        assert_eq!(g.format(), "2000x1200+100+50");
        let n = Geometry::parse("800x600-10-20").unwrap();
        assert_eq!((n.x, n.y), (Some(-10), Some(-20)));
        assert_eq!(n.format(), "800x600+-10+-20");
        assert!(Geometry::parse("garbage").is_none());
        assert_eq!(Geometry::parse("640x480").unwrap().x, None);
    }
}
