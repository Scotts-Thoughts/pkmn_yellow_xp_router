//! Version string parsing from `utils/auto_update.py` (`vX.Ya`: major,
//! minor, and the last character as a bug-fix letter).

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VersionInfo {
    pub major: i64,
    pub minor: i64,
    pub bugfix: char,
}

/// `_extract_version_info`
pub fn extract_version_info(version_string: &str) -> Option<VersionInfo> {
    let parts: Vec<&str> = version_string.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    let digits = |s: &str| -> Option<i64> {
        let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
        d.parse::<i64>().ok()
    };
    let major = digits(parts[0])?;
    let minor = digits(parts[1])?;
    let bugfix = parts[1].chars().last()?;
    Some(VersionInfo { major, minor, bugfix })
}

/// `_get_newer_version`
pub fn get_newer_version<'a>(v_one: &'a str, v_two: &'a str) -> &'a str {
    let one = extract_version_info(v_one);
    let two = extract_version_info(v_two);
    match (one, two) {
        (None, None) => v_one,
        (None, Some(_)) => v_two,
        (Some(_), None) => v_one,
        (Some(a), Some(b)) => {
            if a.major > b.major {
                v_one
            } else if b.major > a.major {
                v_two
            } else if a.minor > b.minor {
                v_one
            } else if b.minor > a.minor {
                v_two
            } else if a.bugfix > b.bugfix {
                v_one
            } else {
                v_two
            }
        }
    }
}

/// `is_upgrade_needed`
pub fn is_upgrade_needed(new_version: Option<&str>, old_version: &str) -> bool {
    match new_version {
        None => false,
        Some(nv) => nv != crate::consts::APP_VERSION && nv == get_newer_version(nv, old_version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let v = extract_version_info("v5.0a").unwrap();
        assert_eq!(v.major, 5);
        assert_eq!(v.minor, 0);
        assert_eq!(v.bugfix, 'a');
        assert_eq!(get_newer_version("v5.0a", "v5.0b"), "v5.0b");
        assert_eq!(get_newer_version("v6.0a", "v5.9z"), "v6.0a");
        assert!(extract_version_info("garbage").is_none());
    }
}
