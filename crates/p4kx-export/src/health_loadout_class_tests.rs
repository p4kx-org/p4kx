// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceEncryptionState, SourceLocation, SourceUseRole, SourceUses, SourceValueState,
};

use super::health_example_cells::ExampleBodyCells;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::catalog_read::original::ExampleBodyReceipt;
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;

#[path = "health_loadout_class_fixture.rs"]
mod fixture;
#[path = "health_loadout_class_xml_fixture.rs"]
mod xml_fixture;

#[test]
fn class_cycles_merge_actual_referrer_fields_and_keep_first_definition_through_cache_reuse()
-> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(false),
        Vec::new(),
        |_, _, forge, input| {
            let guid = fixture::original(forge)?;
            let locations = fixture::original_class_locations(forge, input)?;
            let body = fixture::body(forge, input, None, guid, 2)?;
            check_retained(input, body, &locations, false)
        },
    )
}

#[test]
fn xml_class_targets_keep_actual_attribute_slots_and_per_datum_encryption_for_text_and_cryxml()
-> TestResult {
    for binary in [false, true] {
        let bytes = xml_fixture::bytes(binary);
        ship_pool_test_input::with_input(
            &fixture::database(true),
            vec![xml_fixture::entry(&bytes)],
            |archive, files, forge, input| {
                archive.set_key(p4kx_testkit::test_key());
                let guid = fixture::original(forge)?;
                let location = xml_fixture::original(archive, &bytes, binary)?;
                let body = fixture::body(forge, input, Some(files), guid, 1)?;
                check_retained(input, body, &[location], true)
            },
        )?;
    }
    Ok(())
}

fn check_retained(
    input: &OriginalCatalogInput<'_, '_>,
    body: ExampleBodyReceipt,
    expected: &[SourceLocation],
    encrypted: bool,
) -> TestResult {
    let retained = RetainedExampleBody::read(body, input)?;
    let uses = &input.origins().uses;
    let texts = input
        .origins()
        .health_texts()
        .seal(uses, None, &retained.texts())?;
    let cells = retained.finish(&texts, uses)?;
    assert_eq!(
        cells.field.cell().state(),
        SourceValueState::Absent,
        "a class target does not invent a field body"
    );
    assert!(
        cells.field.cell().origin().source_uses_refs().is_empty(),
        "the absent field has no phantom input"
    );
    assert_eq!(
        cells.source_type.value().map(crate::HealthText::as_str),
        Some(p4kx_assembly::ENTITY_TYPE),
        "the diagnostic source type comes from the genuine original Root header"
    );
    assert_eq!(
        cells
            .source_type
            .cell()
            .origin()
            .contains_encrypted_source(),
        Some(false),
        "the independent Root type keeps its unencrypted source marker"
    );
    assert_eq!(
        cells.target_guid.origin().contains_encrypted_source(),
        Some(encrypted),
        "the target datum includes the actual class selector independently of its unencrypted GUID payload"
    );
    assert!(
        cells.target_guid.origin().source_set_complete(),
        "all actual Record and XML selector inputs are known"
    );
    check_target_uses(&cells, uses, expected)
}

fn check_target_uses(
    cells: &ExampleBodyCells,
    uses: &SourceUses,
    expected: &[SourceLocation],
) -> TestResult {
    let inventory = uses.freeze()?;
    let contributions: Vec<_> = inventory
        .uses()
        .iter()
        .filter(|used| {
            cells
                .target_guid
                .origin()
                .source_uses_refs()
                .contains(used.id())
        })
        .collect();
    for location in expected {
        assert!(
            contributions
                .iter()
                .any(|used| used.role() == SourceUseRole::Selector && used.location() == location),
            "every original referring class slot retains its own exact selector coordinate"
        );
    }
    let guids: Vec<_> = contributions
        .iter()
        .filter(|used| {
            used.role() == SourceUseRole::Payload
                && matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
            if namespace == "dataforgeRecordHeader" && field == "guid")
        })
        .collect();
    assert_eq!(
        guids.len(),
        1,
        "one actual chosen Record header supplies the merged target GUID"
    );
    assert!(
        matches!(guids.first().ok_or("the genuine GUID payload")?.location(),
        SourceLocation::Subresource { indices, .. } if indices == &[0]),
        "the first physical definition, not its equal later duplicate, supplied the target GUID"
    );
    assert_eq!(
        guids
            .first()
            .ok_or("original GUID input")?
            .effective_encryption(),
        SourceEncryptionState::Unencrypted,
        "the GUID's own physical source remains distinct from encrypted XML selection"
    );
    Ok(())
}

#[path = "health_loadout_class_authority_tests.rs"]
mod authority;
