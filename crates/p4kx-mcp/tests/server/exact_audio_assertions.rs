// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::exact_audio_fixture::Texts;
use crate::exact_audio_source::Original;
use crate::support::{Server, TestResult};

pub(crate) fn first(search: &Value) -> TestResult<&Value> {
    search
        .get("triggers")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or_else(|| "actual returned trigger".into())
}

pub(crate) fn events(value: &Value, texts: &Texts) -> TestResult {
    let events = value
        .get("events")
        .and_then(Value::as_array)
        .ok_or("returned events")?;
    assert_eq!(events.len(), texts.events.len(), "retained event count");
    for (actual, expected) in events.iter().zip(&texts.events) {
        assert_eq!(
            actual.get("name"),
            Some(&json!(expected)),
            "complete original event name"
        );
    }
    Ok(())
}

pub(crate) fn identity(value: &Value, original: &Original, texts: &Texts) {
    for (name, expected) in [
        ("guid", &original.guid),
        ("name", &original.name),
        ("record_path", &original.path),
        ("class_name", &texts.class),
    ] {
        assert_eq!(
            value.get(name),
            Some(&json!(expected)),
            "exact original Entity {name}"
        );
    }
}

pub(crate) fn too_large(server: &mut Server, tool: &str, arguments: &Value) -> TestResult {
    let error = server.error(tool, arguments)?;
    assert!(
        error.contains("response cap of 262144 bytes"),
        "explicit response admission: {error}"
    );
    assert!(
        error.len() < 1024,
        "response-refusal metadata does not clone a huge name"
    );
    Ok(())
}

pub(crate) fn resolved(server: &mut Server, texts: &Texts) -> TestResult {
    assert_eq!(
        texts.trigger.len(),
        256,
        "original trigger is exactly at unchanged input cap"
    );
    let sounds = server.ok("resolve_audio_trigger", &json!({"trigger":texts.trigger}))?;
    events(sounds.get("trigger").ok_or("resolved header")?, texts)?;
    assert_eq!(
        sounds.pointer("/variants/0/event"),
        Some(&json!(texts.events.first().ok_or("event")?)),
        "complete original variant event"
    );
    assert_eq!(
        sounds.pointer("/breaks/0/event"),
        Some(&json!(texts.events.get(1).ok_or("broken")?)),
        "complete original broken-link event"
    );
    assert_eq!(
        sounds.pointer("/variants/0/soundbank/path"),
        Some(&json!(texts.bank_path())),
        "complete original soundbank path"
    );
    let objects = server.ok(
        "list_soundbank_objects",
        &json!({"soundbank":texts.bank_path()}),
    )?;
    assert!(
        objects
            .get("total")
            .and_then(Value::as_u64)
            .is_some_and(|count| count > 0),
        "returned full path selects actual bank"
    );
    Ok(())
}
