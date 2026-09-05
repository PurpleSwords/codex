//! Terminal keyboard enhancement setup and teardown helpers.
//!
//! The TUI uses crossterm's keyboard enhancement stack while it owns the terminal, but
//! process exit gets a stronger reset so the parent shell does not inherit enhanced key
//! reporting if a terminal misses the normal stack pop.

use std::fmt;
use std::io::stdout;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;

use codex_terminal_detection::TerminalName;
use codex_terminal_detection::terminal_info;
use crossterm::Command;
use crossterm::event::KeyboardEnhancementFlags;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::event::PushKeyboardEnhancementFlags;
use ratatui::crossterm::execute;

const DISABLE_KEYBOARD_ENHANCEMENT_ENV_VAR: &str = "CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT";
#[cfg(target_os = "linux")]
const WINDOWS_TERM_PROGRAM_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(target_os = "linux")]
const WINDOWS_TERM_PROGRAM_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) fn keyboard_enhancement_disabled() -> bool {
    let disable_env = std::env::var(DISABLE_KEYBOARD_ENHANCEMENT_ENV_VAR).ok();
    keyboard_enhancement_disabled_for(disable_env.as_deref(), || {
        running_in_wsl() && running_in_vscode_terminal()
    })
}

fn keyboard_enhancement_disabled_for(
    disable_env: Option<&str>,
    auto_detect: impl FnOnce() -> bool,
) -> bool {
    if let Some(disabled) = parse_bool_env(disable_env) {
        return disabled;
    }

    // VS Code running a WSL shell can hide TERM_PROGRAM from the Linux process
    // environment, so `running_in_vscode_terminal` also probes the Windows-side
    // environment through WSL interop.
    auto_detect()
}

fn parse_bool_env(value: Option<&str>) -> Option<bool> {
    match value.map(str::trim) {
        Some("1") => Some(true),
        Some(value) if value.eq_ignore_ascii_case("true") => Some(true),
        Some(value) if value.eq_ignore_ascii_case("yes") => Some(true),
        Some("0") => Some(false),
        Some(value) if value.eq_ignore_ascii_case("false") => Some(false),
        Some(value) if value.eq_ignore_ascii_case("no") => Some(false),
        _ => None,
    }
}

fn running_in_wsl() -> bool {
    #[cfg(target_os = "linux")]
    {
        crate::clipboard_paste::is_probably_wsl()
    }

    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub(super) fn running_in_vscode_terminal() -> bool {
    let term_program = std::env::var("TERM_PROGRAM").ok();
    vscode_terminal_detected(term_program.as_deref(), windows_term_program)
}

fn vscode_terminal_detected(
    linux_term_program: Option<&str>,
    windows_term_program: impl FnOnce() -> Option<String>,
) -> bool {
    term_program_is_vscode(linux_term_program)
        || term_program_is_vscode(windows_term_program().as_deref())
}

fn term_program_is_vscode(value: Option<&str>) -> bool {
    value.is_some_and(|value| value.eq_ignore_ascii_case("vscode"))
}

fn windows_term_program() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        static WINDOWS_TERM_PROGRAM: std::sync::OnceLock<Option<String>> =
            std::sync::OnceLock::new();
        WINDOWS_TERM_PROGRAM
            .get_or_init(read_windows_term_program)
            .clone()
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
fn read_windows_term_program() -> Option<String> {
    if !running_in_wsl() {
        return None;
    }
    let executable = codex_utils_path::system_executable("cmd.exe")?;
    let working_directory = executable.parent()?;
    let mut command = std::process::Command::new(&executable);
    command
        .args(["/d", "/s", "/c", "set TERM_PROGRAM"])
        .current_dir(working_directory)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    let output = run_command_with_timeout(&mut command, WINDOWS_TERM_PROGRAM_TIMEOUT).ok()??;

    if !output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| {
            line.trim_end_matches('\r')
                .strip_prefix("TERM_PROGRAM=")
                .map(str::to_string)
        })
        .filter(|value| !value.trim().is_empty())
}

#[cfg(target_os = "linux")]
fn run_command_with_timeout(
    command: &mut std::process::Command,
    timeout: Duration,
) -> std::io::Result<Option<std::process::Output>> {
    let mut child = command.stdout(std::process::Stdio::piped()).spawn()?;
    let deadline = Instant::now() + timeout;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => return child.wait_with_output().map(Some),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(WINDOWS_TERM_PROGRAM_POLL_INTERVAL);
            }
            Ok(None) => {
                child.kill()?;
                child.wait()?;
                return Ok(None);
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        }
    }
}

pub(super) fn enable_keyboard_enhancement() {
    if keyboard_enhancement_disabled() {
        return;
    }

    let running_in_tmux_session = running_in_tmux_session();
    let tmux_extended_keys_format = if running_in_tmux_session {
        read_tmux_extended_keys_format()
    } else {
        None
    };

    let _ = execute!(
        stdout(),
        DisableModifyOtherKeys,
        PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
            terminal_info().name,
            running_in_tmux_session,
            tmux_extended_keys_format.as_deref()
        ))
    );

    if tmux_should_enable_modify_other_keys_for(
        running_in_tmux_session,
        tmux_extended_keys_format.as_deref(),
    ) {
        let _ = execute!(stdout(), EnableModifyOtherKeys);
    }
}

fn keyboard_enhancement_flags(
    terminal_name: TerminalName,
    running_in_tmux_session: bool,
    tmux_extended_keys_format: Option<&str>,
) -> KeyboardEnhancementFlags {
    let flags = KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
        | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS;

    // iTerm and Ghostty can leak shortcut release events that the terminal consumes.
    // tmux's xterm key format also loses Shift-Enter when event types are
    // reported. An unavailable/unrecognized tmux probe must take the same safe
    // fallback. Preserve repeat classification on confirmed csi-u transports.
    if matches!(terminal_name, TerminalName::Ghostty | TerminalName::Iterm2)
        || (running_in_tmux_session && !matches!(tmux_extended_keys_format, Some("csi-u")))
    {
        flags
    } else {
        flags | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
    }
}

fn running_in_tmux_session() -> bool {
    tmux_session_detected(
        std::env::var("TMUX").ok().as_deref(),
        std::env::var("TMUX_PANE").ok().as_deref(),
    )
}

fn tmux_session_detected(tmux: Option<&str>, tmux_pane: Option<&str>) -> bool {
    tmux.is_some() || tmux_pane.is_some()
}

fn tmux_should_enable_modify_other_keys_for(
    running_in_tmux_session: bool,
    extended_keys_format: Option<&str>,
) -> bool {
    // Only request mode 2 when tmux confirms csi-u formatting. Older tmux
    // versions do not expose this option and may emit xterm-style sequences,
    // which crossterm does not parse consistently for modified keys.
    running_in_tmux_session && matches!(extended_keys_format, Some("csi-u"))
}

fn read_tmux_extended_keys_format() -> Option<String> {
    let executable = codex_utils_path::system_executable("tmux")?;
    let path = codex_utils_path::system_path().ok()?;
    for args in [
        ["display-message", "-p", "#{extended-keys-format}"],
        ["show-options", "-gqv", "extended-keys-format"],
    ] {
        let output = std::process::Command::new(&executable)
            .env("PATH", &path)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;

        if !output.status.success() {
            continue;
        }

        if let Some(value) = String::from_utf8(output.stdout)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        {
            return Some(value);
        }
    }

    None
}

pub(super) fn restore_keyboard_enhancement_stack() {
    let _ = execute!(
        stdout(),
        PopKeyboardEnhancementFlags,
        DisableModifyOtherKeys
    );
}

pub(super) fn reset_keyboard_reporting_after_exit() {
    let _ = execute!(
        stdout(),
        PopKeyboardEnhancementFlags,
        ResetKeyboardEnhancementFlags,
        DisableModifyOtherKeys
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResetKeyboardEnhancementFlags;

impl Command for ResetKeyboardEnhancementFlags {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[<u")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "keyboard enhancement reset is not implemented for the legacy Windows API",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EnableModifyOtherKeys;

impl Command for EnableModifyOtherKeys {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[>4;2m")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "modifyOtherKeys enable is not implemented for the legacy Windows API",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DisableModifyOtherKeys;

impl Command for DisableModifyOtherKeys {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("\x1b[>4;0m")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "modifyOtherKeys reset is not implemented for the legacy Windows API",
        ))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::DisableModifyOtherKeys;
    use super::EnableModifyOtherKeys;
    use super::ResetKeyboardEnhancementFlags;
    use super::keyboard_enhancement_disabled_for;
    use super::keyboard_enhancement_flags;
    use super::parse_bool_env;
    #[cfg(target_os = "linux")]
    use super::run_command_with_timeout;
    use super::tmux_session_detected;
    use super::tmux_should_enable_modify_other_keys_for;
    use super::vscode_terminal_detected;
    use codex_terminal_detection::TerminalName;
    use crossterm::Command;
    use crossterm::event::PushKeyboardEnhancementFlags;
    use pretty_assertions::assert_eq;

    fn ansi_for(command: impl Command) -> String {
        let mut out = String::new();
        command.write_ansi(&mut out).unwrap();
        out
    }

    #[test]
    fn keyboard_enhancement_suppresses_release_reporting_for_iterm() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Iterm2,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_suppresses_release_reporting_for_ghostty() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Ghostty,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_repeat_reporting_for_kitty() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Kitty,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>7u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_repeat_reporting_for_csi_u_tmux() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Kitty,
                /*running_in_tmux_session*/ true,
                Some("csi-u")
            ))),
            "\x1b[>7u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_shift_enter_for_xterm_tmux() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Kitty,
                /*running_in_tmux_session*/ true,
                Some("xterm")
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_preserves_repeat_reporting_for_unknown_terminals() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Unknown,
                /*running_in_tmux_session*/ false,
                /*tmux_extended_keys_format*/ None
            ))),
            "\x1b[>7u"
        );
    }

    #[test]
    fn keyboard_enhancement_uses_conservative_flags_when_tmux_format_is_unknown() {
        assert_eq!(
            ansi_for(PushKeyboardEnhancementFlags(keyboard_enhancement_flags(
                TerminalName::Unknown,
                /*running_in_tmux_session*/ true,
                /*tmux_extended_keys_format*/ None,
            ))),
            "\x1b[>5u"
        );
    }

    #[test]
    fn keyboard_enhancement_env_flag_parses_common_values() {
        assert_eq!(parse_bool_env(Some("1")), Some(true));
        assert_eq!(parse_bool_env(Some("true")), Some(true));
        assert_eq!(parse_bool_env(Some("YES")), Some(true));
        assert_eq!(parse_bool_env(Some("0")), Some(false));
        assert_eq!(parse_bool_env(Some("false")), Some(false));
        assert_eq!(parse_bool_env(Some("NO")), Some(false));
        assert_eq!(parse_bool_env(Some("unexpected")), None);
        assert_eq!(parse_bool_env(/*value*/ None), None);
    }

    #[test]
    fn keyboard_enhancement_auto_disables_for_vscode_in_wsl() {
        assert!(keyboard_enhancement_disabled_for(
            /*disable_env*/ None,
            || true
        ));
    }

    #[test]
    fn keyboard_enhancement_auto_disable_requires_wsl_and_vscode() {
        assert!(!keyboard_enhancement_disabled_for(
            /*disable_env*/ None,
            || false
        ));
    }

    #[test]
    fn keyboard_enhancement_env_flag_skips_auto_detection() {
        assert!(!keyboard_enhancement_disabled_for(Some("0"), || panic!(
            "explicit enable should skip auto detection"
        )));
        assert!(keyboard_enhancement_disabled_for(Some("1"), || panic!(
            "explicit disable should skip auto detection"
        )));
    }

    #[test]
    fn vscode_terminal_detection_uses_linux_and_windows_term_program() {
        assert!(vscode_terminal_detected(Some("vscode"), || panic!(
            "Linux TERM_PROGRAM should skip the Windows probe"
        )));
        assert!(vscode_terminal_detected(
            /*linux_term_program*/ None,
            || Some("vscode".to_string())
        ));
        assert!(!vscode_terminal_detected(
            /*linux_term_program*/ None,
            || Some("WindowsTerminal".to_string())
        ));
        assert!(!vscode_terminal_detected(
            /*linux_term_program*/ None,
            || None
        ));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn command_timeout_terminates_a_hanging_process() {
        let executable = codex_utils_path::system_executable("sleep")
            .expect("sleep should be available in a system directory");
        let mut command = std::process::Command::new(executable);
        command
            .arg("30")
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        let start = std::time::Instant::now();

        let output = run_command_with_timeout(&mut command, std::time::Duration::from_millis(20))
            .expect("timeout helper should terminate and reap the process");

        assert!(output.is_none());
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }

    #[test]
    fn tmux_session_detection_accepts_tmux_or_tmux_pane() {
        assert!(tmux_session_detected(
            Some("/tmp/tmux-501/default,1,0"),
            /*tmux_pane*/ None
        ));
        assert!(tmux_session_detected(/*tmux*/ None, Some("%0")));
        assert!(!tmux_session_detected(
            /*tmux*/ None, /*tmux_pane*/ None
        ));
    }

    #[test]
    fn tmux_modify_other_keys_only_requests_confirmed_csi_u_format() {
        assert!(tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true,
            Some("csi-u")
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true, /*extended_keys_format*/ None
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true,
            Some("xterm")
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ true,
            Some("")
        ));
        assert!(!tmux_should_enable_modify_other_keys_for(
            /*running_in_tmux_session*/ false,
            Some("csi-u")
        ));
    }

    #[test]
    fn reset_keyboard_enhancement_flags_clears_all_pushed_levels() {
        assert_eq!(ansi_for(ResetKeyboardEnhancementFlags), "\x1b[<u");
    }

    #[test]
    fn enable_modify_other_keys_requests_xterm_keyboard_reporting() {
        assert_eq!(ansi_for(EnableModifyOtherKeys), "\x1b[>4;2m");
    }

    #[test]
    fn disable_modify_other_keys_resets_xterm_keyboard_reporting() {
        assert_eq!(ansi_for(DisableModifyOtherKeys), "\x1b[>4;0m");
    }
}
