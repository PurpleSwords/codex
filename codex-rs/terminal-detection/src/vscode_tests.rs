use super::FakeEnvironment;
use super::terminal_info;
use crate::TerminalName;
use crate::detect_terminal_info_from_env;
use pretty_assertions::assert_eq;

#[test]
fn cursor_uses_vscode_compatibility_without_losing_metadata() {
    for program in ["cursor", "Cursor", "CURSOR"] {
        let env = FakeEnvironment::new()
            .with_var("TERM_PROGRAM", program)
            .with_var("TERM_PROGRAM_VERSION", "3.14.7")
            .with_var("WT_SESSION", "inherited-host");
        assert_eq!(
            detect_terminal_info_from_env(&env),
            terminal_info(
                TerminalName::VsCode,
                Some(program),
                Some("3.14.7"),
                /*term*/ None,
                /*multiplexer*/ None,
            )
        );
    }
}

#[test]
fn vscode_injection_takes_precedence_over_inherited_windows_terminal() {
    for signal in ["VSCODE_INJECTION", "VSCODE_IPC_HOOK_CLI"] {
        let env = FakeEnvironment::new()
            .with_var(signal, "present")
            .with_var("WT_SESSION", "inherited-host");
        assert_eq!(
            detect_terminal_info_from_env(&env),
            terminal_info(
                TerminalName::VsCode,
                /*term_program*/ None,
                /*version*/ None,
                /*term*/ None,
                /*multiplexer*/ None,
            )
        );
    }
}

#[test]
fn explicit_terminal_program_still_wins_over_inherited_injection() {
    let env = FakeEnvironment::new()
        .with_var("TERM_PROGRAM", "WezTerm")
        .with_var("VSCODE_INJECTION", "present")
        .with_var("WT_SESSION", "inherited-host");
    assert_eq!(
        detect_terminal_info_from_env(&env),
        terminal_info(
            TerminalName::WezTerm,
            Some("WezTerm"),
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None,
        )
    );
}

#[test]
fn empty_vscode_signals_do_not_override_windows_terminal() {
    let env = FakeEnvironment::new()
        .with_var("VSCODE_INJECTION", "")
        .with_var("VSCODE_IPC_HOOK_CLI", "  ")
        .with_var("WT_SESSION", "host");
    assert_eq!(
        detect_terminal_info_from_env(&env),
        terminal_info(
            TerminalName::WindowsTerminal,
            /*term_program*/ None,
            /*version*/ None,
            /*term*/ None,
            /*multiplexer*/ None,
        )
    );
}
