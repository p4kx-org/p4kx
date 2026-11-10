// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::exact_audio_assertions as check;
use crate::exact_audio_fixture::{self as fixture, Encoding, Texts};
use crate::exact_audio_source;
use crate::support::TestResult;

#[test]
fn exact_audio_search_preserves_whole_names_events_and_entity_identifiers() -> TestResult {
    for encoding in [Encoding::Text, Encoding::CryXml] {
        let texts = Texts::long();
        let install = fixture::install(&texts, encoding)?;
        let original = exact_audio_source::original(&install, &texts)?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let result = server.ok(
            "search_audio_triggers",
            &json!({"search":"Exact_Reusable","entities":1}),
        )?;
        assert_eq!(
            result.get("total"),
            Some(&json!(1)),
            "one original trigger match"
        );
        let hit = check::first(&result)?;
        assert_eq!(
            hit.get("name"),
            Some(&json!(texts.trigger)),
            "full original trigger"
        );
        check::events(hit, &texts)?;
        assert_eq!(
            hit.pointer("/entities/count"),
            Some(&json!(1)),
            "one genuine owning Entity"
        );
        assert_eq!(
            hit.pointer("/entities/listed/0/class_name"),
            Some(&json!(texts.class)),
            "full reusable class"
        );
        assert_eq!(
            hit.pointer("/entities/listed/0/guid"),
            Some(&json!(original.guid)),
            "original identity"
        );
        let event_query = texts.events.first().ok_or("original event")?;
        let by_event = server.ok("search_audio_triggers", &json!({"search":event_query}))?;
        assert_eq!(
            by_event.get("total"),
            Some(&json!(2)),
            "the complete returned event selects both genuine definitions"
        );
    }
    Ok(())
}

#[test]
fn exact_audio_entity_locations_and_resolved_events_remain_complete() -> TestResult {
    let texts = Texts::long();
    let install = fixture::install(&texts, Encoding::CryXml)?;
    let original = exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok("get_entity_sounds", &json!({"entity":original.guid}))?;
    check::identity(
        result.get("entity").ok_or("Entity header")?,
        &original,
        &texts,
    );
    let trigger = check::first(&result)?;
    assert_eq!(
        trigger.get("name"),
        Some(&json!(texts.trigger)),
        "complete source trigger"
    );
    assert_eq!(
        trigger.get("location"),
        Some(&json!(original.location)),
        "entire original field coordinate"
    );
    check::events(trigger, &texts)?;
    let by_name = server.ok("get_entity_sounds", &json!({"entity":texts.class}))?;
    assert_eq!(
        by_name.pointer("/entity/guid"),
        Some(&json!(original.guid)),
        "returned class roundtrip"
    );
    check::resolved(&mut server, &texts)?;
    Ok(())
}

#[test]
fn exact_audio_trigger_above_input_cap_is_returned_whole_then_explicitly_refused() -> TestResult {
    let texts = Texts::long();
    let install = fixture::install(&texts, Encoding::Text)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok("search_audio_triggers", &json!({"search":"Exact_Long"}))?;
    let name = check::first(&result)?
        .get("name")
        .and_then(Value::as_str)
        .ok_or("returned trigger")?;
    assert_eq!(name, texts.later, "original full name above input cap");
    assert!(
        name.len() > 256,
        "the concrete input transport boundary applies"
    );
    let error = server.error("resolve_audio_trigger", &json!({"trigger":name}))?;
    assert!(
        error.contains("trigger") && error.contains("longer than 256 bytes"),
        "explicit input refusal: {error}"
    );
    Ok(())
}

#[test]
fn exact_audio_huge_single_name_is_refused_by_existing_response_admission() -> TestResult {
    let mut texts = Texts::long();
    texts.trigger = format!("Exact_Huge_{}", "x".repeat(300 << 10));
    let install = fixture::install(&texts, Encoding::Text)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::too_large(
        &mut server,
        "search_audio_triggers",
        &json!({"search":"Exact_Huge"}),
    )
}

#[test]
fn exact_audio_retained_event_names_share_the_existing_envelope_budget() -> TestResult {
    let mut texts = Texts::long();
    texts.trigger = "Exact_Aggregate".to_owned();
    texts.events = (0..4)
        .map(|index| format!("Play_{index}_{}", "e".repeat(70 << 10)))
        .collect();
    let install = fixture::install(&texts, Encoding::CryXml)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::too_large(
        &mut server,
        "resolve_audio_trigger",
        &json!({"trigger":texts.trigger}),
    )
}
