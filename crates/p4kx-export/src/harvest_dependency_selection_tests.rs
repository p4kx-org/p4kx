// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, FieldOrigin, SourceInventory, SourceLocation, SourceValueState, SourcedValue,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{EntitySelection, HarvestDependencySelection, choose};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::prepared_definition::PreparedDefinition;
use crate::definition_reference::DefinitionStatus;
use crate::record_observation::RecordObservation;

const PRESET: u128 = 17;
const FALLBACK: u128 = 18;
const OVERRIDE: u128 = 19;
const WRONG_FAMILY: u128 = 20;
const DANGLING: u128 = 99;
const PROVIDER_INDEX: u32 = 5;

fn dataforge(overrides: &[Option<u128>]) -> Vec<u8> {
    let elements = overrides
        .iter()
        .map(|value| {
            let element = TestInstance::new("HarvestableElement")
                .value("harvestable", TestValue::Reference(PRESET));
            let element = match value {
                Some(guid) => element.value("harvestableEntityClass", TestValue::Reference(*guid)),
                None => element,
            };
            TestValue::Struct(element)
        })
        .collect();
    let group = TestInstance::new("HarvestableElementGroup")
        .value("harvestables", TestValue::Array(elements));
    fixture_structures()
        .record(
            TestRecord::new(
                PRESET,
                "HarvestablePreset",
                "HarvestablePreset.Preset",
                "preset.xml",
            )
            .value("entityClass", TestValue::Reference(FALLBACK)),
        )
        .record(TestRecord::new(
            FALLBACK,
            "EntityClassDefinition",
            "Fallback",
            "fallback.xml",
        ))
        .record(TestRecord::new(
            OVERRIDE,
            "EntityClassDefinition",
            "Override",
            "override.xml",
        ))
        .record(TestRecord::new(
            OVERRIDE,
            "EntityClassDefinition",
            "Alias",
            "alias.xml",
        ))
        .record(TestRecord::new(
            WRONG_FAMILY,
            "OtherRecord",
            "Other",
            "other.xml",
        ))
        .record(
            TestRecord::new(21, "HarvestableProviderPreset", "Provider", "provider.xml").value(
                "harvestableGroups",
                TestValue::Array(vec![TestValue::Struct(group)]),
            ),
        )
        .build()
}

fn fixture_structures() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("HarvestableElement")
                .field("harvestable", TestType::Reference)
                .field("harvestableEntityClass", TestType::Reference),
        )
        .structure(TestStruct::new("HarvestableElementGroup").array(
            "harvestables",
            TestType::Class("HarvestableElement".to_owned()),
        ))
        .structure(TestStruct::new("HarvestableProviderPreset").array(
            "harvestableGroups",
            TestType::Class("HarvestableElementGroup".to_owned()),
        ))
        .structure(TestStruct::new("HarvestablePreset").field("entityClass", TestType::Reference))
        .structure(TestStruct::new("EntityClassDefinition").field("marker", TestType::Bool))
        .structure(TestStruct::new("OtherRecord").field("marker", TestType::Bool))
}

type ElementChoices = SourcedValue<Vec<SourcedValue<HarvestDependencySelection>>>;
type GroupChoices = SourcedValue<Vec<SourcedValue<ElementChoices>>>;

fn read_choices<'source>(
    forge: &'source DataForge<'source>,
    projection: &CatalogProjection<'_, 'source>,
) -> TestResult<GroupChoices> {
    let provider = forge.record(PROVIDER_INDEX).ok_or("provider")?;
    let tree = provider.tree()?;
    let admitted = RecordObservation::capture(projection.original(), &provider)?;
    let (selected, _) =
        PreparedDefinition::read(&tree, projection, &admitted, None, |_, owner, _, notes| {
            owner.array(
                "harvestableGroups",
                &["HarvestableElementGroup"],
                notes,
                |group, notes| {
                    group.array(
                        "harvestables",
                        &["HarvestableElement"],
                        notes,
                        |element, notes| choose(element, forge, projection.original(), None, notes),
                    )
                },
            )
        })?;
    Ok(selected.into_definition())
}

#[test]
fn a_resolved_override_uses_the_physically_admitted_alias_and_leaves_the_fallback_body_unused()
-> TestResult {
    test_support::with_projection(&dataforge(&[Some(OVERRIDE)]), |forge, projection| {
        let groups = read_choices(forge, projection)?;
        let choice = groups
            .value()
            .and_then(|groups| groups.first())
            .and_then(|group| group.value())
            .and_then(|elements| elements.value())
            .and_then(|elements| elements.first())
            .and_then(|element| element.value())
            .ok_or("ordered element")?;
        assert_eq!(
            choice.entity_selection.value(),
            Some(&EntitySelection::Override),
            "the actual override controls the result"
        );
        let raw = choice
            .authored_override
            .value()
            .ok_or("authored override")?;
        assert_eq!(
            choice.entity_record_index.value(),
            raw.target_record_index().value(),
            "the selected physical index comes from original Reference admission"
        );
        let guid = raw.guid();
        let guid = *guid.value().ok_or("original target GUID")?;
        assert_eq!(
            forge.records_by_guid(guid).count(),
            2,
            "two physical alias candidates"
        );
        assert_eq!(
            choice.entity_record_index.value().copied(),
            forge.record_by_guid(guid).map(|record| record.index()),
            "same first GUID target as the original reader"
        );
        assert!(
            choice.fallback_entity.is_none(),
            "no fallback body was read"
        );
        let inventory = projection.origins().uses.freeze()?;
        assert!(
            inventory.uses().iter().all(|used| !matches!(
                used.location(),
                SourceLocation::Subresource { namespace, indices, .. }
                    if namespace == "dataforgeValue" && indices.first() == Some(&0)
            )),
            "the unused preset body contributes no claims"
        );
        Ok(())
    })
}

#[test]
fn null_dangling_and_wrong_family_overrides_remain_distinct_in_authored_order() -> TestResult {
    test_support::with_projection(
        &dataforge(&[None, Some(DANGLING), Some(WRONG_FAMILY)]),
        |forge, projection| {
            let groups = read_choices(forge, projection)?;
            let choices = groups
                .value()
                .and_then(|groups| groups.first())
                .and_then(|group| group.value())
                .and_then(|elements| elements.value())
                .ok_or("ordered authored elements")?;
            let [null, dangling, wrong] = choices.as_slice() else {
                return Err("one physical position for each of the three authored elements".into());
            };
            let null = null.value().ok_or("null override element")?;
            let dangling = dangling.value().ok_or("dangling override element")?;
            let wrong = wrong.value().ok_or("wrong-family element")?;
            assert_eq!(
                null.authored_override.state(),
                SourceValueState::Null,
                "first original slot"
            );
            let missing = dangling
                .authored_override
                .value()
                .ok_or("dangling Reference")?;
            assert!(
                matches!(missing.status(), DefinitionStatus::Dangling),
                "second original slot"
            );
            for carried in [null, dangling] {
                assert_fallback(carried)?;
            }
            assert_eq!(
                wrong.entity_record_index.state(),
                SourceValueState::Unsupported,
                "third original slot has wrong family"
            );
            assert_eq!(
                wrong.entity_selection.value(),
                Some(&EntitySelection::Override),
                "never substitute a different chosen body"
            );
            assert!(
                wrong.fallback_entity.is_none(),
                "unexpected nominal type remains visible"
            );

            let inventory = projection.origins().uses.freeze()?;
            assert_dangling_sources(&inventory, dangling.entity_record_index.origin());
            Ok(())
        },
    )
}

fn assert_dangling_sources(inventory: &SourceInventory, origin: &FieldOrigin) {
    assert!(
        origin.source_set_complete(),
        "the verified dangling lookup and fallback are complete"
    );
    assert!(
        origin
            .source_uses_refs()
            .iter()
            .all(|reference| { inventory.uses().iter().any(|used| used.id() == reference) }),
        "every used field origin resolves in the shared registry"
    );
    for expected in [0, u64::from(PROVIDER_INDEX)] {
        assert!(
            inventory.uses().iter().any(|use_row| {
                origin.source_uses_refs().contains(use_row.id())
                    && matches!(use_row.location(),
                        SourceLocation::Subresource { namespace, indices, .. }
                            if namespace == "dataforgeValue" && indices.first() == Some(&expected)
                    )
            }),
            "fallback includes body evidence from both actual selection Records"
        );
    }
}

fn assert_fallback(choice: &HarvestDependencySelection) -> TestResult {
    assert_eq!(
        choice.entity_selection.value(),
        Some(&EntitySelection::PresetFallback),
        "fallback branch"
    );
    assert_eq!(
        choice.preset_record_index.value(),
        Some(&0),
        "exact provider dependency"
    );
    assert!(
        choice.authored_preset.value().is_some(),
        "actual authored preset link"
    );
    let fallback = choice
        .fallback_entity
        .as_ref()
        .and_then(|value| value.value())
        .ok_or("actual fallback link")?;
    assert_eq!(
        choice.entity_record_index.value(),
        fallback.target_record_index().value(),
        "chosen original entity"
    );
    assert_eq!(
        choice.entity_record_index.value(),
        Some(&1),
        "physical fallback entity Index"
    );
    Ok(())
}
