// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole, SourceValueState};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{MineableElement, read};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_projection::CatalogProjection;
use crate::definition_reference::DefinitionStatus;
use crate::record_observation::RecordObservation;

const RESOURCE: u128 = 9;
const NAN_BITS: u32 = 0x7fc0_0003;

fn dataforge() -> Vec<u8> {
    let resource = TestStruct::new("ResourceType").field("marker", TestType::Bool);
    let element = TestStruct::new("MineableElement")
        .field(
            "resourceType",
            TestType::ReferenceTo("ResourceType".to_owned()),
        )
        .field("elementInstability", TestType::Single)
        .field("elementResistance", TestType::Single);
    let element_record = |guid, reference| {
        TestRecord::new(
            guid,
            "MineableElement",
            "MineableElement.Proof",
            "libs/foundry/records/mining/mineableelements/proof.xml",
        )
        .value("resourceType", TestValue::Reference(reference))
        .value(
            "elementInstability",
            TestValue::Single(f32::from_bits(NAN_BITS)),
        )
        .value("elementResistance", TestValue::Single(-0.0))
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(resource)
        .structure(element)
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "ResourceType.Proof",
            "proof.xml",
        ))
        .record(element_record(10, RESOURCE))
        .record(element_record(11, 99))
        .build()
}

#[test]
fn numeric_receipts_keep_source_bits_and_legacy_values() -> TestResult {
    test_support::with_projection(&dataforge(), |forge, projection| {
        let record = forge.record(1).ok_or("record")?;
        let tree = record.tree()?;
        let admitted = RecordObservation::capture(projection.original(), &record)?;
        let (row, notes) = read(&tree, projection, &admitted, None)?;
        let resource = forge.record(0).ok_or("target Record")?;
        assert!(notes.is_empty(), "valid declarations and storage");
        assert_eq!(
            row.resource_type_guid(),
            Some(resource.guid()),
            "same direct target"
        );
        assert!(
            row.instability
                .value()
                .and_then(super::ResourceFloat32::finite)
                .is_none(),
            "legacy nonfinite tuning is null"
        );
        assert_eq!(
            row.resistance
                .value()
                .and_then(super::ResourceFloat32::finite)
                .map(f32::to_bits),
            Some((-0.0_f32).to_bits()),
            "legacy signed zero"
        );
        assert_eq!(
            row.instability
                .value()
                .map(super::ResourceFloat32::raw_bits),
            Some(NAN_BITS),
            "original NaN bits"
        );
        assert_eq!(
            row.cluster_factor.state(),
            SourceValueState::Absent,
            "a missing declaration retains an explicit absence state"
        );
        check_instability_origin(&row, projection, record.index())?;
        let wire = serde_json::to_value(&row)?;
        assert_eq!(wire["guid"]["state"], "present", "direct sourced identity");
        assert!(
            wire.get("sourceValues").is_none(),
            "one representation of each datum"
        );
        assert!(
            wire.get("resourceTypeGuid").is_none(),
            "resource link goes through the single sourced carrier"
        );
        assert!(
            wire["instability"]["value"]["finite"].is_null(),
            "legacy wire value is still null"
        );
        assert_eq!(
            wire["instability"]["value"]["rawBits"], "7fc00003",
            "the raw bits are protected on the direct output field"
        );
        Ok(())
    })
}

fn check_instability_origin(
    row: &MineableElement,
    projection: &CatalogProjection<'_, '_>,
    record_index: u32,
) -> TestResult {
    let evidence = row.instability.origin();
    let inventory = projection.origins().uses.freeze()?;
    assert!(evidence.source_set_complete(), "known field evidence");
    assert!(
        inventory.uses().iter().any(|used| {
            evidence.source_uses_refs().contains(used.id())
                && used.role() == SourceUseRole::Payload
                && matches!(
                    used.location(),
                    SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "dataforgeValue"
                            && indices.first() == Some(&u64::from(record_index))
                            && field == "$value"
                )
        }),
        "the raw scalar uses its actual physical Record and original field occurrence"
    );
    Ok(())
}

#[test]
fn dangling_resource_keeps_original_guid_and_legacy_null() -> TestResult {
    test_support::with_projection(&dataforge(), |forge, projection| {
        let record = forge.record(2).ok_or("record")?;
        let tree = record.tree()?;
        let admitted = RecordObservation::capture(projection.original(), &record)?;
        let (row, _) = read(&tree, projection, &admitted, None)?;
        assert!(
            row.resource_type_guid().is_none(),
            "legacy dangling link remains null"
        );
        let link = row.resource_type.value().ok_or("original Reference")?;
        assert!(
            matches!(link.status(), DefinitionStatus::Dangling),
            "missing target is explicit"
        );
        let guid = link.guid();
        assert_eq!(
            guid.value().map(ToString::to_string).as_deref(),
            Some("00000000-0000-0000-0000-000000000063"),
            "the authored dangling GUID is preserved"
        );
        assert!(
            !guid.origin().source_uses_refs().is_empty(),
            "the original dangling GUID has evidence"
        );
        Ok(())
    })
}
