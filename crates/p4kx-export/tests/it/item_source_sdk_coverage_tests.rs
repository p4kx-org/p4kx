// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogPublicationMode, ItemClass, Playability};
use p4kx_extraction::{
    Archive, DataForgeEntry, Field, SourceValueState, StructValue, Value as Raw,
};
use serde_json::Value;

use super::fixture::{self, Layout};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn later_unreadable_component_does_not_taint_selected_item_source_coverage() -> TestResult {
    complete_source_with_incomplete_definition(Layout::UnknownSuffix)
}

#[test]
fn unused_invalid_physics_mass_does_not_taint_selected_item_source_coverage() -> TestResult {
    complete_source_with_incomplete_definition(Layout::InvalidMass)
}

fn complete_source_with_incomplete_definition(layout: Layout) -> TestResult {
    let (_folder, archive) = fixture::open(layout, DcbOrigin::Plain, false, false)?;
    check_original(&archive, layout)?;
    equipment::with_item_sources(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one actual selected Item source".into());
            };
            assert_eq!(
                row.source().record_index().value(),
                Some(&0),
                "selected owner"
            );
            for ordinal in [
                row.selection().attachment_component_ordinal(),
                row.selection().physics_component_ordinal(),
                row.selection().purchasable_component_ordinal(),
            ] {
                assert_eq!(ordinal.state(), SourceValueState::Present, "first position");
                assert!(
                    ordinal.origin().source_set_complete(),
                    "first position proof"
                );
            }
            assert_eq!(
                row.item_class().value(),
                Some(&ItemClass::Military),
                "original class"
            );
            assert_eq!(
                row.playability().state(),
                SourceValueState::Absent,
                "original playability"
            );
            assert!(
                row.item_class().origin().source_set_complete(),
                "class source"
            );
            assert!(
                row.playability().origin().source_set_complete(),
                "playability sources"
            );
            let coverage = snapshot.coverage();
            for missing in [
                &coverage.unreadable_headers,
                &coverage.unreadable_bodies,
                &coverage.incomplete_fields,
            ] {
                assert_eq!(
                    missing.as_slice(),
                    &[] as &[u32],
                    "the selected Item source fields are complete"
                );
            }
            Ok(())
        },
    )??;
    incomplete_definition(&archive)
}

#[test]
fn required_playability_input_failures_retain_their_states_in_producer_and_sdk() -> TestResult {
    for (layout, state, wire_state) in [
        (
            Layout::InvalidShopName,
            SourceValueState::Invalid,
            "invalid",
        ),
        (Layout::InvalidName, SourceValueState::Invalid, "invalid"),
        (
            Layout::UnreadableFirst,
            SourceValueState::Unreadable,
            "unreadable",
        ),
    ] {
        let (_folder, archive) = fixture::open(layout, DcbOrigin::Plain, false, false)?;
        check_original(&archive, layout)?;
        let catalog = diagnostic_catalog(&archive)?;
        let [producer] = proof::rows(&catalog, "/items")? else {
            return Err("one actual producer Item".into());
        };
        let producer = proof::cell(producer, "playability", wire_state)?;
        equipment::with_item_sources(
            &archive,
            CatalogRecordSelection::RecordIndex(0),
            None,
            |snapshot| -> TestResult {
                let [row] = snapshot.rows() else {
                    return Err("one actual selected Item source".into());
                };
                assert_eq!(row.playability().state(), state, "required input state");
                assert_eq!(
                    snapshot.coverage().incomplete_fields,
                    [0],
                    "incomplete owner"
                );
                let sdk = serde_json::to_value(row.playability())?;
                for field in [
                    "state",
                    "value",
                    "sourceSetComplete",
                    "containsEncryptedSource",
                ] {
                    assert_eq!(sdk.get(field), producer.get(field), "shared {field}");
                }
                Ok(())
            },
        )??;
        incomplete_definition(&archive)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn invalid_unused_attachment_name_does_not_taint_observed_purchasable_playability() -> TestResult {
    let layout = Layout::InvalidUnusedName;
    let (_folder, archive) = fixture::open(layout, DcbOrigin::Plain, false, false)?;
    check_original(&archive, layout)?;
    let catalog = diagnostic_catalog(&archive)?;
    let [producer] = proof::rows(&catalog, "/items")? else {
        return Err("one actual producer Item".into());
    };
    proof::cell(producer, "playability", "present")?;
    equipment::with_item_sources(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one actual selected Item source".into());
            };
            assert_eq!(
                row.item_class().value(),
                Some(&ItemClass::Military),
                "original class"
            );
            assert_eq!(
                row.playability().value(),
                Some(&Playability::Player),
                "purchase name decides"
            );
            assert!(
                row.item_class().origin().source_set_complete(),
                "class source"
            );
            assert!(
                row.playability().origin().source_set_complete(),
                "playability sources"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                Vec::<u32>::new(),
                "complete selected fields"
            );
            assert_eq!(
                producer
                    .pointer("/playability/value")
                    .and_then(Value::as_str),
                Some("player"),
                "producer uses the same purchase-name rule"
            );
            Ok(())
        },
    )??;
    incomplete_definition(&archive)?;
    proof::closure(&catalog)
}

#[test]
fn earlier_unreadable_selector_still_taints_both_derived_item_source_cells() -> TestResult {
    let (_folder, archive) = fixture::open(Layout::UnknownPrefix, DcbOrigin::Plain, false, false)?;
    equipment::with_item_sources(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one actual selected Item source".into());
            };
            assert_eq!(
                row.item_class().value(),
                Some(&ItemClass::Military),
                "observed class"
            );
            assert_eq!(
                row.playability().state(),
                SourceValueState::Absent,
                "observed playability"
            );
            assert!(
                !row.item_class().origin().source_set_complete(),
                "class depends on the prefix"
            );
            assert!(
                !row.playability().origin().source_set_complete(),
                "playability depends on the prefix"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "incomplete selection"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn incomplete_definition(archive: &Archive) -> TestResult {
    equipment::with_item_definitions(
        archive,
        CatalogRecordSelection::RecordIndex(0),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| {
            assert_eq!(snapshot.rows().len(), 1, "selected Item definition");
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "Definitions keeps its own unavailable or invalid original payload"
            );
        },
    )?;
    Ok(())
}

fn diagnostic_catalog(archive: &Archive) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    Ok(serde_json::from_slice(
        p4kx_export::build_catalog(archive, &options)?.json(),
    )?)
}

fn check_original(archive: &Archive, layout: Layout) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let record = forge.record(0).ok_or("actual original owner")?;
    let tree = record.tree()?;
    let Raw::Array(components) = field(tree.fields(), "Components")? else {
        return Err("original Components array".into());
    };
    check_components(components, layout)
}

fn check_components(components: &[Raw<'_>], layout: Layout) -> TestResult {
    let component = |index| -> TestResult<&StructValue<'_>> {
        let Some(Raw::Struct(value)) = components.get(index) else {
            return Err("original selected component".into());
        };
        Ok(value)
    };
    match layout {
        Layout::UnknownSuffix => check_suffix(components),
        Layout::InvalidMass => {
            let Raw::Struct(physics) = field(component(3)?.fields(), "PhysType")? else {
                return Err("original physics class".into());
            };
            assert!(
                matches!(field(physics.fields(), "Mass")?, Raw::String("not a mass")),
                "physics mass has invalid storage"
            );
        }
        Layout::InvalidShopName => {
            assert!(
                matches!(
                    field(component(4)?.fields(), "displayName")?,
                    Raw::String("opaque shop")
                ),
                "purchase name has invalid storage"
            );
        }
        Layout::InvalidName | Layout::InvalidUnusedName => {
            let Raw::Struct(definition) = field(component(1)?.fields(), "AttachDef")? else {
                return Err("original attachment definition".into());
            };
            let Raw::Struct(localization) = field(definition.fields(), "Localization")? else {
                return Err("original localization class".into());
            };
            assert!(
                matches!(field(localization.fields(), "Name")?, Raw::String("@name")),
                "attachment name has invalid storage"
            );
        }
        Layout::UnreadableFirst => {
            assert!(
                matches!(field(component(1)?.fields(), "AttachDef")?, Raw::Error(_)),
                "attachment definition is unreadable"
            );
        }
        _ => return Err("expected an explicit coverage-control layout".into()),
    }
    Ok(())
}

fn check_suffix(components: &[Raw<'_>]) {
    assert_eq!(components.len(), 6, "original slots");
    assert!(
        matches!(components.last(), Some(Raw::Error(_))),
        "last slot is unreadable"
    );
    for (index, kind) in [
        (1, "SAttachableComponentParams"),
        (3, "SEntityPhysicsControllerParams"),
        (4, "SCItemPurchasableParams"),
    ] {
        assert!(
            matches!(components.get(index), Some(Raw::Struct(value)) if value.type_name() == kind),
            "{kind} precedes the unreadable slot"
        );
    }
}

fn field<'read, 'source>(
    fields: &'read [Field<'source>],
    name: &str,
) -> TestResult<&'read Raw<'source>> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(Field::value)
        .ok_or_else(|| format!("original field {name}").into())
}
