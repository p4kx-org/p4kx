// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::{DataForge, Guid};

use crate::{
    Entities, EntityNode, LoadoutClassName, LoadoutError, LoadoutResolver, LoadoutTargetGuid,
};

use super::test_source as source;
use source::{Recorder, Stop, TestResult};

#[test]
fn record_class_getters_and_same_guid_referrers_keep_selected_record_and_cache_identity()
-> TestResult {
    let bytes = source::database();
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first original referrer")?;
    let second = forge.record(1).ok_or("second original referrer")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the real GUIDs intentionally coincide"
    );
    assert_ne!(
        first.index(),
        second.index(),
        "physical original referrers remain different"
    );
    let first_tree = first.tree()?;
    let second_tree = second.tree()?;
    source::original(&first_tree);
    source::original(&second_tree);
    let recorder = Arc::new(Recorder::new(&forge, Stop::Never));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder.clone());
    let first_node = resolver.resolve_tree(&first, &first_tree)?;
    let second_node = resolver.resolve_tree(&second, &second_tree)?;
    let again = resolver.resolve(&first)?;
    let log = recorder.log.lock().map_err(|_| "source log")?;
    record_results(&first_node, &second_node, &again, &log, first.guid())?;
    drop(log);
    Ok(())
}

fn record_results(
    first_node: &EntityNode,
    second_node: &EntityNode,
    again: &EntityNode,
    log: &source::Log,
    selected: Guid,
) -> TestResult {
    assert_eq!(
        first_node, again,
        "cache reuse changes no semantic node or Error equality"
    );
    assert_eq!(
        log.getters.len(),
        4,
        "each of four real slots is copied only on its original projection"
    );
    assert_eq!(
        log.chosen.len(),
        6,
        "every genuine attachment observes its chosen Record, also on cache reuse"
    );
    assert!(
        log.chosen
            .iter()
            .all(|chosen| chosen.record_index == 0 && chosen.guid == selected),
        "all selected callbacks identify the actual first physical definition"
    );
    assert_eq!(
        log.getters
            .iter()
            .map(|getter| getter.record_index)
            .collect::<Vec<_>>(),
        [0, 0, 1, 1],
        "nominally equal getters preserve both physical referrers"
    );
    verify_pairs(first_node, 0, log, selected)?;
    verify_pairs(second_node, 2, log, selected)?;
    assert_ne!(
        log.getters.first().ok_or("first getter")?.selection,
        log.getters
            .get(2)
            .ok_or("second referrer getter")?
            .selection,
        "equal nominal GUID/text cannot collapse different referrer frames"
    );
    Ok(())
}

fn verify_pairs(node: &EntityNode, offset: usize, log: &source::Log, selected: Guid) -> TestResult {
    let unresolved: Vec<_> = node.unresolved().collect();
    assert_eq!(
        unresolved.len(),
        2,
        "both original class cycles survive as unresolved attachments"
    );
    for (slot, (_, unresolved)) in unresolved.into_iter().enumerate() {
        let LoadoutError::Cycle { guid } = unresolved.error() else {
            return Err("actual class cycle".into());
        };
        let original = &log
            .getters
            .get(offset + slot)
            .ok_or("original class getter")?
            .selection;
        assert_eq!(
            guid.class_selection(),
            Some(original),
            "each failure keeps its actual referring getter"
        );
        assert!(
            guid.selection().is_none(),
            "class selection is independent of authored Reference origin"
        );
        assert_eq!(
            guid.guid(),
            selected,
            "the independently selected physical Record supplied the GUID"
        );
        assert_eq!(
            guid,
            &LoadoutTargetGuid::unbound(selected),
            "evidence does not alter GUID equality"
        );
    }
    Ok(())
}

#[test]
fn record_class_observer_refusals_stop_before_later_slots_or_selected_record_reuse() -> TestResult {
    for stop in [Stop::Getter, Stop::Chosen] {
        let bytes = source::database();
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(0).ok_or("original class owner")?;
        let tree = record.tree()?;
        source::original(&tree);
        let recorder = Arc::new(Recorder::new(&forge, stop));
        let mut resolver = LoadoutResolver::with_observed_entities(
            Arc::new(Entities::new(&forge)),
            recorder.clone(),
        );
        assert!(
            matches!(
                resolver.resolve_tree(&record, &tree),
                Err(LoadoutError::ObservationStopped)
            ),
            "a refusal cannot publish a complete prefix"
        );
        assert!(
            matches!(
                resolver.resolve(&record),
                Err(LoadoutError::ObservationStopped)
            ),
            "the stopped state survives reuse"
        );
        let log = recorder.log.lock().map_err(|_| "source log")?;
        let (getters, chosen) = match stop {
            Stop::Getter => (1, 0),
            _ => (2, 1),
        };
        assert_eq!(
            (log.getters.len(), log.chosen.len()),
            (getters, chosen),
            "later operations are not delivered after refusal"
        );
    }
    Ok(())
}

#[test]
fn equal_class_text_copies_cannot_mint_selection_and_nested_guid_errors_keep_their_source()
-> TestResult {
    let bytes = source::database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("original owner")?;
    let tree = record.tree()?;
    source::original(&tree);
    let recorder = Arc::new(Recorder::new(&forge, Stop::Never));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), recorder);
    let node = resolver.resolve_tree(&record, &tree)?;
    let (_, unresolved) = node.unresolved().next().ok_or("actual class cycle")?;
    let LoadoutError::Cycle { guid } = unresolved.error() else {
        return Err("actual cycle".into());
    };
    let copied = LoadoutClassName::unbound("Self");
    assert!(
        copied.selection().is_none(),
        "equal actual text alone cannot create an authored class operation"
    );
    let other = "00000000-0000-0000-0000-000000000009".parse()?;
    let nested = LoadoutError::UnreadableRecord {
        guid: LoadoutTargetGuid::unbound(other),
        reason: "actual nested failure".to_owned(),
    };
    assert_eq!(
        nested.clone().with_target(guid, None),
        nested,
        "a different immediate target cannot relabel a nested failure"
    );
    let unbound = LoadoutTargetGuid::unbound(guid.guid());
    assert!(
        unbound.selection().is_none() && unbound.class_selection().is_none(),
        "a naked copied GUID has neither original authority"
    );
    assert_eq!(
        unbound.to_string(),
        guid.to_string(),
        "Display remains value-only"
    );
    Ok(())
}
