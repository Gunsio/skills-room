use std::{
    env, fmt, io,
    path::{Path, PathBuf},
    process::Command,
};

use crate::source::{SourceCheck, SourceCheckKind, SourceCheckStatus};

pub const AGENTBUDDY_BIN: &str = "agentbuddy";
pub const AGENTBUDDY_PINNED_VERSION: &str = "0.4.0";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AgentBuddyCli {
    bin: String,
    pinned_version: String,
}

impl Default for AgentBuddyCli {
    fn default() -> Self {
        Self {
            bin: AGENTBUDDY_BIN.to_string(),
            pinned_version: AGENTBUDDY_PINNED_VERSION.to_string(),
        }
    }
}

impl AgentBuddyCli {
    pub fn new(bin: impl Into<String>, pinned_version: impl Into<String>) -> Self {
        Self {
            bin: bin.into(),
            pinned_version: pinned_version.into(),
        }
    }

    pub fn detect<R: CommandRunner>(&self, runner: &R) -> AgentBuddyCliReport {
        self.detect_with_path(runner, resolve_executable(&self.bin))
    }

    pub fn detect_with_path<R: CommandRunner>(
        &self,
        runner: &R,
        path: Option<PathBuf>,
    ) -> AgentBuddyCliReport {
        let mut checks = Vec::new();

        if path.is_none() {
            checks.push(SourceCheck::fail(
                SourceCheckKind::Cli,
                format!("{} is not on PATH", self.bin),
            ));
            checks.push(SourceCheck::skipped(
                SourceCheckKind::Auth,
                "auth check skipped because CLI is missing",
            ));
            return AgentBuddyCliReport {
                bin: self.bin.clone(),
                path,
                version: None,
                checks,
            };
        }

        let version = match runner.run(&self.bin, &["--version"]) {
            Ok(output) if output.status == 0 => parse_agentbuddy_version(&output.stdout),
            Ok(output) => {
                checks.push(SourceCheck::fail(
                    SourceCheckKind::Cli,
                    format!("version check exited {}", output.status),
                ));
                parse_agentbuddy_version(&output.stderr)
            }
            Err(error) => {
                checks.push(SourceCheck::fail(
                    SourceCheckKind::Cli,
                    format!("version check failed: {error}"),
                ));
                None
            }
        };

        match version.as_deref() {
            Some(found) if found == self.pinned_version => checks.push(SourceCheck::pass(
                SourceCheckKind::Cli,
                format!("agentbuddy {found}"),
            )),
            Some(found) => checks.push(SourceCheck::warn(
                SourceCheckKind::Cli,
                format!("agentbuddy {found}; expected {}", self.pinned_version),
            )),
            None => checks.push(SourceCheck::fail(
                SourceCheckKind::Cli,
                "unable to parse agentbuddy version",
            )),
        }

        checks.push(self.auth_check(runner));

        AgentBuddyCliReport {
            bin: self.bin.clone(),
            path,
            version,
            checks,
        }
    }

    fn auth_check<R: CommandRunner>(&self, runner: &R) -> SourceCheck {
        match runner.run(&self.bin, &["get-jwt"]) {
            Ok(output) if output.status == 0 && !output.stdout.trim().is_empty() => {
                SourceCheck::pass(SourceCheckKind::Auth, "agentbuddy jwt available")
            }
            Ok(output) if output.status == 0 => {
                SourceCheck::fail(SourceCheckKind::Auth, "agentbuddy jwt is empty")
            }
            Ok(output) => SourceCheck::fail(
                SourceCheckKind::Auth,
                format!("agentbuddy auth failed with exit {}", output.status),
            ),
            Err(error) => SourceCheck::fail(
                SourceCheckKind::Auth,
                format!("agentbuddy auth check failed: {error}"),
            ),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct AgentBuddyCliReport {
    pub bin: String,
    pub path: Option<PathBuf>,
    pub version: Option<String>,
    pub checks: Vec<SourceCheck>,
}

impl AgentBuddyCliReport {
    pub fn status(&self) -> SourceCheckStatus {
        if self
            .checks
            .iter()
            .any(|check| check.status == SourceCheckStatus::Fail)
        {
            SourceCheckStatus::Fail
        } else if self
            .checks
            .iter()
            .any(|check| check.status == SourceCheckStatus::Warn)
        {
            SourceCheckStatus::Warn
        } else {
            SourceCheckStatus::Pass
        }
    }
}

pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, CommandRunError>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug)]
pub struct CommandRunError {
    source: io::Error,
}

impl fmt::Display for CommandRunError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.source)
    }
}

impl std::error::Error for CommandRunError {}

#[derive(Debug, Default)]
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, CommandRunError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|source| CommandRunError { source })?;
        Ok(CommandOutput {
            status: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}

pub fn resolve_executable(program: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| resolve_executable_from_paths(program, &paths))
}

fn resolve_executable_from_paths(program: &str, paths: &std::ffi::OsStr) -> Option<PathBuf> {
    env::split_paths(paths)
        .map(|path| path.join(program))
        .find(|path| path.is_file() && is_executable(path))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

fn parse_agentbuddy_version(output: &str) -> Option<String> {
    output
        .split_whitespace()
        .find(|part| {
            part.trim_start_matches('v')
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        })
        .map(|value| value.trim_start_matches('v').to_string())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::VecDeque, fs};

    use tempfile::tempdir;

    use super::*;

    #[derive(Debug)]
    struct MockRunner {
        outputs: RefCell<VecDeque<CommandOutput>>,
    }

    impl MockRunner {
        fn new(outputs: Vec<CommandOutput>) -> Self {
            Self {
                outputs: RefCell::new(outputs.into()),
            }
        }
    }

    impl CommandRunner for MockRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Result<CommandOutput, CommandRunError> {
            Ok(self
                .outputs
                .borrow_mut()
                .pop_front()
                .unwrap_or_else(|| output(0, "", "")))
        }
    }

    fn output(status: i32, stdout: &str, stderr: &str) -> CommandOutput {
        CommandOutput {
            status,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
        }
    }

    #[test]
    fn parses_agentbuddy_version_from_common_outputs() {
        assert_eq!(
            parse_agentbuddy_version("agentbuddy 0.4.0\n"),
            Some("0.4.0".to_string())
        );
        assert_eq!(
            parse_agentbuddy_version("v0.4.0\n"),
            Some("0.4.0".to_string())
        );
    }

    #[test]
    fn resolves_executable_from_path_entries() {
        let temp = tempdir().unwrap();
        let bin = temp.path().join("agentbuddy");
        fs::write(&bin, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&bin).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&bin, permissions).unwrap();
        }

        assert_eq!(
            resolve_executable_from_paths("agentbuddy", temp.path().as_os_str()),
            Some(bin)
        );
    }

    #[test]
    fn report_marks_missing_cli_without_auth_probe() {
        let cli = AgentBuddyCli::new("__missing_agentbuddy__", "0.4.0");
        let report = cli.detect_with_path(&MockRunner::new(Vec::new()), None);

        assert_eq!(report.status(), SourceCheckStatus::Fail);
        assert_eq!(report.checks[0].kind, SourceCheckKind::Cli);
        assert_eq!(report.checks[1].status, SourceCheckStatus::Skipped);
    }

    #[test]
    fn report_checks_version_and_auth_with_mocked_runner() {
        let temp = tempdir().unwrap();
        let bin = temp.path().join("agentbuddy-test");
        fs::write(&bin, "#!/bin/sh\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&bin).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&bin, permissions).unwrap();
        }
        let cli = AgentBuddyCli::new("agentbuddy-test", "0.4.0");
        let report = cli.detect_with_path(
            &MockRunner::new(vec![
                output(0, "agentbuddy 0.4.0", ""),
                output(0, "jwt-token", ""),
            ]),
            Some(bin),
        );

        assert_eq!(report.status(), SourceCheckStatus::Pass);
        assert_eq!(report.version, Some("0.4.0".to_string()));
    }
}
