//! Every message round-trips; garbage, cut and oversized frames are errors, never panics; and no
//! response can carry key material.

use std::collections::BTreeSet;

use super::samples::{self, at};
use super::*;
use crate::profile::Base64;
use crate::profile::fixtures::{OTHER_PRIVATE, PRESHARED, PRIVATE, full_conf};

fn body(frame: &[u8]) -> &[u8] {
    Frame::body(frame).unwrap()
}

fn request_frames() -> Vec<Vec<u8>> {
    samples::requests()
        .iter()
        .map(|request| request.encode().unwrap())
        .collect()
}

fn response_frames() -> Vec<Vec<u8>> {
    samples::responses()
        .iter()
        .map(|response| response.encode().unwrap())
        .collect()
}

#[test]
fn every_request_round_trips() {
    for request in samples::requests() {
        let frame = request.encode().unwrap();
        assert_eq!(Request::decode(body(&frame)).unwrap(), request);
    }
}

#[test]
fn every_response_round_trips() {
    for response in samples::responses() {
        let frame = response.encode().unwrap();
        assert_eq!(Response::decode(body(&frame)).unwrap(), response);
    }
}

#[test]
fn the_samples_cover_every_message() {
    let tags: BTreeSet<u8> = samples::requests().iter().map(Request::tag).collect();
    // 19 was CancelTimer: retired with the timed connections, never reused.
    assert_eq!(tags, (1..=24).filter(|tag| *tag != 19).collect());
    let tags: BTreeSet<u8> = samples::responses().iter().map(Response::tag).collect();
    assert_eq!(tags, (128..=142).collect());
}

#[test]
fn every_tray_state_and_attempt_kind_round_trips() {
    let Some(Response::Status(status)) = samples::responses().into_iter().nth(1) else {
        panic!("the second sample is a status");
    };
    for tray_state in TrayState::ALL {
        let response = Response::Status(Box::new(StatusReport {
            tray_state,
            ..(*status).clone()
        }));
        let frame = response.encode().unwrap();
        assert_eq!(Response::decode(body(&frame)).unwrap(), response);
    }
    let response = Response::ConnectFailed {
        name: "AcmeCorp".into(),
        attempt: Attempt {
            at: at(1),
            kind: AttemptKind::Recover,
            result: AttemptResult::Succeeded,
            step: None,
            error: None,
            steps: Vec::new(),
        },
    };
    let frame = response.encode().unwrap();
    assert_eq!(Response::decode(body(&frame)).unwrap(), response);
}

#[test]
fn frames_stream_one_after_another() {
    let mut stream = Vec::new();
    for frame in request_frames() {
        Frame::write(&mut stream, &frame).unwrap();
    }
    let mut input = stream.as_slice();
    for request in samples::requests() {
        assert_eq!(
            Request::decode(&Frame::read(&mut input).unwrap()).unwrap(),
            request
        );
    }
    assert!(input.is_empty());
}

#[test]
fn a_response_is_never_read_as_a_request_nor_the_other_way() {
    for frame in response_frames() {
        assert!(matches!(
            Request::decode(body(&frame)),
            Err(WireError::UnknownTag(tag)) if tag >= 128
        ));
    }
    for frame in request_frames() {
        assert!(matches!(
            Response::decode(body(&frame)),
            Err(WireError::UnknownTag(tag)) if tag < 128
        ));
    }
    assert_eq!(Request::decode(b"SW1\x00"), Err(WireError::UnknownTag(0)));
    assert_eq!(
        Request::decode(b"SW1\x13\x04Acme"),
        Err(WireError::UnknownTag(19))
    );
}

#[test]
fn every_cut_message_is_an_error() {
    for frame in request_frames() {
        let body = body(&frame);
        for len in 0..body.len() {
            assert!(
                Request::decode(&body[..len]).is_err(),
                "{len} bytes of {body:?}"
            );
        }
    }
    for frame in response_frames() {
        let body = body(&frame);
        for len in 0..body.len() {
            assert!(Response::decode(&body[..len]).is_err());
        }
    }
}

#[test]
fn a_byte_after_the_message_is_an_error() {
    for frame in request_frames() {
        let mut body = body(&frame).to_vec();
        body.push(0);
        assert_eq!(Request::decode(&body), Err(WireError::Trailing(1)));
    }
    for frame in response_frames() {
        let mut body = body(&frame).to_vec();
        body.push(0);
        assert_eq!(Response::decode(&body), Err(WireError::Trailing(1)));
    }
}

/// Changed bytes decode to an error or to some other valid message, never a panic.
#[test]
fn changed_bytes_never_panic() {
    for frame in request_frames().iter().chain(&response_frames()) {
        let body = body(frame);
        for index in 0..body.len() {
            for change in [0xff, 0x01, 0x80] {
                let mut changed = body.to_vec();
                changed[index] ^= change;
                let _ = Request::decode(&changed);
                let _ = Response::decode(&changed);
            }
        }
    }
}

#[test]
fn random_bytes_never_panic() {
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..20_000 {
        let len = (next() % 64) as usize;
        let mut garbage = b"SW1".to_vec();
        let tag = next() as u8;
        garbage.push(tag);
        garbage.extend((0..len).map(|_| next() as u8));
        let _ = Request::decode(&garbage);
        let _ = Response::decode(&garbage);
    }
}

/// Each made-up secret key as base64 and as its raw bytes.
fn secrets() -> Vec<(&'static str, [u8; 32])> {
    [PRIVATE, PRESHARED, OTHER_PRIVATE]
        .into_iter()
        .map(|key| (key, Base64::decode_key(key).unwrap()))
        .collect()
}

/// Whether `bytes` hold any 8 characters of a key's base64 or 10 of its raw bytes. Ten bytes,
/// because one fixture key is ASCII ("partner-private-…"), which shorter windows of ordinary
/// text would match.
fn holds_key_material(bytes: &[u8]) -> bool {
    secrets().iter().any(|(text, raw)| {
        let found = |window: &[u8]| bytes.windows(window.len()).any(|b| b == window);
        text.as_bytes().windows(8).any(found) || raw.windows(10).any(found)
    })
}

/// The redaction scan: every response, including previews built from `.conf`s full of keys and
/// a profile page mapped from a stored profile, encodes without a trace of a secret key.
#[test]
fn no_response_carries_key_material() {
    for (frame, response) in response_frames().iter().zip(samples::responses()) {
        let debug = format!("{response:?}");
        assert!(!holds_key_material(frame), "{debug}");
        assert!(!holds_key_material(debug.as_bytes()), "{debug}");
    }
    // The scan finds a key where one is: an import request's frame.
    let requests = samples::requests();
    let import = requests
        .iter()
        .find(|r| matches!(r, Request::Import { .. }))
        .unwrap();
    assert!(holds_key_material(&import.encode().unwrap()));
}

/// A `.conf` travels only from a client to the service, in a zeroed, redacted `ConfText`.
#[test]
fn a_conf_in_a_request_is_redacted() {
    for request in samples::requests() {
        let debug = format!("{request:?}");
        assert!(!holds_key_material(debug.as_bytes()), "{debug}");
    }
    let request = Request::Import {
        name: "Partner".into(),
        conf_text: ConfText::new(full_conf()),
        colour: crate::profile::Pin::new(1).unwrap(),
        replace: false,
    };
    let frame = request.encode().unwrap();
    let Request::Import { conf_text, .. } = Request::decode(body(&frame)).unwrap() else {
        panic!("an import");
    };
    assert_eq!(conf_text.expose(), full_conf());
    assert_eq!(
        format!("{conf_text:?}"),
        format!("ConfText({} bytes, redacted)", full_conf().len())
    );
}

/// No response-side type can even name key material: the files that define and encode them do
/// not mention a key-bearing type or a key field, so a `Profile` could not be encoded either.
#[test]
fn no_response_type_names_a_key_bearing_type() {
    let files = [
        ("response.rs", include_str!("response.rs")),
        ("status.rs", include_str!("status.rs")),
        ("profile_summary.rs", include_str!("profile_summary.rs")),
        ("profile_detail.rs", include_str!("profile_detail.rs")),
        ("alert.rs", include_str!("alert.rs")),
        ("attempt.rs", include_str!("attempt.rs")),
        ("lists.rs", include_str!("lists.rs")),
        ("routes.rs", include_str!("routes.rs")),
        ("traffic.rs", include_str!("traffic.rs")),
        ("logs.rs", include_str!("logs.rs")),
        ("settings.rs", include_str!("settings.rs")),
        ("error_reply.rs", include_str!("error_reply.rs")),
        ("profile_wire.rs", include_str!("profile_wire.rs")),
        ("settings_wire.rs", include_str!("settings_wire.rs")),
        ("domain_wire.rs", include_str!("domain_wire.rs")),
    ];
    let names = [
        "ConfText",
        "SecretKey",
        "SecretBytes",
        "WgConf",
        "ParsedConf",
        "private_key",
        "preshared_key",
        "expose",
    ];
    for (file, source) in files {
        for token in source.split(|c: char| !(c.is_alphanumeric() || c == '_')) {
            assert!(!names.contains(&token), "{file} names {token}");
        }
    }
}
