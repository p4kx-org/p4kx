// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::CatalogError;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::SourceDisposition;
use crate::mission_index::MissionIndex;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const TEMPLATE: u128 = 0xd101;
const GENERATOR: u128 = 0xd201;

fn template() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("ContractTemplate"))
        .record(TestRecord::new(
            TEMPLATE,
            "ContractTemplate",
            "ContractTemplate.Fixture",
            "libs/foundry/records/custom/excluded/template.xml",
        ))
}

#[test]
fn a_catalog_without_selected_mission_inputs_has_no_fabricated_dependency_reads() -> TestResult {
    let bytes = template()
        .structure(TestStruct::new("ResourceType"))
        .record(TestRecord::new(
            GENERATOR,
            "ResourceType",
            "ResourceType.Fixture",
            "libs/foundry/records/resourcetypedatabase/included/resource.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let target = forge.record(0).ok_or("the template")?.guid();
    let library = MissionIndex::read(&forge, None)?;
    assert!(library.template(target).is_some());
    let mut catalog = MissionIndex::read_catalog(&forge, Some(&filter), None)?;
    assert!(catalog.template(target).is_none());
    let (_, failures, ledger) = catalog.take_template_results();
    assert_eq!(failures, []);
    assert_eq!(ledger.sources, []);
    Ok(())
}

#[test]
fn a_selected_generator_keeps_a_needed_template_outside_the_filter_and_canonical_prefix()
-> TestResult {
    let bytes = template()
        .structure(TestStruct::new("ContractGenerator").field("template", TestType::Reference))
        .record(
            TestRecord::new(
                GENERATOR,
                "ContractGenerator",
                "ContractGenerator.Fixture",
                "libs/foundry/records/contracts/contractgenerator/included/generator.xml",
            )
            .value("template", TestValue::Reference(TEMPLATE)),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let target = forge.record(0).ok_or("the template")?.guid();
    let mut index = MissionIndex::read_catalog(&forge, Some(&filter), None)?;
    assert!(index.template(target).is_some());
    let (_, _, ledger) = index.take_template_results();
    assert!(ledger.sources.iter().any(|source| source.record_index == 0
        && source.role == ExportSourceRole::Dependency
        && source.disposition == SourceDisposition::Projected));
    Ok(())
}

#[test]
fn an_explicitly_selected_template_outside_the_canonical_prefix_keeps_the_index() -> TestResult {
    let bytes = template().build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("custom/excluded");
    let target = forge.record(0).ok_or("the template")?.guid();
    let index = MissionIndex::read_catalog(&forge, Some(&filter), None)?;
    assert!(index.template(target).is_some());
    Ok(())
}

#[test]
fn unknown_selected_metadata_retains_the_full_index_and_selection_failure() -> TestResult {
    let name = "FixtureUnreadableMissionType";
    let mut bytes = DataForgeBuilder::new()
        .structure(TestStruct::new(name))
        .record(TestRecord::new(
            GENERATOR,
            name,
            "FixtureUnknown",
            "libs/foundry/records/custom/included/unknown.xml",
        ))
        .build();
    let position = bytes
        .windows(name.len())
        .position(|bytes| bytes == name.as_bytes())
        .ok_or("the synthetic struct-name token")?;
    *bytes
        .get_mut(position)
        .ok_or("the synthetic struct-name byte")? = 0xff;
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let mut index = MissionIndex::read_catalog(&forge, Some(&filter), None)?;
    let (_, failures, ledger) = index.take_template_results();
    assert_ne!(failures, []);
    assert!(ledger.sources.iter().any(|source| source.record_index == 0
        && source.role == ExportSourceRole::SelectionCandidate
        && source.disposition == SourceDisposition::ReadFailed));
    Ok(())
}

#[test]
fn cancelled_empty_catalog_metadata_cannot_return_a_healthy_empty_index() -> TestResult {
    let bytes = DataForgeBuilder::new().build();
    let forge = DataForge::open(&bytes)?;
    let filter = Filter::new("included");
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        MissionIndex::read_catalog(&forge, Some(&filter), Some(&cancel)),
        Err(CatalogError::Cancelled)
    ));
    Ok(())
}
