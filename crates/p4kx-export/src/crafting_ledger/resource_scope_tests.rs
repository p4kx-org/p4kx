// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::crafting::ledger::{read, read_catalog};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceDisposition;
use crate::{CatalogError, Kind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const RESOURCE: u128 = 0xc101;
const QUANTIZATION: u128 = 0xc201;
const MISSING: u128 = 0xcfff;

fn resource(path: &str) -> TestRecord {
    TestRecord::new(
        RESOURCE,
        "ResourceType",
        "ResourceType.Fixture",
        format!("libs/foundry/records/resourcetypedatabase/{path}.xml"),
    )
}

#[test]
fn the_library_keeps_unfiltered_link_reads_while_the_catalog_requires_actual_targets() -> TestResult
{
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("ResourceType"))
        .record(resource("excluded/resource"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let library = read(&forge, Some(&filter), None, None)?;
    let catalog = read_catalog(&forge, Some(&filter), None, None)?;
    assert_eq!(library.ledger.sources.len(), 1);
    assert_eq!(catalog.ledger.sources, []);
    assert_eq!(catalog.ledger.field_inspections, []);
    Ok(())
}

#[test]
fn a_selected_resource_dangling_link_keeps_its_actual_read_and_the_existing_counter() -> TestResult
{
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("ResourceType").array(
            "properties",
            TestType::Class("ResourceTypeCraftingData".to_owned()),
        ))
        .structure(TestStruct::new("ResourceTypeCraftingData").field(
            "qualityQuantization",
            TestType::StrongPointer("CraftingQualityQuantization_RecordRef".to_owned()),
        ))
        .structure(
            TestStruct::new("CraftingQualityQuantization_RecordRef")
                .field("qualityQuantizationRecord", TestType::Reference),
        )
        .record(resource("included/resource").value(
            "properties",
            TestValue::Array(vec![
                    TestValue::Struct(
                        TestInstance::new("ResourceTypeCraftingData").value(
                            "qualityQuantization",
                            TestValue::Struct(
                                TestInstance::new("CraftingQualityQuantization_RecordRef").value(
                                    "qualityQuantizationRecord",
                                    TestValue::Reference(MISSING),
                                ),
                            ),
                        ),
                    ),
                ]),
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let library = read(&forge, Some(&filter), None, None)?;
    let catalog = read_catalog(&forge, Some(&filter), None, None)?;
    assert_eq!(
        catalog.data.unresolved_quantization_links,
        library.data.unresolved_quantization_links
    );
    assert_eq!(catalog.data.unresolved_quantization_links, 0);
    let source = catalog
        .ledger
        .sources
        .first()
        .ok_or("the selected resource's actual dependency read")?;
    assert_eq!(source.record_index, 0);
    assert_eq!(source.role, ExportSourceRole::Dependency);
    let proof = catalog
        .ledger
        .field_inspections
        .first()
        .ok_or("the retained source field evidence")?;
    let expected = format!("00000000-0000-0000-0000-{MISSING:012x}");
    assert!(
        proof
            .unresolved_references
            .iter()
            .any(|reference| reference.target_guid.as_deref() == Some(expected.as_str()))
    );
    Ok(())
}

fn quantization(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("CraftingQualityQuantizationRecord").field(
            "qualityQuantization",
            TestType::StrongPointer("CraftingQualityQuantization".to_owned()),
        ))
        .structure(TestStruct::new("CraftingQualityQuantization").array(
            "bands",
            TestType::Class("CraftingQualityQuantizationBand".to_owned()),
        ))
        .structure(
            TestStruct::new("CraftingQualityQuantizationBand")
                .field("start", TestType::Int32)
                .field("end", TestType::Int32)
                .field("mappedValue", TestType::Int32),
        )
        .record(
            TestRecord::new(
                QUANTIZATION,
                "CraftingQualityQuantizationRecord",
                "CraftingQualityQuantizationRecord.Fixture",
                "libs/foundry/records/crafting/qualityquantization/included/fixture.xml",
            )
            .value(
                "qualityQuantization",
                TestValue::Struct(TestInstance::new("CraftingQualityQuantization").value(
                    "bands",
                    TestValue::Array(vec![TestValue::Struct(
                            TestInstance::new("CraftingQualityQuantizationBand")
                                .value("start", TestValue::Int32(0))
                                .value("end", TestValue::Int32(10))
                                .value("mappedValue", TestValue::Int32(5)),
                        )]),
                )),
            ),
        )
}

#[test]
fn a_selected_quantization_keeps_failed_resource_dependencies_outside_the_filter() -> TestResult {
    let bytes = quantization(
        DataForgeBuilder::new()
            .structure(TestStruct::new("ResourceType").declared_size(1))
            .record(resource("excluded/resource")),
    )
    .build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let catalog = read_catalog(&forge, Some(&filter), None, None)?;
    assert_eq!(catalog.data.quality_quantizations.len(), 1);
    assert!(
        catalog
            .ledger
            .sources
            .iter()
            .any(|source| source.record_index == 0
                && source.kind == Some(Kind::QualityQuantizations)
                && source.role == ExportSourceRole::Dependency
                && source.disposition == SourceDisposition::ReadFailed)
    );
    Ok(())
}

#[test]
fn the_target_metadata_inventory_is_cancellable_before_link_selection() -> TestResult {
    let bytes = quantization(DataForgeBuilder::new()).build();
    let forge = DataForge::open(&bytes)?;
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        read_catalog(&forge, None, Some(&cancel), None),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}
