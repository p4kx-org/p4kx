// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::AudioIndex;
use p4kx_extraction::Archive;
use serde_json::{Value, json};

use crate::exact_audio_assertions as check;
use crate::exact_audio_fixture::{self as fixture, Encoding, Texts};
use crate::exact_audio_source;
use crate::support::{Install, TestResult};

const EVENTS: usize = 26;
const QUERY: &str = "TailOnlyTarget";

fn texts() -> Texts {
    let mut texts = Texts::long();
    texts.events = (0..EVENTS)
        .map(|ordinal| {
            if ordinal == EVENTS - 1 {
                format!("Play_{QUERY}")
            } else {
                format!("Play_Complete_{ordinal:02}")
            }
        })
        .collect();
    texts
}

fn original(install: &Install, texts: &Texts) -> TestResult<exact_audio_source::Original> {
    let original = exact_audio_source::original(install, texts)?;
    let archive = Archive::open(install.archive())?;
    let index = AudioIndex::build(&archive);
    let found = index.search(QUERY, 0, 100);
    assert_eq!(found.total(), 1, "one trigger matches only its final event");
    let hit = found.hits().first().ok_or("original selected trigger")?;
    assert_eq!(
        hit.trigger().name(),
        texts.trigger,
        "original selected definition"
    );
    assert!(!hit.matched_by_trigger(), "the trigger name does not match");
    let events: Vec<_> = hit.events().collect();
    assert_eq!(events.len(), EVENTS, "all original matcher decisions exist");
    for (ordinal, (event, matched)) in events.iter().enumerate() {
        assert_eq!(
            event.event().name(),
            texts.events.get(ordinal).ok_or("original event name")?,
            "original event ordinal"
        );
        assert_eq!(
            *matched,
            ordinal == EVENTS - 1,
            "only the actual26th event matches"
        );
    }
    Ok(original)
}

#[test]
fn complete_audio_search_retains_the_actual_matching_event_beyond25() -> TestResult {
    for encoding in [Encoding::Text, Encoding::CryXml] {
        let texts = texts();
        let install = fixture::install(&texts, encoding)?;
        original(&install, &texts)?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let result = server.ok("search_audio_triggers", &json!({"search":QUERY}))?;
        assert_eq!(
            result.get("total"),
            Some(&json!(1)),
            "same actual selection"
        );
        let hit = check::first(&result)?;
        check::events(hit, &texts)?;
        assert_eq!(
            hit.get("matched_trigger"),
            Some(&json!(false)),
            "not a name match"
        );
        let events = hit
            .get("events")
            .and_then(Value::as_array)
            .ok_or("complete events")?;
        for (ordinal, event) in events.iter().enumerate() {
            assert_eq!(
                event.get("matched"),
                Some(&json!(ordinal == EVENTS - 1)),
                "actual matcher decision at original ordinal"
            );
        }
        assert!(result.get("next_cursor").is_none(), "one complete trigger");
    }
    Ok(())
}

#[test]
fn complete_audio_entity_retains_all_original_trigger_events() -> TestResult {
    let texts = texts();
    let install = fixture::install(&texts, Encoding::CryXml)?;
    let source = original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok("get_entity_sounds", &json!({"entity":source.guid}))?;
    check::identity(
        result.get("entity").ok_or("original Entity")?,
        &source,
        &texts,
    );
    check::events(check::first(&result)?, &texts)
}

#[test]
fn complete_audio_resolve_retains_all_original_trigger_events() -> TestResult {
    let texts = texts();
    let install = fixture::install(&texts, Encoding::Text)?;
    original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::resolved(&mut server, &texts)
}
