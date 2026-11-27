// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::EntityNode;
use serde_json::{Value, json};

use crate::complete_loadout_notes_fixture as fixture;
use crate::complete_loadout_notes_source as source;
use crate::exact_audio_assertions;
use crate::support::TestResult;

fn check(output: &Value, original: &EntityNode) -> TestResult {
    assert_eq!(
        output.pointer("/entity/guid"),
        Some(&json!(original.guid().to_string())),
        "same root"
    );
    assert_eq!(
        output.pointer("/entity/issue_count"),
        Some(&json!(0)),
        "notes remain notes"
    );
    assert_eq!(
        output.pointer("/entity/issues"),
        Some(&json!([])),
        "no invented source failure"
    );
    let notes = output
        .pointer("/entity/notes")
        .and_then(Value::as_array)
        .ok_or("all original notes")?;
    assert_eq!(notes.len(), original.notes().len(), "no fixed note prefix");
    for (actual, expected) in notes.iter().zip(original.notes()) {
        assert_eq!(
            actual.get("kind"),
            Some(&json!(expected.kind())),
            "actual original kind"
        );
        assert_eq!(
            actual.get("reason"),
            Some(&json!(expected.to_string())),
            "original reason/order"
        );
    }
    Ok(())
}

#[test]
fn complete_loadout_keeps_the_actual65th_distinct_note() -> TestResult {
    let install = fixture::install(fixture::MANY, false)?;
    let original = source::original(&install, fixture::MANY, false)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok("entity_loadout", &json!({"entity":fixture::CLASS}))?;
    check(&result, &original)
}

#[test]
fn complete_loadout_note_budget_refuses_an_actual_huge65th_note() -> TestResult {
    let install = fixture::install(fixture::MANY, true)?;
    let original = source::original(&install, fixture::MANY, true)?;
    assert!(
        original
            .notes()
            .last()
            .ok_or("original65th note")?
            .to_string()
            .len()
            > 300 << 10,
        "the actual final reason exceeds response admission"
    );
    let mut server = install.serve(&["--archive", "LIVE"])?;
    exact_audio_assertions::too_large(
        &mut server,
        "entity_loadout",
        &json!({"entity":fixture::CLASS}),
    )
}

#[test]
fn complete_loadout_single_note_keeps_existing_classification_and_values() -> TestResult {
    let install = fixture::install(1, false)?;
    let original = source::original(&install, 1, false)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let result = server.ok("entity_loadout", &json!({"entity":fixture::CLASS}))?;
    check(&result, &original)
}
