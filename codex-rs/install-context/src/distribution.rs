//! Distribution identity is separate from the native upstream/protocol version.

use semver::Version;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Distribution {
    Official,
    Fork,
}

impl Distribution {
    pub fn current() -> Self {
        match std::env::var("CODEX_NPM_PACKAGE_NAME").as_deref() {
            Ok("@purplesword/codex") => Self::Fork,
            Ok("@openai/codex") => Self::Official,
            _ if option_env!("CODEX_FORK_VERSION").is_some() => Self::Fork,
            _ => Self::Official,
        }
    }

    pub fn npm_package(self) -> &'static str {
        match self {
            Self::Official => "@openai/codex",
            Self::Fork => "@purplesword/codex",
        }
    }

    pub fn registry_url(self) -> &'static str {
        match self {
            Self::Official => "https://registry.npmjs.org/@openai%2fcodex",
            Self::Fork => "https://registry.npmjs.org/@purplesword%2fcodex",
        }
    }

    pub fn release_notes_url(self) -> &'static str {
        match self {
            Self::Official => "https://github.com/openai/codex/releases/latest",
            Self::Fork => "https://github.com/PurpleSwords/codex/releases",
        }
    }

    pub fn repository_url(self) -> &'static str {
        match self {
            Self::Official => "https://github.com/openai/codex",
            Self::Fork => "https://github.com/PurpleSwords/codex",
        }
    }

    pub fn version_filename(self) -> &'static str {
        match self {
            Self::Official => "version.json",
            Self::Fork => "version.purplesword.json",
        }
    }

    pub fn current_version(self, native_version: &str) -> Option<String> {
        let npm_version = std::env::var("CODEX_NPM_PACKAGE_VERSION").ok();
        self.resolve_version(
            native_version,
            npm_version.as_deref().or(option_env!("CODEX_FORK_VERSION")),
        )
    }

    fn resolve_version(self, native_version: &str, npm_version: Option<&str>) -> Option<String> {
        if self == Self::Official {
            return Some(native_version.to_string());
        }
        let version = npm_version.unwrap_or(native_version);
        if version == "0.153.4" && native_version == version {
            return Some(version.to_string());
        }
        let (major, minor, patch, _) = fork_version_key(version)?;
        (format!("{major}.{minor}.{patch}") == native_version).then(|| version.to_string())
    }
}

/// Fork-only precedence. The historical bare 0.153.4 is revision zero, not a
/// general exception to SemVer precedence for official packages or other versions.
pub fn is_newer_fork_release(latest: &str, current: &str) -> Option<bool> {
    let current_key = if current == "0.153.4" {
        (0, 153, 4, 0)
    } else {
        fork_version_key(current)?
    };
    Some(fork_version_key(latest)? > current_key)
}

fn fork_version_key(value: &str) -> Option<(u64, u64, u64, u64)> {
    let version = Version::parse(value).ok()?;
    let revision = version.pre.as_str().strip_prefix("fork.")?;
    let number = revision.parse::<u64>().ok()?;
    if number == 0 || number.to_string() != revision || !version.build.is_empty() {
        return None;
    }
    Some((version.major, version.minor, version.patch, number))
}

#[cfg(test)]
#[path = "distribution_tests.rs"]
mod tests;
