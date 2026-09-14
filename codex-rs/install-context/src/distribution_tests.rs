use super::*;
use pretty_assertions::assert_eq;

#[test]
fn release_identity_keeps_native_version_separate() {
    for (distribution, native, npm, expected) in [
        (
            Distribution::Official,
            "0.153.4",
            Some("0.153.4-fork.9"),
            Some("0.153.4"),
        ),
        (
            Distribution::Fork,
            "0.153.4",
            Some("0.153.4-fork.9"),
            Some("0.153.4-fork.9"),
        ),
        (Distribution::Fork, "0.153.4", None, Some("0.153.4")),
        (Distribution::Fork, "0.154.0", None, None),
        (Distribution::Fork, "0.153.4", Some("0.154.0-fork.1"), None),
        (
            Distribution::Fork,
            "0.153.4",
            Some("0.153.4-fork.1-linux-x64"),
            None,
        ),
    ] {
        assert_eq!(
            distribution.resolve_version(native, npm).as_deref(),
            expected
        );
    }
}

#[test]
fn fork_order_handles_numeric_revisions_and_legacy_migration() {
    for (latest, current, expected) in [
        ("0.153.4-fork.10", "0.153.4-fork.2", Some(true)),
        ("0.153.4-fork.2", "0.153.4-fork.10", Some(false)),
        ("0.154.0-fork.1", "0.153.4-fork.99", Some(true)),
        ("0.153.4-fork.99", "0.154.0-fork.1", Some(false)),
        ("0.153.4-fork.1", "0.153.4-fork.1", Some(false)),
        ("0.153.4-fork.1", "0.153.4", Some(true)),
        ("0.153.3-fork.1", "0.153.4", Some(false)),
        ("0.153.4", "0.153.4-fork.1", None),
        ("0.153.4-fork.1", "0.154.0", None),
    ] {
        assert_eq!(is_newer_fork_release(latest, current), expected);
    }
}

#[test]
fn reject_non_root_and_ambiguous_versions() {
    for value in [
        "0.153.4",
        "0.153.4-fork.0",
        "0.153.4-fork.01",
        "0.153.4-fork.1+build",
        "0.153.4-fork.1-linux-x64",
        "0.153.4-fork.1.rc.1",
        "0.153.4.1",
        "0.153.4-fork.18446744073709551616",
    ] {
        assert_eq!(is_newer_fork_release(value, "0.153.4"), None, "{value}");
    }
}
