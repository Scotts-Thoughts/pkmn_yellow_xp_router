//! Presentational formatting helpers for the Misc calculators (Solodex
//! `misc/format.ts`). Pure, never NaN.

/// JavaScript's `Number.prototype.toFixed` for ordinary magnitudes: the
/// decimal expansion of the exact binary value, rounded half up (Rust's
/// formatter rounds ties to even: `3.125` is "3.13" in JS, "3.12" here).
pub fn to_fixed(x: f64, decimals: usize) -> String {
    if !x.is_finite() {
        return if x.is_nan() {
            "NaN".to_string()
        } else if x > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        };
    }
    let negative = x < 0.0;
    // exact digits, with room to look at the digit after the last kept one
    let exact = format!("{:.*}", decimals + 30, x.abs());
    let (int_part, frac_part) = exact.split_once('.').unwrap_or((exact.as_str(), ""));
    let mut digits: Vec<u8> = int_part
        .bytes()
        .chain(frac_part.bytes().take(decimals))
        .collect();
    let next = frac_part.as_bytes().get(decimals).copied().unwrap_or(b'0');
    if next >= b'5' {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let int_len = digits.len() - decimals;
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(std::str::from_utf8(&digits[..int_len]).unwrap_or("0"));
    if decimals > 0 {
        out.push('.');
        out.push_str(std::str::from_utf8(&digits[int_len..]).unwrap_or(""));
    }
    out
}

/// Format a probability (0-1) as a percentage string.
pub fn format_percent(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "0%".to_string();
    }
    let pct = value * 100.0;
    if pct <= 0.0 {
        return "0%".to_string();
    }
    if pct >= 100.0 {
        return "100%".to_string();
    }
    if pct < 0.01 {
        return "<0.01%".to_string();
    }
    if pct > 99.99 {
        return ">99.99%".to_string();
    }
    format!("{}%", to_fixed(pct, decimals))
}

/// [`format_percent`] with Solodex's default of two decimals.
pub fn percent(value: f64) -> String {
    format_percent(value, 2)
}

/// Format a plain number to at most `decimals` places, trimming trailing zeros
/// (`Number(value.toFixed(decimals)).toString()`).
pub fn format_number(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    let fixed = to_fixed(value, decimals);
    let n: f64 = fixed.parse().unwrap_or(0.0);
    // JS prints -0 as "0"
    if n == 0.0 {
        "0".to_string()
    } else {
        format!("{}", n)
    }
}

/// [`format_number`] with Solodex's default of two decimals.
pub fn number(value: f64) -> String {
    format_number(value, 2)
}

/// `String(value)` for the calculators' inputs (whole numbers without ".0").
pub fn js_string(value: f64) -> String {
    if value == 0.0 {
        "0".to_string()
    } else {
        format!("{}", value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_fixed_matches_javascript() {
        // exact ties: JS rounds up, Rust's {:.2} rounds to even
        assert_eq!(to_fixed(3.125, 2), "3.13");
        assert_eq!(to_fixed(0.125, 2), "0.13");
        assert_eq!(to_fixed(0.375, 2), "0.38");
        assert_eq!(to_fixed(2.5, 0), "3");
        assert_eq!(to_fixed(0.5, 0), "1");
        // 1.005 is really 1.00499999999999989...
        assert_eq!(to_fixed(1.005, 2), "1.00");
        assert_eq!(to_fixed(1.0, 2), "1.00");
        assert_eq!(to_fixed(12.5, 2), "12.50");
        assert_eq!(to_fixed(99.994, 2), "99.99");
        assert_eq!(to_fixed(0.0, 2), "0.00");
        // the carry runs through every digit
        assert_eq!(to_fixed(999.999, 2), "1000.00");
        assert_eq!(to_fixed(9.996, 2), "10.00");
        assert_eq!(to_fixed(-3.125, 2), "-3.13");
        assert_eq!(to_fixed(3.5, 3), "3.500");
        assert_eq!(to_fixed(7.0, 0), "7");
    }

    #[test]
    fn percent_edge_cases() {
        assert_eq!(percent(0.0), "0%");
        assert_eq!(percent(-1.0), "0%");
        assert_eq!(percent(f64::NAN), "0%");
        assert_eq!(percent(f64::INFINITY), "0%");
        assert_eq!(percent(1.0), "100%");
        assert_eq!(percent(1.5), "100%");
        assert_eq!(percent(0.00005), "<0.01%");
        assert_eq!(percent(0.99995), ">99.99%");
        assert_eq!(percent(0.0001), "0.01%");
        assert_eq!(percent(0.5), "50.00%");
        // 5 uses at 50%: P(X = 0) = 3.125% (an exact tie)
        assert_eq!(percent(0.03125), "3.13%");
        assert_eq!(percent(1.0 / 3.0), "33.33%");
        assert_eq!(format_percent(1.0 / 3.0, 0), "33%");
        assert_eq!(format_percent(0.5, 1), "50.0%");
    }

    #[test]
    fn number_trims_zeros() {
        assert_eq!(number(3.5), "3.5");
        assert_eq!(number(3.0), "3");
        assert_eq!(number(3.1), "3.1");
        assert_eq!(number(4.4999), "4.5");
        assert_eq!(number(0.0), "0");
        assert_eq!(number(-0.001), "0");
        assert_eq!(number(f64::NAN), "0");
        assert_eq!(number(1.0 / 3.0), "0.33");
        assert_eq!(format_number(1.0 / 3.0, 3), "0.333");
        assert_eq!(number(2.5_f64.sqrt()), "1.58");
        assert_eq!(number(0.9 * 3.1), "2.79");
    }

    #[test]
    fn js_string_of_inputs() {
        assert_eq!(js_string(70.0), "70");
        assert_eq!(js_string(70.5), "70.5");
        assert_eq!(js_string(0.0), "0");
        assert_eq!(js_string(-0.0), "0");
    }
}
