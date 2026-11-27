// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    DataForge, Guid, InputScope, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use p4kx_testkit::{DataForgeBuilder, TestStruct};

use super::{
    BlueprintDefinition, BlueprintRowData, CatalogGuid, SOURCE_TYPES, availability,
    retain_definitions,
};
use crate::blueprint_backfill_source::BlueprintResources;
use crate::blueprint_choice_inputs::BlueprintChoice;
use crate::blueprint_definition::PhysicalBlueprintRef;
use crate::blueprint_presentation::BlueprintPresentation;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::{CapturedNumber, FloatToken, NumberOccurrence};
use crate::catalog_rows::staged::StagedCatalogRows;
use crate::catalog_table::CatalogTable;
use crate::resource_raw_policy::ResourceNumberContract;
use crate::source_value_catalog::{SourceArenaOutput, SourceNode, SourcePayload};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const ORIGINAL_PATH: &str = "libs/foundry/records/crafting/blueprints/crafting/fixture.xml";
const TYPE: &str = "CraftingBlueprintRecord";

#[test]
fn staged_blueprint_definitions_keep_exact_sorted_bytes_and_physical_receipts() -> TestResult {
    let directory = tempfile::tempdir()?;
    let bytes = database(TYPE);
    let forge = DataForge::open(&bytes)?;
    let sources = SourceUses::with_compact_coordinates();
    let scope = sources.observe(SourceInput::Unknown, InputScope::Payload)?;
    let guid = Guid::from_stored([1; 16]);
    let mut stage = StagedCatalogRows::new(directory.path(), None);

    let rows = [(guid, 2), (Guid::default(), 0), (guid, 1)]
        .into_iter()
        .map(|(guid, record)| fixture_row(&sources, &scope, guid, record))
        .collect::<TestResult<Vec<_>>>()?;
    let retained = retain_definitions(rows, |definition| {
        stage.append(definition).map_err(Into::into)
    })?;
    let baseline = [(Guid::default(), 0), (guid, 1), (guid, 2)]
        .into_iter()
        .map(|(guid, record)| fixture_row(&sources, &scope, guid, record).map(|row| row.definition))
        .collect::<TestResult<Vec<_>>>()?;
    let expected = CatalogTable::new(&forge, SOURCE_TYPES, baseline);
    let staged = stage.finish(availability(&forge, retained.len()))?;
    assert_eq!(
        serde_json::to_vec(&staged)?,
        serde_json::to_vec(&expected)?,
        "full raw definitions, original links and duplicate GUID physical order match typed output",
    );
    assert!(staged.stored_bytes() > 0, "the compressed file is counted");
    for (proof, original) in retained.iter().zip(expected.rows) {
        assert_eq!(
            proof.definition.physical_guid, original.physical_guid,
            "health keeps nominal identity"
        );
        assert_eq!(
            proof.definition.physical_path, ORIGINAL_PATH,
            "health keeps physical source path"
        );
        assert_eq!(
            proof.definition.numbers, original.numbers,
            "verification keeps the exact original numeric token"
        );
    }
    Ok(())
}

#[test]
fn staging_no_definitions_keeps_the_exact_family_availability() -> TestResult {
    let directory = tempfile::tempdir()?;
    for name in [TYPE, "OtherDeclaration"] {
        let bytes = database(name);
        let forge = DataForge::open(&bytes)?;
        let staged =
            StagedCatalogRows::new(directory.path(), None).finish(availability(&forge, 0))?;
        let typed = CatalogTable::<BlueprintDefinition<'_>>::new(&forge, SOURCE_TYPES, Vec::new());
        assert_eq!(
            serde_json::to_vec(&staged)?,
            serde_json::to_vec(&typed)?,
            "empty declarations remain available or absent by source"
        );
        assert_eq!(
            staged.stored_bytes(),
            0,
            "an empty table writes no staging bytes"
        );
    }
    Ok(())
}

fn database(name: &str) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new(name))
        .build()
}

fn fixture_row(
    sources: &SourceUses,
    scope: &SourceScope,
    guid: Guid,
    record: u32,
) -> TestResult<BlueprintRowData<BlueprintDefinition<'static>>> {
    let used = scope.at(
        SourceLocation::DataForge {
            record_index: record,
            field: "blueprint".to_owned(),
        },
        SourceUseRole::Payload,
    )?;
    let input_ref = scope.source_ref().clone();
    Ok(BlueprintRowData {
        definition: BlueprintDefinition {
            physical_guid: CatalogGuid(guid),
            physical_path: ORIGINAL_PATH,
            physical_index: record,
            numbers: vec![number(record)],
            input_ref: input_ref.clone(),
            header: header(sources, &used, guid, record),
            source: SourceArenaOutput {
                fields: Vec::new(),
                values: vec![SourceNode {
                    value: present(
                        sources,
                        &used,
                        SourcePayload::Boolean { value: record == 1 },
                    ),
                    original_error: None,
                }],
            },
        },
        resources: BlueprintResources::new(record),
        tiers: Vec::new(),
        presentation: BlueprintPresentation {
            blueprint: PhysicalBlueprintRef {
                input_ref,
                record_index: record,
            },
            name: sources.value(SourceValue::Absent, &[], true),
            category: BlueprintChoice {
                state: SourceValue::Absent,
                used: Vec::new(),
                complete: true,
            },
            produced_item: sources.value(SourceValue::Absent, &[], true),
            first_tier: None,
            ingredients: Vec::new(),
            default_available: sources.value(SourceValue::Present(false), &[], true),
            dismantle: sources.value(SourceValue::Absent, &[], true),
        },
    })
}

fn present<T>(sources: &SourceUses, used: &SourceUse, data: T) -> SourcedValue<T> {
    sources.value(SourceValue::Present(data), std::slice::from_ref(used), true)
}

fn header(
    sources: &SourceUses,
    used: &SourceUse,
    guid: Guid,
    record: u32,
) -> CanonicalRecordHeader<'static> {
    CanonicalRecordHeader {
        guid: present(sources, used, CatalogGuid(guid)),
        record_name: present(sources, used, "CraftingBlueprintRecord.Fixture"),
        class_name: present(sources, used, "Fixture"),
        record_path: present(sources, used, ORIGINAL_PATH),
        record_index: present(sources, used, record),
        source_type: present(sources, used, TYPE),
        file_name: present(sources, used, ORIGINAL_PATH),
        metadata_tag: sources.value(SourceValue::Absent, &[], true),
    }
}

fn number(record_index: u32) -> CapturedNumber {
    CapturedNumber {
        record_index,
        occurrence: NumberOccurrence::BlueprintValue { ordinal: 0 },
        field: "blueprint".to_owned(),
        token: FloatToken::Single(f32::NAN.to_bits()),
        source_type: TYPE.to_owned(),
        contract: ResourceNumberContract::Number,
    }
}
