#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum CliAction {
    RunTui,
    PrintVersion,
    PrintNonInteractiveNotice,
}

pub fn decide<I, S>(args: I, stdin_is_terminal: bool, stdout_is_terminal: bool) -> CliAction
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if args
        .into_iter()
        .any(|arg| matches!(arg.as_ref(), "-V" | "--version"))
    {
        return CliAction::PrintVersion;
    }

    if stdin_is_terminal && stdout_is_terminal {
        CliAction::RunTui
    } else {
        CliAction::PrintNonInteractiveNotice
    }
}

pub fn version_line() -> String {
    format!("skillroom {}", env!("CARGO_PKG_VERSION"))
}

pub fn non_interactive_notice() -> &'static str {
    "skillroom: interactive TUI requires stdin and stdout to be terminals"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_flag_wins_without_tty() {
        assert_eq!(decide(["--version"], false, false), CliAction::PrintVersion);
        assert_eq!(decide(["-V"], false, false), CliAction::PrintVersion);
    }

    #[test]
    fn non_interactive_without_version_does_not_start_tui() {
        assert_eq!(
            decide(Vec::<String>::new(), false, true),
            CliAction::PrintNonInteractiveNotice
        );
        assert_eq!(
            decide(Vec::<String>::new(), true, false),
            CliAction::PrintNonInteractiveNotice
        );
    }

    #[test]
    fn interactive_terminal_runs_tui() {
        assert_eq!(decide(Vec::<String>::new(), true, true), CliAction::RunTui);
    }
}
