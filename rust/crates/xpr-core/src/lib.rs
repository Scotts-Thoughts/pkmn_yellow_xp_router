//! Constants, configuration, filesystem helpers and the Python-compatible
//! JSON writer shared by every other crate of the router port.

pub mod config;
pub mod consts;
pub mod io_utils;
pub mod logging;
pub mod pyjson;
pub mod version;

pub use config::{CalcConfig, Config};
pub use consts::Paths;

/// Python's `math.floor(a / b)` for non-negative integer operands (exact).
#[inline]
pub fn floor_div(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

/// `math.floor(x * 1.1)` and friends computed exactly. `num`/`den` describe
/// the multiplier as a rational (11/10 for 1.1). The Python code multiplies a
/// (non-negative) integer by a binary float; the results are proven equal on
/// the golden corpus for every multiplier the engine uses.
#[inline]
pub fn floor_mul_ratio(x: i64, num: i64, den: i64) -> i64 {
    (x * num).div_euclid(den)
}

/// `min(math.ceil(math.sqrt(x)), 255)` via integer square root.
pub fn ceil_sqrt_capped(x: i64) -> i64 {
    if x <= 0 {
        return 0;
    }
    let mut r = (x as f64).sqrt() as i64;
    while r * r > x {
        r -= 1;
    }
    while (r + 1) * (r + 1) <= x {
        r += 1;
    }
    let c = if r * r == x { r } else { r + 1 };
    c.min(255)
}
