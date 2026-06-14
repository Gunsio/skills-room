use std::{
    collections::VecDeque,
    fmt,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};

use crate::actions::{ActionCommand, ActionKind, ActionPlan};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RunnerEvent {
    Started {
        argv: Vec<String>,
    },
    Stdout(String),
    Stderr(String),
    CommandExit {
        argv: Vec<String>,
        code: Option<i32>,
    },
    Finished {
        code: Option<i32>,
    },
    Failed(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RunnerError {
    pub message: String,
}

impl RunnerError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ActionRunner {
    Process,
    Mock(MockActionRunner),
}

impl ActionRunner {
    pub fn for_environment() -> Self {
        if cfg!(test) {
            Self::Mock(MockActionRunner::default())
        } else {
            Self::Process
        }
    }

    pub fn start(&mut self, plan: ActionPlan) -> Result<RunningAction, RunnerError> {
        match self {
            Self::Process => start_process(plan),
            Self::Mock(runner) => Ok(runner.start(plan)),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MockActionRunner {
    events: Vec<RunnerEvent>,
}

impl Default for MockActionRunner {
    fn default() -> Self {
        Self {
            events: vec![
                RunnerEvent::Stdout("mock runner accepted argv".to_string()),
                RunnerEvent::Finished { code: Some(0) },
            ],
        }
    }
}

impl MockActionRunner {
    pub fn new(events: Vec<RunnerEvent>) -> Self {
        Self { events }
    }

    fn start(&self, plan: ActionPlan) -> RunningAction {
        let mut events = VecDeque::new();
        for command in &plan.commands {
            events.push_back(RunnerEvent::Started {
                argv: command.argv.clone(),
            });
        }
        events.extend(self.events.clone());

        RunningAction::buffered(plan, events)
    }
}

pub struct RunningAction {
    pub title: String,
    pub kind: ActionKind,
    pub skill_name: String,
    pub source_label: String,
    pub target_key: String,
    pub command_lines: Vec<String>,
    stderr_tail: VecDeque<String>,
    event_source: RunningSource,
    finished: bool,
}

impl fmt::Debug for RunningAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunningAction")
            .field("title", &self.title)
            .field("kind", &self.kind)
            .field("skill_name", &self.skill_name)
            .field("source_label", &self.source_label)
            .field("target_key", &self.target_key)
            .field("command_lines", &self.command_lines)
            .field("stderr_tail", &self.stderr_tail)
            .field("finished", &self.finished)
            .finish()
    }
}

impl RunningAction {
    fn channel(plan: ActionPlan, receiver: Receiver<RunnerEvent>) -> Self {
        Self::new(plan, RunningSource::Channel(receiver))
    }

    fn buffered(plan: ActionPlan, events: VecDeque<RunnerEvent>) -> Self {
        Self::new(plan, RunningSource::Buffered(events))
    }

    fn new(plan: ActionPlan, event_source: RunningSource) -> Self {
        let command_lines = plan.command_lines();
        Self {
            title: plan.title,
            kind: plan.kind,
            skill_name: plan.skill_name,
            source_label: plan.source,
            target_key: plan.target_key,
            command_lines,
            stderr_tail: VecDeque::new(),
            event_source,
            finished: false,
        }
    }

    pub fn drain_events(&mut self) -> Vec<RunnerEvent> {
        let events: Vec<RunnerEvent> = match &mut self.event_source {
            RunningSource::Buffered(events) => events.pop_front().into_iter().collect(),
            RunningSource::Channel(receiver) => receiver.try_iter().collect(),
        };

        for event in events.iter() {
            match event {
                RunnerEvent::Stderr(line) => {
                    self.stderr_tail.push_back(line.clone());
                    while self.stderr_tail.len() > 4 {
                        self.stderr_tail.pop_front();
                    }
                }
                RunnerEvent::Finished { .. } | RunnerEvent::Failed(_) => self.finished = true,
                _ => {}
            }
        }

        events
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn stderr_summary(&self) -> String {
        if self.stderr_tail.is_empty() {
            "none".to_string()
        } else {
            self.stderr_tail
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(" | ")
        }
    }
}

enum RunningSource {
    Channel(Receiver<RunnerEvent>),
    Buffered(VecDeque<RunnerEvent>),
}

fn start_process(plan: ActionPlan) -> Result<RunningAction, RunnerError> {
    if plan.commands.is_empty() {
        return Err(RunnerError::new("action has no executable argv"));
    }

    let commands = plan.commands.clone();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || run_commands(commands, sender));

    Ok(RunningAction::channel(plan, receiver))
}

fn run_commands(commands: Vec<ActionCommand>, sender: mpsc::Sender<RunnerEvent>) {
    let mut final_code = Some(0);
    for command in commands {
        if sender
            .send(RunnerEvent::Started {
                argv: command.argv.clone(),
            })
            .is_err()
        {
            return;
        }

        let program = &command.argv[0];
        let args = &command.argv[1..];
        let mut process = Command::new(program);
        process
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if command.stdin.is_some() {
            process.stdin(Stdio::piped());
        }
        let child = process.spawn();

        let mut child = match child {
            Ok(child) => child,
            Err(error) => {
                let _ = sender.send(RunnerEvent::Failed(format!(
                    "failed to spawn {}: {error}",
                    command.display_line()
                )));
                return;
            }
        };

        if let Some(stdin) = &command.stdin
            && let Some(mut child_stdin) = child.stdin.take()
            && let Err(error) = child_stdin.write_all(stdin.as_bytes())
        {
            let _ = sender.send(RunnerEvent::Failed(format!(
                "failed to write stdin for {}: {error}",
                command.display_line()
            )));
            return;
        }

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let stdout_sender = sender.clone();
        let stderr_sender = sender.clone();

        let stdout_thread = stdout.map(|stdout| {
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    let _ = stdout_sender.send(RunnerEvent::Stdout(line));
                }
            })
        });
        let stderr_thread = stderr.map(|stderr| {
            thread::spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    let _ = stderr_sender.send(RunnerEvent::Stderr(line));
                }
            })
        });

        let status = child.wait();
        if let Some(thread) = stdout_thread {
            let _ = thread.join();
        }
        if let Some(thread) = stderr_thread {
            let _ = thread.join();
        }

        match status {
            Ok(status) => {
                let code = status.code();
                final_code = code;
                let _ = sender.send(RunnerEvent::CommandExit {
                    argv: command.argv.clone(),
                    code,
                });
                if !status.success() {
                    let _ = sender.send(RunnerEvent::Finished { code });
                    return;
                }
            }
            Err(error) => {
                let _ = sender.send(RunnerEvent::Failed(format!(
                    "failed to wait for {}: {error}",
                    command.display_line()
                )));
                return;
            }
        }
    }

    let _ = sender.send(RunnerEvent::Finished { code: final_code });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{ActionKind, ActionPlanner},
        skill::fixture_skills,
    };

    #[test]
    fn mock_runner_streams_started_output_and_finish_events() {
        let skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "taproom")
            .unwrap();
        let plan = ActionPlanner::new(false)
            .plan_selected(ActionKind::Install, Some(&skill))
            .unwrap();
        let mut running = MockActionRunner::default().start(plan);

        assert!(matches!(
            running.drain_events().as_slice(),
            [RunnerEvent::Started { .. }]
        ));
        assert!(matches!(
            running.drain_events().as_slice(),
            [RunnerEvent::Stdout(_)]
        ));
        assert!(matches!(
            running.drain_events().as_slice(),
            [RunnerEvent::Finished { code: Some(0) }]
        ));
        assert!(running.is_finished());
    }

    #[test]
    fn running_action_keeps_stderr_tail_for_failure_summary() {
        let skill = fixture_skills()
            .into_iter()
            .find(|skill| skill.name == "taproom")
            .unwrap();
        let plan = ActionPlanner::new(false)
            .plan_selected(ActionKind::Install, Some(&skill))
            .unwrap();
        let runner = MockActionRunner::new(vec![
            RunnerEvent::Stderr("first".to_string()),
            RunnerEvent::Stderr("last".to_string()),
            RunnerEvent::Finished { code: Some(1) },
        ]);
        let mut running = runner.start(plan);

        while !running.is_finished() {
            let _ = running.drain_events();
        }

        assert_eq!(running.stderr_summary(), "first | last");
    }
}
