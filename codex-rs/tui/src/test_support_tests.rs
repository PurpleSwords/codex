use super::normalize_version;
use pretty_assertions::assert_eq;

#[test]
fn version_normalization_preserves_header_width_and_other_content() {
    let input = "│ >_ OpenAI Codex (v0.153.4)   │\n│ directory: /tmp/0.153.4     │\n";
    let expected = "│ >_ OpenAI Codex (v0.0.0)     │\n│ directory: /tmp/0.153.4     │\n";
    assert_eq!(normalize_version(input, "0.153.4"), expected);
}

#[test]
fn update_normalization_keeps_latest_version_and_border() {
    assert_eq!(
        normalize_version("│ ✨ Update available! 0.153.4 -> 0.153.4   │", "0.153.4"),
        "│ ✨ Update available! 0.0.0 -> 0.153.4     │"
    );
}

#[test]
fn shorter_versions_consume_padding_without_hiding_overflow() {
    assert_eq!(
        normalize_version("│ OpenAI Codex (v1.0)   │", "1.0"),
        "│ OpenAI Codex (v0.0.0) │"
    );
    assert_eq!(
        normalize_version("│ OpenAI Codex (v1.0)│", "1.0"),
        "│ OpenAI Codex (v1.0)│"
    );
}
