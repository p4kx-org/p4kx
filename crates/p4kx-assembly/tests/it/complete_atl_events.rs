// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::complete_atl_source::{self as source, TestResult};
use p4kx_assembly::audio::{AudioIndex, WwiseEvent};

#[test]
fn atl_complete_text_keeps_all_257_original_events() -> TestResult {
    let names = source::names();
    let [text, _] = source::inputs(&names);
    complete(&text, &names)
}

#[test]
fn atl_complete_cryxml_keeps_all_257_original_events() -> TestResult {
    let names = source::names();
    let [_, binary] = source::inputs(&names);
    complete(&binary, &names)
}

fn complete(bytes: &[u8], names: &[String]) -> TestResult {
    source::with_archive(bytes, names, |archive| {
        let index = AudioIndex::build(archive);
        let trigger = index
            .trigger(source::TRIGGER)
            .ok_or("the indexed original trigger")?;
        let actual: Vec<_> = trigger.events().iter().map(WwiseEvent::name).collect();
        assert_eq!(
            actual, names,
            "complete original order and duplicates survive indexing"
        );
        let last = trigger.events().last().ok_or("the late original event")?;
        assert!(
            last.is_localised(),
            "the final original event retains its flag"
        );
        assert_eq!(
            last.id(),
            p4kx_testkit::wwise_short_id(names.last().ok_or("last original name")?),
            "the event ID still derives from the complete original name",
        );
        assert_eq!(
            index.definition_path(trigger),
            Some(source::PATH),
            "original definition owner"
        );
        assert!(
            index.issues().is_empty(),
            "valid source needs no incomplete-index issue"
        );
        Ok(())
    })
}

#[test]
fn atl_complete_text_search_reaches_the_257th_original_event() -> TestResult {
    let names = source::names();
    let [text, _] = source::inputs(&names);
    late_match(&text, &names)
}

#[test]
fn atl_complete_cryxml_search_reaches_the_257th_original_event() -> TestResult {
    let names = source::names();
    let [_, binary] = source::inputs(&names);
    late_match(&binary, &names)
}

fn late_match(bytes: &[u8], names: &[String]) -> TestResult {
    source::with_archive(bytes, names, |archive| {
        let index = AudioIndex::build(archive);
        let page = index.search(source::LATE, 0, 10);
        assert_eq!(
            page.total(),
            1,
            "an actual late source event participates in search"
        );
        let hit = page.hits().first().ok_or("the matching trigger")?;
        assert_eq!(
            hit.trigger().name(),
            source::TRIGGER,
            "the original trigger wins"
        );
        assert!(
            !hit.matched_by_trigger(),
            "the event, not the trigger name, selected this hit"
        );
        let matched: Vec<_> = hit
            .events()
            .filter(|(_, matched)| *matched)
            .map(|(event, _)| event.event().name())
            .collect();
        assert_eq!(matched, [source::LATE], "only the real late event matched");
        assert_eq!(page.next_offset(), None, "one complete trigger page");
        Ok(())
    })
}

#[test]
fn atl_complete_small_duplicate_empty_and_non_event_controls() -> TestResult {
    let names = vec!["First".to_owned(), "First".to_owned(), "Last".to_owned()];
    for bytes in source::inputs(&names) {
        complete(&bytes, &names)?;
        source::with_archive(&bytes, &names, |archive| {
            let index = AudioIndex::build(archive);
            let silent = index.trigger("Silent").ok_or("the actual empty trigger")?;
            assert!(
                silent.events().is_empty(),
                "an authored empty list remains empty"
            );
            assert_eq!(
                index.search("not_a_wwise_event", 0, 10).total(),
                0,
                "wrong element kinds stay ignored"
            );
            Ok(())
        })?;
    }
    Ok(())
}
