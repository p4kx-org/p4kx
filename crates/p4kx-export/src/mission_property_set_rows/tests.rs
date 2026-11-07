// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use crate::guid_text::CatalogGuid;
use crate::mission_property_set::{
    MissionPropertyDeclaration, MissionPropertyLevel, MissionPropertySet,
};
use crate::template_property::{NumberOption, PropertyValue};

use super::MissionPropertySets;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn set(id: u128, value: PropertyValue<'static>) -> MissionPropertySet<'static> {
    MissionPropertySet {
        guid: CatalogGuid(Guid::from_stored(id.to_le_bytes())),
        record_guid: CatalogGuid(Guid::from_stored(1_u128.to_le_bytes())),
        record_path: "synthetic/generator",
        source_type: "ContractGenerator",
        source_location: "generators[0].contractParams.propertyOverrides".to_owned(),
        level: MissionPropertyLevel::Handler,
        properties: vec![Some(MissionPropertyDeclaration {
            variable: Some("value".to_owned()),
            token: None,
            value_type: Some("Synthetic".to_owned()),
            value: Some(value),
        })],
    }
}

fn boolean(id: u128, value: bool) -> MissionPropertySet<'static> {
    set(id, PropertyValue::Boolean { value: Some(value) })
}

fn float(value: f32) -> MissionPropertySet<'static> {
    set(
        1,
        PropertyValue::Float {
            options: vec![NumberOption {
                display_name: crate::contract::LocalisedText::default(),
                value: Some(value),
                variation: None,
                weighting: None,
                dependent_properties: Vec::new(),
            }],
        },
    )
}

#[test]
fn identical_physical_owners_are_idempotent() -> TestResult {
    let mut first = MissionPropertySets::default();
    let mut second = MissionPropertySets::default();
    first.insert(0, boolean(1, false))?;
    second.insert(1, boolean(1, false))?;
    first.merge(second)?;
    assert_eq!(
        first.ambiguous_owners().count(),
        0,
        "identical aliases are not ambiguous"
    );
    assert_eq!(
        first.into_rows().len(),
        1,
        "the canonical identity stays idempotent"
    );
    Ok(())
}

#[test]
fn incompatible_aliases_quarantine_all_owners_without_losing_other_sets() -> TestResult {
    let mut first = MissionPropertySets::default();
    let mut second = MissionPropertySets::default();
    first.insert(0, boolean(1, false))?;
    first.insert(0, boolean(2, true))?;
    second.insert(1, boolean(1, true))?;
    first.merge(second)?;
    first.insert(2, boolean(1, false))?;
    let owners: Vec<_> = first
        .ambiguous_owners()
        .map(|owner| owner.record_index)
        .collect();
    assert_eq!(
        owners,
        [0, 1, 2],
        "later aliases cannot resurrect a quarantined identity"
    );
    let rows = first.into_rows();
    assert_eq!(rows.len(), 1, "unambiguous source sets remain exported");
    assert_eq!(
        rows[0].guid.0,
        Guid::from_stored(2_u128.to_le_bytes()),
        "only the other identity survives"
    );
    Ok(())
}

#[test]
fn ieee_payloads_are_compared_without_json_float_loss() -> TestResult {
    for (left, right, equal) in [
        (0.0, -0.0, false),
        (f32::INFINITY, f32::NEG_INFINITY, false),
        (f32::NAN, f32::INFINITY, false),
        (
            f32::from_bits(0x7fc0_0001),
            f32::from_bits(0x7fc0_0002),
            false,
        ),
        (
            f32::from_bits(0x7fc0_0001),
            f32::from_bits(0x7fc0_0001),
            true,
        ),
        (1.25, 1.25, true),
    ] {
        let mut sets = MissionPropertySets::default();
        sets.insert(0, float(left))?;
        sets.insert(1, float(right))?;
        assert_eq!(
            sets.ambiguous_owners().count() == 0,
            equal,
            "all f32 payload bits distinguish {:08x} from {:08x}",
            left.to_bits(),
            right.to_bits()
        );
    }
    Ok(())
}

#[test]
fn conflicting_inputs_remain_quarantined_in_either_merge_order() -> TestResult {
    for reversed in [false, true] {
        let mut known = MissionPropertySets::default();
        known.insert(0, boolean(1, false))?;
        known.insert(1, boolean(1, true))?;
        let mut other = MissionPropertySets::default();
        other.insert(2, boolean(1, true))?;
        let result = if reversed {
            other.merge(known)?;
            other
        } else {
            known.merge(other)?;
            known
        };
        assert_eq!(
            result.ambiguous_owners().count(),
            3,
            "all exact physical owners survive the merge"
        );
        assert_eq!(result.into_rows().len(), 0, "no ambiguous row is published");
    }
    Ok(())
}
