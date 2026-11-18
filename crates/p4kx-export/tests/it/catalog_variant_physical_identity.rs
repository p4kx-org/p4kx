// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, VariantCounts};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

use super::physical_fixture::{self as fixture, Identity, Key, Order};
use crate::commodity_v3_fixture::{self as common, TestResult};

#[test]
fn physical_ship_alias_order_preserves_each_original_variant_key() -> TestResult {
    let actual = [
        observed_variant(Identity::Shared, Key::Different, Order::BaseFirst)?,
        observed_variant(Identity::Shared, Key::Different, Order::AliasFirst)?,
    ];
    assert_eq!(
        actual[0], actual[1],
        "reordering same-GUID physical owners cannot erase a real variant key"
    );
    let expected = common::guid(fixture::BASE);
    for variant in actual {
        assert_eq!(
            variant.as_deref(),
            Some(expected.as_str()),
            "the variant's own implementation still matches the base"
        );
    }
    Ok(())
}

#[test]
fn equal_guid_aliases_with_equal_ship_keys_keep_the_nominal_base_rule() -> TestResult {
    for order in [Order::BaseFirst, Order::AliasFirst] {
        let actual = observed_variant(Identity::Shared, Key::Same, order)?;
        let expected = common::guid(fixture::BASE);
        assert_eq!(
            actual.as_deref(),
            Some(expected.as_str()),
            "the real variant remains linked while same-GUID aliases stay bases"
        );
    }
    Ok(())
}

#[test]
fn distinct_ship_guids_keep_independent_implementation_keys() -> TestResult {
    for order in [Order::BaseFirst, Order::AliasFirst] {
        let actual = observed_variant(Identity::Distinct, Key::Different, order)?;
        let expected = common::guid(fixture::BASE);
        assert_eq!(
            actual.as_deref(),
            Some(expected.as_str()),
            "the original distinct-GUID control has one real variant"
        );
    }
    Ok(())
}

fn observed_variant(identity: Identity, key: Key, order: Order) -> TestResult<Option<String>> {
    let (_folder, archive) = fixture::open(identity, key, order)?;
    check_originals(&archive, identity, order)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_physical_ship(&catalog, fixture::BASE_CLASS, 111.0, 50.0)?;
    let (mass, health) = match key {
        Key::Different => (222.0, 80.0),
        Key::Same => (111.0, 50.0),
    };
    assert_physical_ship(&catalog, fixture::ALIAS_CLASS, mass, health)?;
    check_reference_groups(&archive)?;
    let row = ship(&catalog, fixture::VARIANT_CLASS)?;
    Ok(row
        .get("variantOf")
        .ok_or("actual canonical variantOf")?
        .as_str()
        .map(str::to_owned))
}

fn check_originals(archive: &Archive, identity: Identity, order: Order) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let records: Vec<_> = forge.records().collect();
    let [first, second, _] = records.as_slice() else {
        return Err("three original physical Records are admitted".into());
    };
    let classes = match order {
        Order::BaseFirst => [fixture::BASE_CLASS, fixture::ALIAS_CLASS],
        Order::AliasFirst => [fixture::ALIAS_CLASS, fixture::BASE_CLASS],
    };
    for (record, class) in records.iter().zip(classes) {
        assert_eq!(
            record.name()?,
            format!("EntityClassDefinition.{class}"),
            "original Record order is retained"
        );
    }
    assert_ne!(
        first.index(),
        second.index(),
        "aliases remain physically distinct"
    );
    match identity {
        Identity::Shared => assert_eq!(
            first.guid(),
            second.guid(),
            "the two original GUIDs are equal"
        ),
        Identity::Distinct => assert_ne!(
            first.guid(),
            second.guid(),
            "the control GUIDs are distinct"
        ),
    }
    Ok(())
}

fn check_reference_groups(archive: &Archive) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let groups = p4kx_export::read_variant_groups(archive, &forge, &|| false)
        .ok_or("completed independent physical candidate read")?;
    let base = common::guid(fixture::BASE).parse()?;
    let variant = common::guid(fixture::VARIANT).parse()?;
    assert_eq!(
        groups.base_of(variant),
        Some(base),
        "the real per-record implementation reader retains the variant"
    );
    assert_eq!(
        groups.base_of(base),
        None,
        "same-GUID aliases are not nominal variants of themselves"
    );
    assert_eq!(
        groups.summary().ships,
        VariantCounts {
            groups: 1,
            variants: 1
        },
        "the original group policy has one real variant"
    );
    Ok(())
}

fn assert_physical_ship(catalog: &Value, class: &str, mass: f64, health: f64) -> TestResult {
    let row = ship(catalog, class)?;
    assert_eq!(
        row.get("mass").and_then(Value::as_f64),
        Some(mass),
        "{class} uses its own actual implementation mass"
    );
    assert_eq!(
        row.get("hullHealth").and_then(Value::as_f64),
        Some(health),
        "{class} uses its own actual implementation health"
    );
    assert!(
        row.get("variantOf").is_some_and(Value::is_null),
        "the base or unrelated alias is not a nominal variant of itself"
    );
    Ok(())
}

fn ship<'a>(catalog: &'a Value, class: &str) -> TestResult<&'a Value> {
    let rows = catalog
        .get("ships")
        .and_then(Value::as_array)
        .ok_or("canonical Ship rows")?;
    let mut matches = rows
        .iter()
        .filter(|row| row.pointer("/className/value").and_then(Value::as_str) == Some(class));
    let row = matches.next().ok_or("the selected physical class row")?;
    assert!(
        matches.next().is_none(),
        "the physical class identifies exactly one row"
    );
    Ok(row)
}
