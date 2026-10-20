use std::fmt::Write;

use super::Times;
use crate::ipc::{Alert, ErrorCode, ErrorReply, RefusedProblem, Severity};

/// Error answers and alerts as text: a refusal is printed with its line numbers.
pub struct ReplyText;

impl ReplyText {
    pub fn error(reply: &ErrorReply) -> String {
        let mut text = match &reply.code {
            ErrorCode::BadRequest => format!("error: {}\n", reply.message),
            ErrorCode::UnknownProfile => format!("{}\n", reply.message),
            ErrorCode::StaleRev => format!("refused: {}\n", reply.message),
            ErrorCode::Refused(_) if reply.message.is_empty() => "refused\n".into(),
            ErrorCode::Refused(_) => format!("refused, {}\n", reply.message),
        };
        if let ErrorCode::Refused(problems) = &reply.code {
            for problem in problems {
                let line = match problem {
                    RefusedProblem::List(problem) => problem.to_string(),
                    RefusedProblem::Setting(problem) => problem.to_string(),
                    RefusedProblem::Text(problem) => problem.clone(),
                };
                let _ = writeln!(text, "  {line}");
            }
        }
        text
    }

    /// Active alerts first, dismissed ones marked; `id` is what `--dismiss` takes.
    pub fn alerts(alerts: &[Alert], times: &Times) -> String {
        if alerts.is_empty() {
            return "No alerts.\n".into();
        }
        let mut text = String::new();
        let mut sorted: Vec<&Alert> = alerts.iter().collect();
        sorted.sort_by_key(|a| (a.dismissed, a.severity));
        for alert in sorted {
            let severity = match alert.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "info",
            };
            let mut flags = Vec::new();
            if alert.dismissed {
                flags.push("dismissed");
            }
            if alert.blocking {
                flags.push("blocking");
            }
            let flags = match flags.is_empty() {
                true => String::new(),
                false => format!(" ({})", flags.join(", ")),
            };
            let _ = writeln!(
                text,
                "{severity:<7} {}{flags}\n        {}\n        since {}, id {}",
                alert.title,
                alert.message,
                times.at(alert.since),
                alert.id
            );
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::TextProblem;

    #[test]
    fn a_refusal_lists_its_problems_with_both_numbers() {
        let reply = ErrorReply {
            code: ErrorCode::Refused(vec![RefusedProblem::List(TextProblem {
                text_line: Some(14),
                profile: Some("AcmeCorp".into()),
                file_line: Some(4),
                text: "corp.com".into(),
                reason: "also in Partner line 2: a name belongs to one profile".into(),
                other: Some(("Partner".into(), 2)),
                blocking: true,
            })]),
            message: "nothing was written".into(),
        };
        assert_eq!(
            ReplyText::error(&reply),
            "refused, nothing was written\n  text line 14 (AcmeCorp line 4): also in Partner \
             line 2: a name belongs to one profile\n"
        );
    }
}
