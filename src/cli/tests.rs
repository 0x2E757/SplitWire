//! The pipe commands over a fake engine: what each sends, prints and exits with.

use std::collections::VecDeque;
use std::fs;
use std::time::UNIX_EPOCH;

use super::*;
use crate::ipc::{
    ErrorCode, ErrorReply, ListsView, ProfileState, ProfileSummary, ProfileTarget, RefusedProblem,
    Request, Response,
};
use crate::list::ListSet;
use crate::platform::pipe::PipeError;
use crate::profile::fixtures::{PRIVATE, full_conf};
use crate::profile::{ImportContext, ImportPreview, ImportRequest, Pin};

/// Answers from a queue and keeps the tags of what it was sent.
struct FakeLink {
    answers: VecDeque<Result<Response, PipeError>>,
    sent: Vec<String>,
}

impl EngineLink for FakeLink {
    fn exchange(&mut self, request: &Request) -> Result<Response, PipeError> {
        self.sent.push(format!("{request:?}"));
        self.answers
            .pop_front()
            .expect("an answer for every request")
    }

    fn describe(&self) -> String {
        r"\\.\pipe\SplitWire.Fake".into()
    }
}

struct Outcome {
    code: i32,
    out: String,
    err: String,
    sent: Vec<String>,
}

fn run(action: PipeAction, answers: Vec<Result<Response, PipeError>>, input: &str) -> Outcome {
    let mut link = FakeLink {
        answers: answers.into(),
        sent: Vec::new(),
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut input = input.as_bytes();
    let times = Times {
        now: UNIX_EPOCH,
        offset_secs: 0,
    };
    let code = {
        let console = Console {
            out: &mut out,
            err: &mut err,
            input: &mut input,
        };
        PipeCommands::new(&mut link, console, times).run(action)
    };
    Outcome {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
        sent: link.sent,
    }
}

impl EngineLink for &mut FakeLink {
    fn exchange(&mut self, request: &Request) -> Result<Response, PipeError> {
        (**self).exchange(request)
    }

    fn describe(&self) -> String {
        (**self).describe()
    }
}

fn error(code: ErrorCode, message: &str) -> Result<Response, PipeError> {
    Ok(Response::Error(ErrorReply {
        code,
        message: message.into(),
    }))
}

fn summary(name: &str, state: ProfileState) -> ProfileSummary {
    ProfileSummary {
        name: name.into(),
        colour: Pin::new(2).unwrap(),
        state,
        since: None,
        uptime_secs: None,
        last_connected: None,
        handshake_age_secs: None,
        rx_rate: 0,
        tx_rate: 0,
        rx_total: 0,
        tx_total: 0,
        learned: 0,
        dns_rtt_ms: None,
        upstream: None,
        keepalive: 0,
        last_attempt: None,
        problem: None,
    }
}

fn empty_status() -> crate::ipc::StatusReport {
    crate::ipc::StatusReport {
        version: "0.1.0".into(),
        wgnt_version: None,
        since: UNIX_EPOCH,
        tray_state: crate::ipc::TrayState::Off,
        revs: crate::ipc::Revisions::default(),
        counters: crate::ipc::Counters::default(),
        nrpt: crate::ipc::NrptStatus::default(),
        fallback: Vec::new(),
        problems: Vec::new(),
        alerts: Vec::new(),
        profiles: Vec::new(),
    }
}

fn preview(name: &str, text: &str) -> ImportPreview {
    let file_name = format!("{name}.conf");
    ImportPreview::build(
        &ImportRequest {
            name,
            file_name: &file_name,
            conf_text: text,
        },
        &ImportContext {
            profiles: &[],
            lists: &ListSet::default(),
            replaced: None,
            endpoint_addresses: &[],
            answers: &[],
        },
    )
}

#[test]
fn no_engine_exits_with_3_and_names_the_pipe() {
    let outcome = run(PipeAction::List, vec![Err(PipeError::NotRunning)], "");
    assert_eq!(outcome.code, ExitCode::NOT_REACHABLE);
    assert!(
        outcome.err.contains(r"\\.\pipe\SplitWire.Fake"),
        "{}",
        outcome.err
    );
    let broken = run(
        PipeAction::List,
        vec![Err(PipeError::Failed("cut".into()))],
        "",
    );
    assert_eq!(broken.code, ExitCode::ERROR);
}

#[test]
fn error_answers_map_to_their_exit_codes() {
    let unknown = run(
        PipeAction::Connect {
            name: "Ghost".into(),
        },
        vec![error(ErrorCode::UnknownProfile, "no profile named Ghost")],
        "",
    );
    assert_eq!(unknown.code, ExitCode::UNKNOWN_PROFILE);
    assert_eq!(unknown.err, "no profile named Ghost\n");
    let refused = run(
        PipeAction::Alerts {
            dismiss: Some("settings_invalid::".into()),
            restore: false,
        },
        vec![error(
            ErrorCode::Refused(vec![RefusedProblem::Text(
                "a blocking alert cannot be dismissed".into(),
            )]),
            "",
        )],
        "",
    );
    assert_eq!(refused.code, ExitCode::REFUSED);
    assert!(refused.err.contains("a blocking alert cannot be dismissed"));
    let bad = run(
        PipeAction::Remove { name: "A".into() },
        vec![error(ErrorCode::BadRequest, "cut short")],
        "",
    );
    assert_eq!(bad.code, ExitCode::ERROR);
}

#[test]
fn a_failed_connect_prints_its_step_table() {
    let mut attempt =
        crate::tunnel::ConnectAttempt::new(UNIX_EPOCH, crate::tunnel::AttemptKind::Connect);
    attempt.set(
        crate::tunnel::ConnectStep::Profile,
        crate::tunnel::StepResult::Failed,
        "splitwire.conf is invalid",
    );
    let wire = crate::application::WireAttempts::attempt(&attempt);
    let outcome = run(
        PipeAction::Connect {
            name: "Acme".into(),
        },
        vec![Ok(Response::ConnectFailed {
            name: "Acme".into(),
            attempt: wire,
        })],
        "",
    );
    assert_eq!(outcome.code, ExitCode::ERROR);
    assert!(
        outcome.err.contains("Acme did not connect"),
        "{}",
        outcome.err
    );
    assert!(
        outcome.err.contains("failed at step 1 (profile)"),
        "{}",
        outcome.err
    );
}

#[test]
fn list_prints_every_profile_with_its_state() {
    let mut status = empty_status();
    status.profiles = vec![
        summary(
            "AcmeCorp",
            ProfileState::Connected(crate::tunnel::HandshakeHealth::Ok),
        ),
        summary("Partner", ProfileState::Off),
    ];
    let outcome = run(
        PipeAction::List,
        vec![Ok(Response::Status(Box::new(status)))],
        "",
    );
    assert_eq!(outcome.code, ExitCode::OK);
    let lines: Vec<&str> = outcome.out.lines().collect();
    assert!(lines[0].starts_with("Profile"));
    assert!(lines[1].starts_with("AcmeCorp") && lines[1].contains("connected"));
    assert!(lines[2].starts_with("Partner") && lines[2].contains("off"));
}

#[test]
fn import_previews_then_imports_with_the_proposed_colour() {
    let folder = std::env::temp_dir().join(format!("splitwire-cli-{}", std::process::id()));
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join("Partner.conf");
    fs::write(&file, full_conf()).unwrap();
    let action = |preview, replace| PipeAction::Import {
        files: vec![file.clone()],
        name: None,
        preview,
        replace,
    };
    let ready = preview("Partner", &full_conf());
    assert!(!ready.blocks());

    let only = run(
        action(true, false),
        vec![Ok(Response::ImportPreview(Box::new(ready.clone())))],
        "",
    );
    assert_eq!(only.code, ExitCode::OK);
    assert!(
        only.out
            .starts_with("Partner.conf -> Partner (a new profile"),
        "{}",
        only.out
    );
    assert!(only.out.contains("Ready to import."));
    assert!(!only.out.contains(PRIVATE));
    assert_eq!(only.sent.len(), 1);
    // The request is redacted even in Debug.
    assert!(!only.sent[0].contains(PRIVATE));

    let imported = run(
        action(false, false),
        vec![
            Ok(Response::ImportPreview(Box::new(ready.clone()))),
            Ok(Response::Imported(Box::new(summary(
                "Partner",
                ProfileState::Off,
            )))),
        ],
        "",
    );
    assert_eq!(imported.code, ExitCode::OK, "{}", imported.err);
    assert_eq!(imported.out, "Imported Partner (colour pin 2): off\n");
    assert!(imported.sent[1].starts_with("Import"));
    assert!(imported.sent[1].contains(&format!("colour: {:?}", ready.colour)));

    let broken = preview("Partner", "[Interface]\n");
    let refused = run(
        action(false, false),
        vec![Ok(Response::ImportPreview(Box::new(broken)))],
        "",
    );
    assert_eq!(refused.code, ExitCode::REFUSED);
    assert!(refused.out.contains("Cannot be imported:"));
    assert_eq!(refused.sent.len(), 1);

    let mut existing = ready;
    existing.replace = true;
    let clash = run(
        action(false, false),
        vec![Ok(Response::ImportPreview(Box::new(existing)))],
        "",
    );
    assert_eq!(clash.code, ExitCode::REFUSED);
    assert!(clash.err.contains("--replace"));
    fs::remove_dir_all(&folder).unwrap();

    let missing = run(action(false, false), Vec::new(), "");
    assert_eq!(missing.code, ExitCode::ERROR);
}

#[test]
fn lists_save_sends_stdin_with_the_current_revision() {
    let view = ListsView {
        rev: 42,
        text: "[profile A]\ncorp.com\n".into(),
        files: Vec::new(),
        problems: Vec::new(),
    };
    let printed = run(
        PipeAction::Lists { save: false },
        vec![Ok(Response::Lists(Box::new(view.clone())))],
        "",
    );
    assert_eq!(printed.out, "[profile A]\ncorp.com\n");
    let saved = run(
        PipeAction::Lists { save: true },
        vec![
            Ok(Response::Lists(Box::new(view.clone()))),
            Ok(Response::Lists(Box::new(view))),
        ],
        "[profile A]\nnew.com\n",
    );
    assert_eq!(saved.code, ExitCode::OK);
    assert!(saved.sent[1].contains("base_rev: 42"));
    assert!(saved.sent[1].contains("new.com"));
    let stale = run(
        PipeAction::Lists { save: true },
        vec![
            Ok(Response::Lists(Box::new(ListsView {
                rev: 1,
                text: String::new(),
                files: Vec::new(),
                problems: Vec::new(),
            }))),
            error(ErrorCode::StaleRev, "the lists changed on disk; reloaded"),
        ],
        "",
    );
    assert_eq!(stale.code, ExitCode::REFUSED);
}

#[test]
fn disconnect_all_and_restore_alerts_send_their_targets() {
    let all = run(
        PipeAction::Disconnect {
            target: ProfileTarget::All,
        },
        vec![Ok(Response::Done)],
        "",
    );
    assert_eq!(all.code, ExitCode::OK);
    assert_eq!(all.out, "Every profile is disconnected.\n");
    assert!(all.sent[0].contains("All"));
    let restore = run(
        PipeAction::Alerts {
            dismiss: None,
            restore: true,
        },
        vec![Ok(Response::Done)],
        "",
    );
    assert_eq!(restore.code, ExitCode::OK);
    assert!(restore.sent[0].contains("RestoreAlerts { id: None }"));
}
