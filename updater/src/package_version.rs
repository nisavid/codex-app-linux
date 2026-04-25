//! Package-version parsing and comparison helpers for updater state decisions.

use std::cmp::Ordering;

pub fn installed_version_satisfies_candidate(installed: &str, candidate: &str) -> bool {
    if installed == "unknown" {
        return false;
    }

    match compare_package_versions(installed, candidate) {
        Some(Ordering::Less) => false,
        Some(_) => true,
        None => installed == candidate,
    }
}

pub fn compare_package_versions(left: &str, right: &str) -> Option<Ordering> {
    let left = parse_package_version(left)?;
    let right = parse_package_version(right)?;
    Some(left.cmp(&right))
}

/// Parses upstream app package versions for updater state comparisons.
///
/// Accepts three or four numeric segments such as `26.422.30944` or
/// `26.422.30944.2080`, strips package/build suffixes (`+...` and `-...`),
/// normalizes three-segment versions, and rejects timestamp-style legacy
/// majors (`>= 1000`).
fn parse_package_version(version: &str) -> Option<Vec<u32>> {
    let without_build_metadata = version
        .split_once('+')
        .map(|(prefix, _)| prefix)
        .unwrap_or(version)
        .trim();
    let base = without_build_metadata
        .split_once('-')
        .map(|(prefix, _)| prefix)
        .unwrap_or(without_build_metadata);
    let mut parts = Vec::new();
    for segment in base.split('.') {
        parts.push(segment.parse::<u32>().ok()?);
    }
    if !(3..=4).contains(&parts.len()) || parts.first().is_some_and(|major| *major >= 1000) {
        return None;
    }
    while parts.len() < 4 {
        parts.push(0);
    }
    Some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_versions_compare_by_numeric_segments() {
        assert_eq!(
            compare_package_versions("26.422.30944.2080", "26.422.30944.2079"),
            Some(Ordering::Greater)
        );
    }

    #[test]
    fn package_version_comparison_accepts_missing_build_segment() {
        assert_eq!(
            compare_package_versions("26.422.30944", "26.422.30944.0"),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn package_version_comparison_accepts_package_release_suffixes() {
        assert_eq!(
            compare_package_versions("26.422.30944.2080-1", "26.422.30944.2080"),
            Some(Ordering::Equal)
        );
    }

    #[test]
    fn package_version_comparison_rejects_legacy_timestamp_versions() {
        assert_eq!(
            compare_package_versions("2026.04.01.035152", "26.422.30944.2080"),
            None
        );
    }

    #[test]
    fn legacy_timestamp_installed_version_does_not_satisfy_upstream_candidate() {
        assert!(!installed_version_satisfies_candidate(
            "2026.04.01.035152+abcd1234",
            "26.422.30944.2080"
        ));
    }
}
