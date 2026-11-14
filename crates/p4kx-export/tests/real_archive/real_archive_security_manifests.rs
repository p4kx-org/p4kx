// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, StructValue, TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_law_definitions::{verify_class, verify_scalar};
use crate::real_archive_verification::print_line;

const SOURCE_TYPE: &str = "SecurityNetworkManifest";
const KIND: Kind = Kind::SecurityNetworkManifests;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn original_security_manifests_match_canonical_rules_and_links() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let filter = std::env::var("P4KX_SECURITY_MANIFEST_FILTER")
        .ok()
        .map(|text| Filter::new(&text));
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        filter: filter.as_ref(),
        ..CatalogOptions::new(SystemTime::now())
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|_| "build security manifest catalog")?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let count = verify_records(&forge, &catalog, filter.as_ref())?;
    print_line(&format!(
        "SecurityNetworkManifest: {count} physical definitions checked"
    ));
    watch
        .check()
        .map_err(|_| "Archive changed during manifest verification")?;
    Ok(())
}

fn verify_records(
    forge: &DataForge<'_>,
    catalog: &Value,
    filter: Option<&Filter>,
) -> TestResult<usize> {
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| record.type_name().is_ok_and(|name| name == SOURCE_TYPE))
        .filter(|record| {
            filter
                .is_none_or(|pattern| record.record_path().is_ok_and(|path| pattern.matches(path)))
        })
        .map(|record| record.index())
        .collect();
    let rows = consumer::rows(catalog, KIND)?;
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut builder = TreeBuilder::new();
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, KIND, "recordIndex")?
                .as_u64()
                .ok_or("physical manifest index")?,
        )?;
        assert!(seen.insert(index), "one row per physical manifest");
        let record = forge
            .record(index)
            .ok_or("original SecurityNetworkManifest Record")?;
        assert_eq!(
            record.type_name().ok(),
            Some(SOURCE_TYPE),
            "exact original Record type"
        );
        let guid = consumer::identity(row, KIND)?;
        if guid != record.guid().to_string() {
            return Err("manifest GUID does not match the original Record".into());
        }
        order.push((guid, index));
        let tree = builder
            .build(&record)
            .map_err(|_| "read original manifest Record body")?;
        verify_manifest(forge, tree.root(), row)?;
    }
    if seen != expected {
        return Err("manifest rows differ from the original selection".into());
    }
    assert!(order.is_sorted(), "stable GUID and physical index ordering");
    Ok(rows.len())
}

fn verify_manifest(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    for name in ["isNeutralTerritory", "teleportPlayerOut"] {
        verify_scalar(source.field(name), field(row, name)?)?;
    }
    verify_class(
        source.field("defaultProtocol"),
        field(row, "defaultProtocol")?,
        |source, row| verify_protocol(forge, source, row),
    )?;
    for (name, verify) in [
        ("roomProtocols", verify_room as VerifyOwned),
        ("variables", verify_variable),
        ("hostilityRules", verify_hostility),
        ("variableEffects", verify_effect_group),
    ] {
        verify_array(source.field(name), field(row, name)?, |source, row| {
            verify_class(source, row, |source, row| verify(forge, source, row))
        })?;
    }
    Ok(())
}

type VerifyOwned = fn(&DataForge<'_>, &StructValue<'_>, &Value) -> TestResult;

fn verify_protocol(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    for name in ["permissionsWithClearance", "permissionsWithoutClearance"] {
        verify_class(source.field(name), field(row, name)?, |source, row| {
            for name in ["access", "trespass"] {
                verify_scalar(source.field(name), field(row, name)?)?;
            }
            Ok(())
        })?;
    }
    verify_class(
        source.field("acceptedClearance"),
        field(row, "acceptedClearance")?,
        |source, row| {
            verify_array(
                source.field("tokens"),
                field(row, "tokens")?,
                |source, row| verify_link(forge, "SecurityClearanceToken", source, row),
            )
        },
    )?;
    verify_link(
        forge,
        "Faction",
        source.field("ownerFaction"),
        field(row, "ownerFaction")?,
    )
}

fn verify_room(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    verify_class(
        source.field("roomIdentifiers"),
        field(row, "roomIdentifiers")?,
        |source, row| {
            verify_array(source.field("tags"), field(row, "tags")?, |source, row| {
                verify_link(forge, "Tag", source, row)
            })
        },
    )?;
    verify_class(
        source.field("protocol"),
        field(row, "protocol")?,
        |source, row| verify_protocol(forge, source, row),
    )
}

fn verify_hostility(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    for name in [
        "useReputationHostility",
        "usePersonalHostility",
        "useFactionHostility",
        "policesLawfulTrespass",
        "hostileToCriminals",
        "hostileToStolenVehicles",
    ] {
        verify_scalar(source.field(name), field(row, name)?)?;
    }
    for (name, target) in [
        ("tag", "Tag"),
        ("factionOverride", "Faction_LEGACY"),
        ("factionToOverride", "Faction"),
    ] {
        verify_link(forge, target, source.field(name), field(row, name)?)?;
    }
    Ok(())
}

fn verify_variable(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    verify_link(forge, "Tag", source.field("tag"), field(row, "tag")?)?;
    verify_class(
        source.field("value"),
        field(row, "value")?,
        |source, row| {
            let kind = match source.type_name() {
                "SecurityNetworkVariableValue_Bool" => "bool",
                "SecurityNetworkVariableValue_Int" => "int",
                _ => "authored",
            };
            assert_eq!(field(row, "kind")?, kind, "original variable subtype");
            if kind != "authored" {
                verify_scalar(source.field("value"), field(row, "value")?)?;
            }
            Ok(())
        },
    )
}

fn verify_effect_group(forge: &DataForge<'_>, source: &StructValue<'_>, row: &Value) -> TestResult {
    verify_class(
        source.field("variableValue"),
        field(row, "variableValue")?,
        |source, row| verify_variable(forge, source, row),
    )?;
    verify_array(
        source.field("effects"),
        field(row, "effects")?,
        |source, row| {
            verify_class(source, row, |source, row| match source.type_name() {
                "SecurityNetworkVariableEffect_SetHostilityRules" => {
                    assert_eq!(
                        field(row, "kind")?,
                        "setHostilityRules",
                        "original hostility effect subtype"
                    );
                    verify_class(
                        source.field("rules"),
                        field(row, "rules")?,
                        |source, row| verify_hostility(forge, source, row),
                    )
                }
                "SecurityNetworkVariableEffect_SetInteractionState" => {
                    assert_eq!(
                        field(row, "kind")?,
                        "setInteractionState",
                        "original interaction effect subtype"
                    );
                    verify_weak(source.field("state"), field(row, "state")?)
                }
                _ => {
                    assert_eq!(field(row, "kind")?, "authored", "new effect stays authored");
                    Ok(())
                }
            })
        },
    )
}

fn verify_weak(source: Option<&Original<'_>>, row: &Value) -> TestResult {
    let Some(Original::Reference(reference)) = source else {
        return verify_state(source, row);
    };
    let row = consumer::present(row)?;
    let target = field(row, "target")?;
    if consumer::present(field(row, "recordGuid")?)? != &json!(reference.guid().to_string()) {
        return Err("weak link does not match the original GUID".into());
    }
    assert_eq!(
        consumer::present(field(target, "sourceType")?)?,
        reference.type_name(),
        "weak link keeps the source target type"
    );
    if reference.location().is_some() {
        assert_eq!(field(target, "type")?, "instance", "owner-local weak link");
        consumer::present(field(target, "location")?)?;
    } else {
        assert_eq!(
            field(target, "type")?,
            "record",
            "physical Record weak link"
        );
    }
    Ok(())
}
