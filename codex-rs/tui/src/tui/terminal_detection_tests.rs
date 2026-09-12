use super::vscode_terminal_detected;
use codex_terminal_detection::TerminalName;
use pretty_assertions::assert_eq;

#[test]
fn known_terminals_never_invoke_the_windows_probe() {
    for terminal in [
        TerminalName::VsCode,
        TerminalName::WindowsTerminal,
        TerminalName::AppleTerminal,
        TerminalName::Ghostty,
        TerminalName::Iterm2,
        TerminalName::WarpTerminal,
        TerminalName::WezTerm,
        TerminalName::Kitty,
        TerminalName::Alacritty,
        TerminalName::Konsole,
        TerminalName::GnomeTerminal,
        TerminalName::Vte,
        TerminalName::Dumb,
    ] {
        assert_eq!(
            vscode_terminal_detected(terminal, || panic!("known terminal must skip CMD")),
            terminal == TerminalName::VsCode,
        );
    }
}

#[test]
fn unknown_terminal_preserves_the_windows_compatibility_fallback() {
    for (program, expected) in [
        ("vscode", true),
        ("Cursor", true),
        ("WindowsTerminal", false),
    ] {
        let mut calls = 0;
        assert_eq!(
            vscode_terminal_detected(TerminalName::Unknown, || {
                calls += 1;
                Some(program.to_string())
            }),
            expected,
        );
        assert_eq!(calls, 1);
    }
}
