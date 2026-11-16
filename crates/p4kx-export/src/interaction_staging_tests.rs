// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    CancelToken, DataForge, InputScope, SourceInput, SourceLocation, SourceUse, SourceUseRole,
    SourceUses, SourceValue,
};
use p4kx_testkit::{DataForgeBuilder, TestStruct};
use serde_json::json;

use super::{
    ComponentOwner, INTERACTABLE, InteractionConfiguration, InteractionFields, SOURCE_TYPES,
    STATE_MACHINE, UnitSource,
};
use crate::CatalogError;
use crate::catalog_rows::staged::OrderedComponentRows;
use crate::catalog_table::CatalogTable;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn staged_interactions_match_sorted_typed_rows_and_keep_real_use_references() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bytes = database("OtherStructure");
    let forge = DataForge::open(&bytes)?;
    let sources = SourceUses::with_compact_coordinates();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let original = scope.at(
        SourceLocation::DataForge {
            record_index: 1,
            field: "Components[2].StateTypes".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let mut staging = OrderedComponentRows::new(directory.path(), None);
    let positions = [(0, 3, false), (1, 2, true), (1, 2, false), (100, 0, true)];
    for (record, component, present) in positions {
        row(&sources, &original, record, component, present).stage(&mut staging)?;
    }
    let mut rows: Vec<_> = positions
        .into_iter()
        .map(|(record, component, present)| row(&sources, &original, record, component, present))
        .collect();
    rows.sort_by_key(|row| row.owner.key());
    let expected = serde_json::to_vec(&CatalogTable::new(&forge, SOURCE_TYPES, rows))?;
    let staged = staging.finish(&forge, SOURCE_TYPES)?;
    assert_eq!(staged.len(), positions.len(), "every occurrence is staged");
    assert_eq!(
        serde_json::to_vec(&staged)?,
        expected,
        "ordinal order, equal positions, values and exact source references retain their bytes"
    );
    assert!(
        staged.stored_bytes() > 0,
        "nonempty staging reports compressed bytes"
    );
    Ok(())
}

#[test]
fn empty_interaction_staging_reports_original_declaration_availability() -> TestResult {
    let directory = tempfile::tempdir()?;
    for (name, availability) in [(INTERACTABLE, "available"), ("OtherStructure", "absent")] {
        let bytes = database(name);
        let forge = DataForge::open(&bytes)?;
        let staged =
            OrderedComponentRows::new(directory.path(), None).finish(&forge, SOURCE_TYPES)?;
        assert_eq!(
            serde_json::to_value(&staged)?,
            json!({ "availability": availability, "rows": [] }),
            "empty table still reports its declared source family"
        );
        assert_eq!(
            staged.stored_bytes(),
            0,
            "empty staging leaves no compressed rows"
        );
    }
    assert_eq!(
        directory.path().read_dir()?.count(),
        0,
        "empty staging leaves no file"
    );
    Ok(())
}

#[test]
fn cancelled_empty_interaction_staging_does_not_complete() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bytes = database(INTERACTABLE);
    let forge = DataForge::open(&bytes)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            OrderedComponentRows::new(directory.path(), Some(&cancel)).finish(&forge, SOURCE_TYPES),
            Err(CatalogError::Cancelled)
        ),
        "an empty staging run respects cancellation"
    );
    Ok(())
}

fn database(source_type: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new(source_type))
        .build()
}

fn row(
    sources: &SourceUses,
    original: &SourceUse,
    record: u32,
    component: u32,
    present: bool,
) -> InteractionConfiguration<'static> {
    let source_type = sources.value(
        SourceValue::Present(STATE_MACHINE),
        std::slice::from_ref(original),
        true,
    );
    InteractionConfiguration {
        owner: ComponentOwner::fixture(sources, record, component),
        source: UnitSource::fixture(source_type),
        fields: InteractionFields::StateMachine {
            state_types: sources.value(
                if present {
                    SourceValue::Present(Vec::new())
                } else {
                    SourceValue::Absent
                },
                std::slice::from_ref(original),
                true,
            ),
        },
    }
}
