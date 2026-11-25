// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use super::{
    Archive, CatalogLanguageSelection, CatalogOptions, CatalogRecordSelection, Corrupt,
    EXPORTED_AT, Playability, TestResult, UNIX_EPOCH, build_catalog, write_archive_with,
};
use crate::health_origin_assertions as proof;
use crate::playability_fixture::write_archive_with_crew_alias;

#[test]
fn ship_playability_sdk_and_catalog_share_physical_lookup_and_source_evidence() -> TestResult {
    let directory = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        write_archive_with(directory.path(), Corrupt::Nothing)?,
        Some(p4kx_testkit::test_key()),
    )?;
    let catalog = catalog(&archive, CatalogPublicationMode::Normal)?;
    for (class, expected, dependencies) in [
        ("Ship_Crewed", Playability::Ai, 2),
        ("Ship_Salvage", Playability::Mission, 2),
        ("Ship_Player", Playability::Player, 1),
    ] {
        compare_present(&archive, &catalog, class, expected, true, dependencies)?;
    }

    let row = ship(&catalog, "Ship_Other_Inclusion")?;
    let carrier = proof::cell(row, "playability", "absent")?;
    assert_eq!(
        carrier.get("sourceSetComplete"),
        Some(&json!(true)),
        "an exhausted classification leaves an observed absence"
    );
    assert_eq!(
        carrier.get("containsEncryptedSource"),
        Some(&json!(true)),
        "classification absence depends on the encrypted source predicates"
    );
    assert_lookup(&proof::uses(&catalog, carrier)?);
    compare_sdk(&archive, &catalog, index(row)?, carrier, &BTreeSet::new())?;
    proof::closure(&catalog)
}

#[test]
fn ship_playability_sdk_preserves_each_reached_unreadable_dependency() -> TestResult {
    for (corrupt, class, dependencies) in [
        (Corrupt::Manifests, "Ship_Crewed", 1),
        (Corrupt::Tags, "Ship_Salvage", 2),
    ] {
        let directory = tempfile::tempdir()?;
        let archive = Archive::open_with_key(
            write_archive_with(directory.path(), corrupt)?,
            Some(p4kx_testkit::test_key()),
        )?;
        let catalog = catalog(&archive, CatalogPublicationMode::DiagnosticPartial)?;
        let row = ship(&catalog, class)?;
        let carrier = proof::cell(row, "playability", "unreadable")?;
        assert_eq!(
            carrier.get("sourceSetComplete"),
            Some(&json!(false)),
            "an unreadable reached body leaves classification evidence incomplete"
        );
        assert_eq!(
            carrier.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the attempted body retains the known encryption of its Archive entry"
        );
        let uses = proof::uses(&catalog, carrier)?;
        assert_lookup(&uses);
        let failed = attempted_dependencies(&catalog, &uses)?;
        assert_eq!(
            failed.len(),
            dependencies,
            "each corrupt linked Record has a distinct attempted coordinate"
        );
        compare_sdk(&archive, &catalog, index(row)?, carrier, &failed)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn ship_playability_sdk_uses_both_physical_aliases_for_plain_and_encrypted_entries() -> TestResult {
    for encrypted in [false, true] {
        let directory = tempfile::tempdir()?;
        let archive = Archive::open_with_key(
            write_archive_with_crew_alias(directory.path(), encrypted)?,
            encrypted.then(p4kx_testkit::test_key),
        )?;
        let catalog = catalog(&archive, CatalogPublicationMode::Normal)?;
        compare_present(
            &archive,
            &catalog,
            "Ship_Empty_Manifest",
            Playability::Ai,
            encrypted,
            3,
        )?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn catalog(
    archive: &Archive,
    publication_mode: CatalogPublicationMode,
) -> Result<Value, Box<dyn std::error::Error>> {
    let options = CatalogOptions {
        publication_mode,
        ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
    };
    Ok(serde_json::from_slice(
        build_catalog(archive, options.with_full_provenance())?.json(),
    )?)
}

fn ship<'catalog>(
    catalog: &'catalog Value,
    class: &str,
) -> Result<&'catalog Value, Box<dyn std::error::Error>> {
    proof::rows(catalog, "/ships")?
        .iter()
        .find(|row| row.pointer("/className/value").and_then(Value::as_str) == Some(class))
        .ok_or_else(|| format!("selected canonical Ship {class}").into())
}

fn index(row: &Value) -> Result<u32, Box<dyn std::error::Error>> {
    let raw = proof::at(row, "/recordIndex/value")?
        .as_u64()
        .ok_or("original canonical Ship Record index")?;
    Ok(u32::try_from(raw)?)
}

fn compare_present(
    archive: &Archive,
    catalog: &Value,
    class: &str,
    expected: Playability,
    encrypted: bool,
    dependencies: usize,
) -> TestResult {
    let row = ship(catalog, class)?;
    let producer = proof::present(row, "playability", &json!(expected.name()), Some(encrypted))?;
    let index = index(row)?;
    let uses = proof::uses(catalog, producer)?;
    assert_lookup(&uses);
    let mut foreign = physical_indices(&uses, "dataforgeValue", "$value", "selector");
    foreign.remove(&index);
    assert_eq!(
        foreign.len(),
        dependencies,
        "the classifier cites every reached dependency body for {class}"
    );
    compare_sdk(archive, catalog, index, producer, &BTreeSet::new())
}

fn assert_lookup(uses: &[&Value]) {
    assert!(
        uses.iter().any(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str)
                == Some("dataforgeRecordTable")
        }),
        "the reached GUID lookup cites the physical Record definitions"
    );
}

fn attempted_dependencies(
    catalog: &Value,
    uses: &[&Value],
) -> Result<BTreeSet<u32>, Box<dyn std::error::Error>> {
    let candidates = physical_indices(uses, "dataforgeRecordHeader", "recordIndex", "selector");
    let inventory: Vec<_> = proof::rows(catalog, "/provenance/uses")?.iter().collect();
    let attempts = physical_indices(&inventory, "dataforgeRecordBody", "body", "attempted");
    Ok(candidates.intersection(&attempts).copied().collect())
}
fn physical_indices(uses: &[&Value], namespace: &str, field: &str, role: &str) -> BTreeSet<u32> {
    uses.iter()
        .filter(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && used.get("role").and_then(Value::as_str) == Some(role)
        })
        .filter_map(|used| {
            used.pointer("/location/indices/0")
                .and_then(Value::as_str)?
                .parse()
                .ok()
        })
        .collect()
}

fn compare_sdk(
    archive: &Archive,
    catalog: &Value,
    index: u32,
    producer: &Value,
    failed: &BTreeSet<u32>,
) -> TestResult {
    full_proof::open(archive, CatalogLanguageSelection::Selected(&[]), None)?.with_ship_sources(
        CatalogRecordSelection::RecordIndex(index),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("a single selected physical Ship".into());
            };
            let sdk = serde_json::to_value(row.playability())?;
            assert_eq!(
                proof::with_origins(sdk, &json!({"provenance": snapshot.source_inventory()}))?,
                proof::with_origins(producer.clone(), catalog)?,
                "the selective and canonical classifiers have equal values and coordinates"
            );
            assert_eq!(
                &snapshot
                    .dependency_coverage()
                    .failed_records()
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>(),
                failed,
                "selective coverage lists exactly the failed reached dependency Records"
            );
            Ok(())
        },
    )?
}
