// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::compare::Class;
use super::fixtures::{self, GUID, OTHER, PublishedFixture, RESOURCE};
use super::inventory::FieldKey;
use super::published::{self, Report};
use super::published_model::{Build, BuildGate, PUBLICATIONS, gate};

#[path = "published_projection_fixture.rs"]
mod fixture;

const SLOTS: &str = "/configurations/weaponActions/rows/0/fireActions/value";

fn reference() -> super::Result<PublishedFixture> {
    let values = vec![
        json!({
            "Item": { "reference": GUID },
            "Raw": { "Entity": { "Components": {
                "SCItemShieldGeneratorParams": { "MaxShieldHealth": 20 }
            }}}
        }),
        json!({
            "Item": { "reference": OTHER },
            "Raw": { "Entity": { "Components": {
                "SCItemWeaponComponentParams": { "fireActions": {
                    "SWeaponActionFireRapidParams": { "fireRate": 60 }
                }}
            }}}
        }),
        json!({ "Item": { "reference": RESOURCE }, "Raw": { "Entity": { "Components": {} } } }),
    ];
    let mut fixture = PublishedFixture::new(values)?;
    fixture.snapshot.fields = super::baseline::raw_mappings();
    fixture.save()?;
    Ok(fixture)
}

fn verify(fixture: &PublishedFixture, catalog: &Value) -> super::Result<Report> {
    published::verify(
        &fixture.load()?,
        catalog,
        &fixtures::build(),
        &[fixtures::PUBLICATION],
        &|| false,
    )
}

#[test]
fn reviewed_raw_fields_join_the_actual_canonical_shield_and_direct_fire_action() -> super::Result<()>
{
    let fixture = reference()?;
    let report = verify(&fixture, &fixture::catalog()?)?;
    assert_eq!(
        report.matched_guids, 3,
        "the Item family includes all owners"
    );
    assert_eq!(
        report.fields.len(),
        2,
        "both owning field maps are exercised"
    );
    for (field, summary) in &report.fields {
        assert_eq!(
            summary.count(Class::Equal),
            1,
            "{field:?} compares one original field: {:?}",
            summary.classes
        );
        assert_eq!(
            summary.count(Class::Missing),
            2,
            "other Items lack the component"
        );
    }
    assert!(
        report.succeeds(),
        "nested rapid actions are outside direct fire slots"
    );
    Ok(())
}

#[test]
fn draft_item_stats_cannot_supply_missing_canonical_component_rows() -> super::Result<()> {
    let fixture = reference()?;
    let mut shield = fixtures::item(GUID, 0, json!(20));
    shield["stats"] = json!({"shield": {"maxHealth": 20}});
    let mut weapon = fixtures::item(OTHER, 1, json!(60));
    weapon["stats"] = json!({"weapon": {"fireActions": [
        {"kind": "SWeaponActionFireRapidParams", "fireRate": 60}
    ]}});
    let obsolete = fixtures::catalog(vec![
        shield,
        weapon,
        fixtures::item(RESOURCE, 2, Value::Null),
    ]);
    let error = verify(&fixture, &obsolete)
        .err()
        .ok_or("missing configurations must fail")?;
    assert!(
        error
            .to_string()
            .contains("canonical shieldGenerators rows"),
        "{error}"
    );
    Ok(())
}

#[test]
fn direct_fire_occurrences_are_preserved_and_bad_slot_indices_fail() -> super::Result<()> {
    let fixture = reference()?;
    let mut catalog = fixture::catalog()?;
    let slots = catalog
        .pointer_mut(SLOTS)
        .and_then(Value::as_array_mut)
        .ok_or("fire slots")?;
    slots.push(slots.get(1).ok_or("direct rapid slot")?.clone());
    let report = verify(&fixture, &catalog)?;
    let rapid = report
        .fields
        .get(&FieldKey::new("SWeaponActionFireRapidParams", "fireRate"))
        .ok_or("rapid field comparison")?;
    assert_eq!(
        rapid.count(Class::Equal),
        1,
        "the published direct occurrence compares"
    );
    assert_eq!(
        rapid.count(Class::P4kxOnly),
        1,
        "the second direct slot stays visible"
    );
    let first = catalog
        .pointer_mut(&format!("{SLOTS}/0"))
        .ok_or("first fire slot")?;
    *first = json!({"state": "present", "value": u32::MAX});
    let error = verify(&fixture, &catalog)
        .err()
        .ok_or("invalid fire slot must fail")?;
    assert!(
        error
            .to_string()
            .contains("invalid authored fire-action node index"),
        "{error}"
    );
    Ok(())
}

#[test]
fn reviewed_13165946_release_remains_bound_to_the_public_version_and_channel() -> super::Result<()>
{
    let mut fixture = reference()?;
    let provenance = &mut fixture.snapshot.provenance;
    provenance.data_revision = "f182b7f111279c9193df81927c9b4dbc8c4f3291".to_owned();
    provenance.published_build = Build {
        game_version: "4.10.3".to_owned(),
        channel: "LIVE".to_owned(),
        change_number: "13165946".to_owned(),
    };
    assert_eq!(
        gate(provenance, &provenance.published_build, PUBLICATIONS),
        BuildGate::SamePublishedBuild,
        "the reviewed publication is registered"
    );
    for actual in [
        Build {
            game_version: "4.10.0".to_owned(),
            ..provenance.published_build.clone()
        },
        Build {
            channel: "PTU".to_owned(),
            ..provenance.published_build.clone()
        },
        Build {
            change_number: "13165947".to_owned(),
            ..provenance.published_build.clone()
        },
    ] {
        assert_eq!(
            gate(provenance, &actual, PUBLICATIONS),
            BuildGate::Mismatched,
            "each actual public Build field must match"
        );
    }
    Ok(())
}

#[path = "published_item_owner_tests.rs"]
mod item_owners;
