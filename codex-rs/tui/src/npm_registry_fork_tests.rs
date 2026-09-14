use super::*;
use pretty_assertions::assert_eq;

fn complete_package() -> NpmPackageInfo {
    let version = "0.153.4-fork.2";
    let dist = serde_json::json!({"tarball": "https://registry.npmjs.org/package.tgz", "integrity": "sha512-example"});
    let mut versions = serde_json::Map::new();
    let mut dependencies = serde_json::Map::new();
    for platform in [
        "linux-x64",
        "linux-arm64",
        "darwin-x64",
        "darwin-arm64",
        "win32-x64",
        "win32-arm64",
    ] {
        let payload = format!("{version}-{platform}");
        dependencies.insert(
            format!("@purplesword/codex-{platform}"),
            serde_json::json!(format!("npm:@purplesword/codex@{payload}")),
        );
        versions.insert(payload, serde_json::json!({"dist": dist.clone()}));
    }
    versions.insert(
        version.to_string(),
        serde_json::json!({"dist": dist, "optionalDependencies": dependencies}),
    );
    serde_json::from_value(
        serde_json::json!({"dist-tags": {"latest": version}, "versions": versions}),
    )
    .unwrap()
}

#[test]
fn fork_update_requires_all_platforms() {
    let complete = complete_package();
    assert_eq!(ready_fork_version(&complete).unwrap(), "0.153.4-fork.2");
    for version in complete.versions.keys() {
        let mut incomplete = complete.clone();
        incomplete.versions.remove(version);
        assert!(
            ready_fork_version(&incomplete).is_err(),
            "missing {version}"
        );
    }
}

#[test]
fn fork_update_rejects_wrong_alias_and_missing_integrity() {
    let mut info = complete_package();
    info.versions
        .get_mut("0.153.4-fork.2")
        .unwrap()
        .optional_dependencies
        .insert(
            "@purplesword/codex-linux-x64".to_string(),
            "npm:@openai/codex@0.153.4".to_string(),
        );
    assert!(ready_fork_version(&info).is_err());
    let mut info = complete_package();
    info.versions
        .get_mut("0.153.4-fork.2-linux-x64")
        .unwrap()
        .dist
        .as_mut()
        .unwrap()
        .integrity = None;
    assert!(ready_fork_version(&info).is_err());
}

#[test]
fn fork_update_rejects_platform_latest_tag() {
    let mut info = complete_package();
    info.dist_tags
        .insert("latest".to_string(), "0.153.4-fork.2-linux-x64".to_string());
    assert!(ready_fork_version(&info).is_err());
}
