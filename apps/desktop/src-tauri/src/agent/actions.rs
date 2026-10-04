//! Bounded action adapters. Every adapter declares a risk class, verifies its
//! own result, and records how it can be undone. Nothing here sends data to a
//! provider, and nothing here can message people, delete user data, or change
//! repositories.

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::models::{ActionKindView, CheckPresetView, Verification};
use crate::{
    error::{AppError, AppResult},
    platform::{
        normalized_application_name, open_external_url, open_local_file, open_native_application,
        reopenable_web_url,
    },
    prediction::sanitize_text,
};

pub(crate) const CHECK_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const OUTPUT_TAIL_BYTES: usize = 16 * 1024;
const OUTPUT_EXCERPT_CHARS: usize = 4_000;
const MAX_DRAFT_CHARS: usize = 12_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RiskClass {
    ReadOnly,
    Ephemeral,
    Draft,
    PersistentReversible,
    ExternalCommunication,
    Destructive,
}

impl RiskClass {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::Ephemeral => "ephemeral",
            Self::Draft => "draft",
            Self::PersistentReversible => "persistent_reversible",
            Self::ExternalCommunication => "external_communication",
            Self::Destructive => "destructive",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "Read-only",
            Self::Ephemeral => "Ephemeral, local",
            Self::Draft => "Draft for review",
            Self::PersistentReversible => "Persistent, reversible",
            Self::ExternalCommunication => "External communication",
            Self::Destructive => "Destructive or sensitive",
        }
    }

    fn default_policy(self) -> &'static str {
        match self {
            Self::ReadOnly => "Automatic",
            Self::Ephemeral | Self::Draft => "Ask until you allow it",
            Self::PersistentReversible => "Ask every time",
            Self::ExternalCommunication | Self::Destructive => "Not available",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ActionKind {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub risk: RiskClass,
    pub interrupts_user: bool,
    pub rollback: &'static str,
    pub available: bool,
}

pub(crate) const ACTION_KINDS: &[ActionKind] = &[
    ActionKind {
        id: "open_url",
        title: "Open a web resource",
        description: "Opens a credential-free HTTP(S) page in your default browser.",
        risk: RiskClass::Ephemeral,
        interrupts_user: true,
        rollback: "Not needed; close the tab.",
        available: true,
    },
    ActionKind {
        id: "open_application",
        title: "Open an application",
        description: "Brings an installed Mac application to the front.",
        risk: RiskClass::Ephemeral,
        interrupts_user: true,
        rollback: "Not needed; quit or hide the app.",
        available: true,
    },
    ActionKind {
        id: "write_draft",
        title: "Write a local draft",
        description: "Saves a Markdown draft in Knov's own Drafts folder for you to review.",
        risk: RiskClass::Draft,
        interrupts_user: false,
        rollback: "Knov deletes the draft if you have not edited it.",
        available: true,
    },
    ActionKind {
        id: "run_checks",
        title: "Run checks in a workspace",
        description:
            "Runs an allow-listed test command in a folder you approved. Output stays on this Mac.",
        risk: RiskClass::Ephemeral,
        interrupts_user: false,
        rollback: "Not needed; checks do not change Knov data.",
        available: true,
    },
    ActionKind {
        id: "commit_changes",
        title: "Commit code changes",
        description: "Creating commits or branches is not implemented in this alpha.",
        risk: RiskClass::PersistentReversible,
        interrupts_user: false,
        rollback: "Would require reverting the commit.",
        available: false,
    },
    ActionKind {
        id: "send_message",
        title: "Send a message or email",
        description: "Knov never sends messages, emails, or forms on your behalf in this alpha.",
        risk: RiskClass::ExternalCommunication,
        interrupts_user: false,
        rollback: "Impossible once sent.",
        available: false,
    },
    ActionKind {
        id: "delete_or_pay",
        title: "Delete data, pay, or change security settings",
        description: "Destructive, financial, and credential actions are prohibited.",
        risk: RiskClass::Destructive,
        interrupts_user: false,
        rollback: "Often impossible.",
        available: false,
    },
];

pub(crate) fn action_kind(id: &str) -> Option<&'static ActionKind> {
    ACTION_KINDS.iter().find(|kind| kind.id == id)
}

pub(crate) fn catalog() -> Vec<ActionKindView> {
    ACTION_KINDS
        .iter()
        .map(|kind| ActionKindView {
            action_type: kind.id.into(),
            title: kind.title.into(),
            description: kind.description.into(),
            risk_class: kind.risk.as_str().into(),
            risk_label: kind.risk.label().into(),
            default_policy: kind.risk.default_policy().into(),
            interrupts_user: kind.interrupts_user,
            rollback: kind.rollback.into(),
            available: kind.available,
        })
        .collect()
}

/// A step's stored action. Targets are re-validated when a run is planned and
/// again immediately before execution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ActionSpec {
    #[serde(rename = "open_url")]
    OpenUrl {
        url: String,
        #[serde(default, rename = "resolveDomain")]
        resolve_domain: Option<String>,
    },
    #[serde(rename = "open_application")]
    OpenApplication { app: String },
    #[serde(rename = "write_draft")]
    WriteDraft { template: String },
    #[serde(rename = "run_checks")]
    RunChecks {
        #[serde(rename = "workspaceId")]
        workspace_id: String,
        preset: String,
    },
}

impl ActionSpec {
    pub(crate) fn action_type(&self) -> &'static str {
        match self {
            Self::OpenUrl { .. } => "open_url",
            Self::OpenApplication { .. } => "open_application",
            Self::WriteDraft { .. } => "write_draft",
            Self::RunChecks { .. } => "run_checks",
        }
    }
}

/// A fully resolved, ready-to-execute action stored in the journal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub(crate) enum ResolvedAction {
    #[serde(rename = "open_url")]
    OpenUrl { url: String },
    #[serde(rename = "open_application")]
    OpenApplication { app: String },
    #[serde(rename = "write_draft")]
    WriteDraft {
        title: String,
        #[serde(rename = "fileStem")]
        file_stem: String,
        content: String,
    },
    #[serde(rename = "run_checks")]
    RunChecks {
        #[serde(rename = "workspaceId")]
        workspace_id: String,
        path: String,
        preset: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub(crate) enum RollbackPlan {
    #[serde(rename = "delete_draft")]
    DeleteDraft { path: String, sha256: String },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Execution {
    pub status: &'static str,
    pub summary: String,
    pub output_excerpt: Option<String>,
    pub verification: Verification,
    pub rollback: Option<RollbackPlan>,
}

impl Execution {
    fn failed(summary: impl Into<String>, check: impl Into<String>) -> Self {
        Self {
            status: "failed",
            summary: summary.into(),
            output_excerpt: None,
            verification: Verification {
                passed: false,
                checks: vec![check.into()],
            },
            rollback: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOutcome {
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub output_tail: String,
    pub duration_ms: u128,
}

/// Side effects live behind this trait so planning, policy, journaling, and
/// verification are testable without opening windows or running programs.
pub trait ActionHost: Send + Sync {
    fn open_url(&self, url: &str) -> Result<(), String>;
    fn open_application(&self, app: &str) -> Result<(), String>;
    fn open_file(&self, path: &Path) -> Result<(), String>;
    fn drafts_dir(&self) -> PathBuf;
    fn run_command(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<CommandOutcome, String>;
}

pub struct SystemHost {
    drafts_dir: PathBuf,
}

impl SystemHost {
    pub fn new(drafts_dir: PathBuf) -> Self {
        Self { drafts_dir }
    }
}

impl ActionHost for SystemHost {
    fn open_url(&self, url: &str) -> Result<(), String> {
        match open_external_url(url) {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("the system open command exited with {status}")),
            Err(error) => Err(error.to_string()),
        }
    }

    fn open_application(&self, app: &str) -> Result<(), String> {
        match open_native_application(app) {
            Ok(status) if status.success() => Ok(()),
            Ok(_) => Err(format!("{app} could not be opened")),
            Err(error) => Err(error.to_string()),
        }
    }

    fn open_file(&self, path: &Path) -> Result<(), String> {
        match open_local_file(path) {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("the system open command exited with {status}")),
            Err(error) => Err(error.to_string()),
        }
    }

    fn drafts_dir(&self) -> PathBuf {
        self.drafts_dir.clone()
    }

    fn run_command(
        &self,
        program: &str,
        args: &[&str],
        cwd: &Path,
        timeout: Duration,
    ) -> Result<CommandOutcome, String> {
        let executable = resolve_program(program)
            .ok_or_else(|| format!("`{program}` was not found in the usual install locations"))?;
        let mut command = Command::new(executable);
        command
            .args(args)
            .current_dir(cwd)
            .env("PATH", search_path())
            .env("CI", "1")
            .env("NO_COLOR", "1")
            .env("TERM", "dumb")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let started = Instant::now();
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        let tail = Arc::new(Mutex::new(Vec::<u8>::new()));
        for stream in [
            child
                .stdout
                .take()
                .map(|value| Box::new(value) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|value| Box::new(value) as Box<dyn Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let tail = tail.clone();
            std::thread::spawn(move || {
                let mut stream = stream;
                let mut buffer = [0_u8; 4096];
                while let Ok(read) = stream.read(&mut buffer) {
                    if read == 0 {
                        break;
                    }
                    let mut tail = tail.lock().unwrap_or_else(|error| error.into_inner());
                    tail.extend_from_slice(&buffer[..read]);
                    if tail.len() > OUTPUT_TAIL_BYTES {
                        let excess = tail.len() - OUTPUT_TAIL_BYTES;
                        tail.drain(..excess);
                    }
                }
            });
        }
        let (exit_code, timed_out) = loop {
            match child.try_wait() {
                Ok(Some(status)) => break (status.code(), false),
                Ok(None) if started.elapsed() >= timeout => {
                    terminate(&mut child);
                    break (None, true);
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(150)),
                Err(error) => {
                    terminate(&mut child);
                    return Err(error.to_string());
                }
            }
        };
        // Give the reader threads a moment to drain buffered output.
        std::thread::sleep(Duration::from_millis(120));
        let output_tail = {
            let tail = tail.lock().unwrap_or_else(|error| error.into_inner());
            String::from_utf8_lossy(&tail).into_owned()
        };
        Ok(CommandOutcome {
            exit_code,
            timed_out,
            output_tail,
            duration_ms: started.elapsed().as_millis(),
        })
    }
}

fn terminate(child: &mut std::process::Child) {
    #[cfg(unix)]
    {
        // The command runs in its own process group; stop the whole group so
        // spawned compilers and test runners do not linger.
        let _ = Command::new("/bin/kill")
            .args(["-KILL", &format!("-{}", child.id())])
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn search_directories() -> Vec<PathBuf> {
    let mut directories = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    let home = dirs::home_dir();
    for candidate in [
        Some(PathBuf::from("/opt/homebrew/bin")),
        Some(PathBuf::from("/usr/local/bin")),
        home.as_ref().map(|home| home.join(".cargo/bin")),
        home.as_ref().map(|home| home.join(".local/bin")),
        Some(PathBuf::from("/usr/bin")),
        Some(PathBuf::from("/bin")),
    ]
    .into_iter()
    .flatten()
    {
        if !directories.contains(&candidate) {
            directories.push(candidate);
        }
    }
    directories
}

fn search_path() -> std::ffi::OsString {
    std::env::join_paths(search_directories()).unwrap_or_default()
}

fn resolve_program(program: &str) -> Option<PathBuf> {
    if program.contains('/') || program.is_empty() {
        return None;
    }
    search_directories()
        .into_iter()
        .map(|directory| directory.join(program))
        .find(|candidate| is_executable(candidate))
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CheckPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub markers: &'static [&'static str],
    pub program: &'static str,
    pub args: &'static [&'static str],
}

/// The only commands Knov will ever run. No shell is involved and arguments
/// are fixed; users choose a preset, never a command line.
pub(crate) const CHECK_PRESETS: &[CheckPreset] = &[
    CheckPreset {
        id: "cargo-test",
        label: "cargo test",
        markers: &["Cargo.toml"],
        program: "cargo",
        args: &["test"],
    },
    CheckPreset {
        id: "npm-test",
        label: "npm test",
        markers: &["package.json"],
        program: "npm",
        args: &["test"],
    },
    CheckPreset {
        id: "pnpm-test",
        label: "pnpm test",
        markers: &["pnpm-lock.yaml"],
        program: "pnpm",
        args: &["test"],
    },
    CheckPreset {
        id: "yarn-test",
        label: "yarn test",
        markers: &["yarn.lock"],
        program: "yarn",
        args: &["test"],
    },
    CheckPreset {
        id: "pytest",
        label: "pytest",
        markers: &["pytest.ini", "pyproject.toml", "setup.cfg", "tox.ini"],
        program: "python3",
        args: &["-m", "pytest", "-q"],
    },
    CheckPreset {
        id: "go-test",
        label: "go test ./...",
        markers: &["go.mod"],
        program: "go",
        args: &["test", "./..."],
    },
    CheckPreset {
        id: "swift-test",
        label: "swift test",
        markers: &["Package.swift"],
        program: "swift",
        args: &["test"],
    },
    CheckPreset {
        id: "make-test",
        label: "make test",
        markers: &["Makefile"],
        program: "make",
        args: &["test"],
    },
];

pub(crate) fn check_preset(id: &str) -> Option<&'static CheckPreset> {
    CHECK_PRESETS.iter().find(|preset| preset.id == id)
}

pub(crate) fn detect_presets(path: &Path) -> Vec<CheckPresetView> {
    CHECK_PRESETS
        .iter()
        .filter(|preset| {
            preset
                .markers
                .iter()
                .any(|marker| path.join(marker).is_file())
        })
        .map(|preset| CheckPresetView {
            id: preset.id.into(),
            label: preset.label.into(),
        })
        .collect()
}

/// A folder may be approved for checks only if it is a real directory inside
/// the home folder (not the home folder itself or app data) with a supported
/// project marker.
pub(crate) fn validate_workspace_path(raw: &str) -> AppResult<(PathBuf, String)> {
    let home = dirs::home_dir()
        .and_then(|home| home.canonicalize().ok())
        .ok_or_else(|| AppError::InvalidInput("Home folder is unavailable.".into()))?;
    validate_workspace_path_within(raw, &home)
}

pub(crate) fn validate_workspace_path_within(
    raw: &str,
    home: &Path,
) -> AppResult<(PathBuf, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) || trimmed.len() > 1_024 {
        return Err(AppError::InvalidInput("Enter a folder path.".into()));
    }
    let expanded = match trimmed.strip_prefix("~/") {
        Some(rest) => home.join(rest),
        None => PathBuf::from(trimmed),
    };
    if !expanded.is_absolute() {
        return Err(AppError::InvalidInput(
            "Use an absolute folder path such as ~/code/project.".into(),
        ));
    }
    let canonical = expanded
        .canonicalize()
        .map_err(|_| AppError::InvalidInput("That folder does not exist.".into()))?;
    if !canonical.is_dir() {
        return Err(AppError::InvalidInput("That path is not a folder.".into()));
    }
    if !canonical.starts_with(home)
        || canonical == home
        || canonical.starts_with(home.join("Library"))
    {
        return Err(AppError::InvalidInput(
            "Approve a project folder inside your home folder (not the home folder itself or Library)."
                .into(),
        ));
    }
    if detect_presets(&canonical).is_empty() {
        return Err(AppError::InvalidInput(
            "No supported test setup was found (Cargo.toml, package.json, go.mod, pyproject.toml, Package.swift, or Makefile)."
                .into(),
        ));
    }
    let label = canonical
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "workspace".into());
    Ok((canonical, sanitize_text(&label, 80)))
}

/// Validates a single resolved action right before it runs.
pub(crate) fn validate_resolved(
    action: &ResolvedAction,
    approved_paths: &[(String, String)],
) -> AppResult<()> {
    match action {
        ResolvedAction::OpenUrl { url } => reopenable_web_url(url).map(|_| ()),
        ResolvedAction::OpenApplication { app } => normalized_application_name(app).map(|_| ()),
        ResolvedAction::WriteDraft {
            content, file_stem, ..
        } => {
            if content.chars().count() > MAX_DRAFT_CHARS
                || file_stem.is_empty()
                || !file_stem
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '-')
            {
                return Err(AppError::InvalidInput("The draft is not valid.".into()));
            }
            Ok(())
        }
        ResolvedAction::RunChecks {
            workspace_id,
            path,
            preset,
        } => {
            let approved = approved_paths
                .iter()
                .any(|(id, approved_path)| id == workspace_id && approved_path == path);
            if !approved {
                return Err(AppError::InvalidInput(
                    "The workspace is no longer approved for checks.".into(),
                ));
            }
            let preset = check_preset(preset)
                .ok_or_else(|| AppError::InvalidInput("Unknown check preset.".into()))?;
            let canonical = Path::new(path)
                .canonicalize()
                .map_err(|_| AppError::InvalidInput("The workspace folder is missing.".into()))?;
            if canonical != Path::new(path)
                || !preset
                    .markers
                    .iter()
                    .any(|marker| canonical.join(marker).is_file())
            {
                return Err(AppError::InvalidInput(
                    "The workspace changed since it was approved.".into(),
                ));
            }
            Ok(())
        }
    }
}

pub(crate) fn execute(
    action: &ResolvedAction,
    host: &dyn ActionHost,
    approved_paths: &[(String, String)],
) -> Execution {
    if let Err(error) = validate_resolved(action, approved_paths) {
        return Execution::failed(
            format!("Stopped before acting: {error}"),
            "Pre-execution validation failed",
        );
    }
    match action {
        ResolvedAction::OpenUrl { url } => match host.open_url(url) {
            Ok(()) => Execution {
                status: "succeeded",
                summary: format!("Opened {}", display_url(url)),
                output_excerpt: None,
                verification: Verification {
                    passed: true,
                    checks: vec![
                        "URL passed the credential-free HTTP(S) check".into(),
                        "macOS reported the resource opened".into(),
                    ],
                },
                rollback: None,
            },
            Err(error) => Execution::failed(
                format!("Could not open {}", display_url(url)),
                format!("macOS reported an error: {}", sanitize_text(&error, 160)),
            ),
        },
        ResolvedAction::OpenApplication { app } => match host.open_application(app) {
            Ok(()) => Execution {
                status: "succeeded",
                summary: format!("Opened {app}"),
                output_excerpt: None,
                verification: Verification {
                    passed: true,
                    checks: vec!["macOS reported the application opened".into()],
                },
                rollback: None,
            },
            Err(error) => Execution::failed(
                format!("Could not open {app}"),
                format!("macOS reported an error: {}", sanitize_text(&error, 160)),
            ),
        },
        ResolvedAction::WriteDraft {
            title,
            file_stem,
            content,
        } => write_draft(host, title, file_stem, content),
        ResolvedAction::RunChecks { path, preset, .. } => {
            let preset = check_preset(preset).expect("validated preset");
            run_checks(host, Path::new(path), preset)
        }
    }
}

fn write_draft(host: &dyn ActionHost, title: &str, file_stem: &str, content: &str) -> Execution {
    let directory = host.drafts_dir();
    let result = (|| -> std::io::Result<(PathBuf, String)> {
        fs::create_dir_all(&directory)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
        }
        let path = directory.join(format!("{file_stem}.md"));
        if path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "a draft with this name already exists",
            ));
        }
        fs::write(&path, content.as_bytes())?;
        let written = fs::read(&path)?;
        Ok((path, format!("{:x}", Sha256::digest(&written))))
    })();
    match result {
        Ok((path, digest)) if digest == format!("{:x}", Sha256::digest(content.as_bytes())) => {
            Execution {
                status: "succeeded",
                summary: format!("Saved draft “{}”", sanitize_text(title, 80)),
                output_excerpt: None,
                verification: Verification {
                    passed: true,
                    checks: vec![
                        "Draft written inside Knov's Drafts folder".into(),
                        "Content hash verified after writing".into(),
                    ],
                },
                rollback: Some(RollbackPlan::DeleteDraft {
                    path: path.to_string_lossy().into_owned(),
                    sha256: digest,
                }),
            }
        }
        Ok((path, _)) => {
            let _ = fs::remove_file(path);
            Execution::failed(
                "The draft did not verify and was removed",
                "Content hash mismatch",
            )
        }
        Err(error) => Execution::failed(
            "Could not save the draft",
            format!(
                "File system error: {}",
                sanitize_text(&error.to_string(), 160)
            ),
        ),
    }
}

fn run_checks(host: &dyn ActionHost, path: &Path, preset: &CheckPreset) -> Execution {
    match host.run_command(preset.program, preset.args, path, CHECK_TIMEOUT) {
        Ok(outcome) => {
            let excerpt = sanitize_output(&outcome.output_tail);
            let seconds = (outcome.duration_ms / 1000).max(1);
            if outcome.timed_out {
                return Execution {
                    status: "failed",
                    summary: format!("`{}` timed out and was stopped", preset.label),
                    output_excerpt: Some(excerpt),
                    verification: Verification {
                        passed: false,
                        checks: vec![format!(
                            "Stopped after the {}-minute limit",
                            CHECK_TIMEOUT.as_secs() / 60
                        )],
                    },
                    rollback: None,
                };
            }
            let code = outcome.exit_code.unwrap_or(-1);
            let checks = vec![
                format!("`{}` ran to completion in {seconds}s", preset.label),
                format!("Exit code {code} captured"),
            ];
            if code == 0 {
                Execution {
                    status: "succeeded",
                    summary: format!("Checks passed: `{}`", preset.label),
                    output_excerpt: Some(excerpt),
                    verification: Verification {
                        passed: true,
                        checks,
                    },
                    rollback: None,
                }
            } else {
                Execution {
                    status: "needs_attention",
                    summary: format!("Checks failed: `{}` exited with {code}", preset.label),
                    output_excerpt: Some(excerpt),
                    verification: Verification {
                        passed: true,
                        checks,
                    },
                    rollback: None,
                }
            }
        }
        Err(error) => Execution::failed(
            format!("Could not start `{}`", preset.label),
            sanitize_text(&error, 200),
        ),
    }
}

/// Keeps the useful tail of command output while removing the home path and
/// any line that looks like it carries a credential.
pub(crate) fn sanitize_output(value: &str) -> String {
    let home = dirs::home_dir().map(|home| home.to_string_lossy().into_owned());
    let lines = value
        .lines()
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if [
                "token=",
                "api_key",
                "apikey",
                "secret=",
                "password=",
                "authorization:",
                "bearer ",
            ]
            .iter()
            .any(|marker| lower.contains(marker))
            {
                "[line redacted]".to_string()
            } else {
                match home.as_deref() {
                    Some(home) if !home.is_empty() => line.replace(home, "~"),
                    _ => line.to_string(),
                }
            }
        })
        .filter(|line| !line.chars().all(char::is_whitespace))
        .collect::<Vec<_>>();
    let mut excerpt = lines.join("\n");
    if excerpt.chars().count() > OUTPUT_EXCERPT_CHARS {
        excerpt = excerpt
            .chars()
            .rev()
            .take(OUTPUT_EXCERPT_CHARS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
    }
    excerpt
        .chars()
        .filter(|character| *character == '\n' || !character.is_control())
        .collect()
}

pub(crate) fn rollback(plan: &RollbackPlan, host: &dyn ActionHost) -> AppResult<String> {
    match plan {
        RollbackPlan::DeleteDraft { path, sha256 } => {
            let drafts = host
                .drafts_dir()
                .canonicalize()
                .map_err(|_| AppError::InvalidInput("The Drafts folder is unavailable.".into()))?;
            let path = Path::new(path);
            if !path.exists() {
                return Ok("The draft was already removed.".into());
            }
            let canonical = path
                .canonicalize()
                .map_err(|_| AppError::InvalidInput("The draft path is unavailable.".into()))?;
            if canonical.parent() != Some(drafts.as_path()) {
                return Err(AppError::InvalidInput(
                    "Knov only removes drafts inside its own Drafts folder.".into(),
                ));
            }
            let current = format!("{:x}", Sha256::digest(fs::read(&canonical)?));
            if &current != sha256 {
                return Err(AppError::InvalidInput(
                    "The draft was edited after Knov wrote it, so Knov left your version in place."
                        .into(),
                ));
            }
            fs::remove_file(&canonical)?;
            Ok("Draft deleted.".into())
        }
    }
}

/// Resolves a journaled draft path for opening, confined to the Drafts folder.
pub(crate) fn draft_path_for_open(
    plan: &RollbackPlan,
    host: &dyn ActionHost,
) -> AppResult<PathBuf> {
    let RollbackPlan::DeleteDraft { path, .. } = plan;
    let drafts = host
        .drafts_dir()
        .canonicalize()
        .map_err(|_| AppError::InvalidInput("The Drafts folder is unavailable.".into()))?;
    let canonical = Path::new(path)
        .canonicalize()
        .map_err(|_| AppError::InvalidInput("The draft no longer exists.".into()))?;
    if canonical.parent() != Some(drafts.as_path()) {
        return Err(AppError::InvalidInput(
            "Knov only opens drafts inside its own Drafts folder.".into(),
        ));
    }
    Ok(canonical)
}

pub(crate) fn display_url(url: &str) -> String {
    let trimmed = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    sanitize_text(trimmed, 90)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    pub(crate) struct FakeHost {
        pub drafts: PathBuf,
        pub calls: Mutex<Vec<String>>,
        pub fail_open: bool,
        pub exit_code: i32,
    }

    impl FakeHost {
        pub(crate) fn new(drafts: PathBuf) -> Self {
            Self {
                drafts,
                ..Self::default()
            }
        }

        pub(crate) fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl ActionHost for FakeHost {
        fn open_url(&self, url: &str) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("open_url {url}"));
            if self.fail_open {
                Err("denied".into())
            } else {
                Ok(())
            }
        }
        fn open_application(&self, app: &str) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("open_application {app}"));
            Ok(())
        }
        fn open_file(&self, path: &Path) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("open_file {}", path.display()));
            Ok(())
        }
        fn drafts_dir(&self) -> PathBuf {
            self.drafts.clone()
        }
        fn run_command(
            &self,
            program: &str,
            args: &[&str],
            cwd: &Path,
            _timeout: Duration,
        ) -> Result<CommandOutcome, String> {
            self.calls.lock().unwrap().push(format!(
                "run {program} {} in {}",
                args.join(" "),
                cwd.display()
            ));
            Ok(CommandOutcome {
                exit_code: Some(self.exit_code),
                timed_out: false,
                output_tail: "test result: ok\nAPI_KEY=abc123\n".into(),
                duration_ms: 1_500,
            })
        }
    }

    #[test]
    fn unsafe_targets_are_rejected_before_any_side_effect() {
        let directory = tempfile::tempdir().unwrap();
        let host = FakeHost::new(directory.path().join("drafts"));
        for action in [
            ResolvedAction::OpenUrl {
                url: "file:///etc/passwd".into(),
            },
            ResolvedAction::OpenUrl {
                url: "https://user:secret@example.com".into(),
            },
            ResolvedAction::OpenApplication {
                app: "../../bin/sh".into(),
            },
            ResolvedAction::WriteDraft {
                title: "x".into(),
                file_stem: "../escape".into(),
                content: "x".into(),
            },
            ResolvedAction::RunChecks {
                workspace_id: "missing".into(),
                path: "/tmp".into(),
                preset: "cargo-test".into(),
            },
        ] {
            let execution = execute(&action, &host, &[]);
            assert_eq!(execution.status, "failed", "{action:?}");
            assert!(!execution.verification.passed);
        }
        assert!(host.calls().is_empty());
    }

    #[test]
    fn drafts_verify_and_roll_back_only_when_unedited() {
        let directory = tempfile::tempdir().unwrap();
        let host = FakeHost::new(directory.path().join("drafts"));
        let action = ResolvedAction::WriteDraft {
            title: "Resume brief".into(),
            file_stem: "2026-10-04-resume-brief-ab12".into(),
            content: "# Resume brief\n".into(),
        };
        let execution = execute(&action, &host, &[]);
        assert_eq!(execution.status, "succeeded");
        assert!(execution.verification.passed);
        let plan = execution.rollback.clone().expect("rollback plan");
        let RollbackPlan::DeleteDraft { path, .. } = &plan;
        assert!(Path::new(path).exists());
        assert!(draft_path_for_open(&plan, &host).is_ok());

        // A second write with the same name never overwrites.
        assert_eq!(execute(&action, &host, &[]).status, "failed");

        fs::write(path, "# Edited by me\n").unwrap();
        assert!(rollback(&plan, &host).is_err());
        assert!(Path::new(path).exists());

        let second = execute(
            &ResolvedAction::WriteDraft {
                title: "Second".into(),
                file_stem: "second".into(),
                content: "# Second\n".into(),
            },
            &host,
            &[],
        );
        let plan = second.rollback.expect("rollback plan");
        assert_eq!(rollback(&plan, &host).unwrap(), "Draft deleted.");
        assert_eq!(
            rollback(&plan, &host).unwrap(),
            "The draft was already removed."
        );
    }

    #[test]
    fn checks_run_only_allow_listed_presets_in_approved_workspaces() {
        let home = tempfile::tempdir().unwrap();
        let home_path = home.path().canonicalize().unwrap();
        let project = home_path.join("code/project");
        fs::create_dir_all(&project).unwrap();
        assert!(validate_workspace_path_within(project.to_str().unwrap(), &home_path).is_err());
        fs::write(project.join("Cargo.toml"), "[package]\n").unwrap();
        let (path, label) =
            validate_workspace_path_within(project.to_str().unwrap(), &home_path).unwrap();
        assert_eq!(label, "project");
        assert!(validate_workspace_path_within(home_path.to_str().unwrap(), &home_path).is_err());
        assert!(validate_workspace_path_within("relative/path", &home_path).is_err());

        let host = FakeHost {
            exit_code: 101,
            ..FakeHost::new(home_path.join("drafts"))
        };
        let path = path.to_string_lossy().into_owned();
        let approved = vec![("ws-1".to_string(), path.clone())];
        let action = ResolvedAction::RunChecks {
            workspace_id: "ws-1".into(),
            path: path.clone(),
            preset: "cargo-test".into(),
        };
        let execution = execute(&action, &host, &approved);
        assert_eq!(execution.status, "needs_attention");
        assert!(execution.verification.passed);
        let excerpt = execution.output_excerpt.unwrap();
        assert!(excerpt.contains("test result: ok"));
        assert!(!excerpt.contains("abc123"));
        assert_eq!(host.calls(), vec![format!("run cargo test in {path}")]);

        // A preset whose marker is missing is refused.
        let npm = ResolvedAction::RunChecks {
            workspace_id: "ws-1".into(),
            path,
            preset: "npm-test".into(),
        };
        assert_eq!(execute(&npm, &host, &approved).status, "failed");
    }

    #[test]
    fn catalog_marks_external_and_destructive_actions_unavailable() {
        for kind in ACTION_KINDS {
            if kind.risk >= RiskClass::PersistentReversible {
                assert!(!kind.available, "{}", kind.id);
            }
        }
        assert!(resolve_program("../sh").is_none());
        assert!(resolve_program("/bin/sh").is_none());
    }
}
