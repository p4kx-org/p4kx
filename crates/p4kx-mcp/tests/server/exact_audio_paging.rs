// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::exact_audio_assertions as check;
use crate::exact_audio_fixture::{self as fixture, Encoding, Texts};
use crate::exact_audio_source;
use crate::support::TestResult;

#[test]
fn exact_audio_paging_keeps_full_trigger_names_in_original_order() -> TestResult {
    let mut texts = Texts::long();
    texts.trigger = format!("Exact_Page_0_{}", "a".repeat(140 << 10));
    texts.later = format!("Exact_Page_1_{}", "b".repeat(140 << 10));
    let install = fixture::install(&texts, Encoding::Text)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok("search_audio_triggers", &json!({"search":"Exact_Page"}))?;
    assert_eq!(
        first.get("total"),
        Some(&json!(2)),
        "both original triggers count"
    );
    assert_eq!(
        first
            .get("triggers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "one complete name fits per page"
    );
    assert_eq!(
        check::first(&first)?.get("name"),
        Some(&json!(texts.trigger)),
        "first full original name"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("next cursor")?;
    let second = server.ok(
        "search_audio_triggers",
        &json!({"search":"Exact_Page","cursor":cursor}),
    )?;
    assert_eq!(
        second
            .get("triggers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "remaining original item"
    );
    assert_eq!(
        check::first(&second)?.get("name"),
        Some(&json!(texts.later)),
        "next original ordinal"
    );
    assert!(
        second.get("next_cursor").is_none(),
        "the final original item completes paging"
    );
    Ok(())
}

#[test]
fn exact_audio_later_oversized_trigger_preserves_prefix_and_retry() -> TestResult {
    let mut texts = Texts::long();
    texts.later = format!("Exact_Later_{}", "z".repeat(300 << 10));
    let install = fixture::install(&texts, Encoding::CryXml)?;
    exact_audio_source::original(&install, &texts)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok("search_audio_triggers", &json!({"search":"Exact_"}))?;
    assert_eq!(
        first
            .get("triggers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "retained complete prefix"
    );
    assert_eq!(
        check::first(&first)?.get("name"),
        Some(&json!(texts.trigger)),
        "the original first item remains"
    );
    let cursor = first
        .get("next_cursor")
        .and_then(Value::as_str)
        .ok_or("exact continuation")?;
    check::too_large(
        &mut server,
        "search_audio_triggers",
        &json!({"search":"Exact_","cursor":cursor}),
    )
}

#[test]
fn exact_audio_the_actual_huge26th_event_is_charged() -> TestResult {
    let mut texts = Texts::long();
    texts.events = (0..25).map(|index| format!("Play_Short_{index}")).collect();
    let huge = "H".repeat(300 << 10);
    texts.events.push(huge.clone());
    texts.later = format!("Exact_Unshown_{huge}");
    let install = fixture::install(&texts, Encoding::Text)?;
    exact_audio_source::original(&install, &texts)?;
    assert_eq!(
        texts.events.get(25),
        Some(&huge),
        "the actual26th event is huge"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    check::too_large(
        &mut server,
        "search_audio_triggers",
        &json!({"search":"Exact_","limit":1}),
    )
}

#[test]
fn exact_audio_later_unmaterialized_trigger_is_not_charged_after_complete_events() -> TestResult {
    let mut texts = Texts::long();
    texts.events = (0..26).map(|index| format!("Play_Short_{index}")).collect();
    let huge = "H".repeat(300 << 10);
    texts.later = format!("Exact_Unshown_{huge}");
    let install = fixture::install(&texts, Encoding::Text)?;
    exact_audio_source::original(&install, &texts)?;
    assert!(
        texts.later.len() > 300 << 10,
        "the actual later trigger is huge"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.ok(
        "search_audio_triggers",
        &json!({"search":"Exact_","limit":1}),
    )?;
    let hit = check::first(&first)?;
    assert_eq!(
        hit.get("events_total"),
        Some(&json!(26)),
        "full actual event count"
    );
    check::events(hit, &texts)?;
    assert_eq!(
        first
            .get("triggers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "only requested item materialized"
    );
    assert!(
        first.get("next_cursor").and_then(Value::as_str).is_some(),
        "later original trigger remains"
    );
    Ok(())
}
