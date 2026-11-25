// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataForgeOrigin, Filter, InputScope,
    RecordTree, SourceEncryption, SourceInput, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use crate::real_archive_item_durability::{self as proof, Trace};
use crate::real_archive_verification;
use crate::real_archive_weapon_combat::original::{self, WEAPON};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const EMITTERS: &str = "/configurations/countermeasureEmitters/rows";
const AMMO: &str = "/definitions/ammoDefinitions/rows";
const CONTAINERS: &str = "/configurations/ammoContainers/rows";
const CONTAINER: &str = "SAmmoContainerComponentParams";

#[test]
#[ignore = "real Archive: requires a bounded Record filter and exact Ammo Record path"]
fn countermeasure_emitter_matches_original_weapon_container_and_signature() -> TestResult {
    let filter = std::env::var("P4KX_COUNTERMEASURE_FILTER").map_err(|_| "set bounded filter")?;
    let ammo_path = std::env::var("P4KX_COUNTERMEASURE_AMMO").map_err(|_| "set exact Ammo path")?;
    let filter = Filter::parse(&filter)?;
    if !filter.matches(&ammo_path) {
        return Err("bounded filter must admit original Ammo definition".into());
    }
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read actual DataForge")?;
    let forge = entry.open().map_err(|_| "open actual DataForge")?;
    let ammo = forge
        .records()
        .find(|record| record.record_path() == Ok(ammo_path.as_str()))
        .ok_or("original Ammo Record")?
        .tree()
        .map_err(|_| "read original Ammo Record")?;
    let catalog = bounded_catalog(&archive, &entry, &forge, &filter)?;
    compare(&catalog, &entry, &forge, &ammo)?;
    watch.check().map_err(|_| "Archive changed")?;
    Ok(())
}

fn bounded_catalog(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &DataForge<'_>,
    filter: &Filter,
) -> TestResult<Value> {
    const MAX_SCOPE: usize = 150;
    if forge
        .records()
        .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
        .take(MAX_SCOPE + 1)
        .count()
        > MAX_SCOPE
    {
        return Err("select a small Record subset covering the owner and Ammo".into());
    }
    let options = CatalogOptions {
        filter: Some(filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(archive, entry, options.with_full_provenance())
        .map_err(|_| "build bounded catalog")?;
    serde_json::from_slice(built.json()).map_err(|_| "read bounded catalog".into())
}

fn compare<'source>(
    catalog: &Value,
    entry: &DataForgeEntry,
    forge: &DataForge<'source>,
    ammo: &RecordTree<'source>,
) -> TestResult {
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("actual DataForge Archive origin".into());
    };
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let uses = proof::indexed(catalog, "/provenance/uses", "id")?;
    let ammo_trace = Trace::of(ammo, &uses, &scope, encrypted);
    let (type_name, signatures) = compare_projectile(catalog, ammo, &ammo_trace)?;
    let mut verified = 0;
    for row in proof::rows(catalog, EMITTERS)? {
        if row
            .pointer("/route/value/ammoRecordIndex/value")
            .and_then(Value::as_u64)
            != Some(u64::from(ammo.record_index()))
        {
            continue;
        }
        let owner_index = proof::number(row, "/ownerRecordIndex/value")?;
        let container_index = proof::number(row, "/route/value/containerRecordIndex/value")?;
        if owner_index != container_index {
            continue;
        }
        let owner = forge
            .record(u32::try_from(owner_index)?)
            .ok_or("original emitter owner")?
            .tree()
            .map_err(|_| "actual emitter owner tree")?;
        let owner_trace = Trace::of(&owner, &uses, &scope, encrypted);
        compare_route(catalog, row, &owner, ammo, &owner_trace, &ammo_trace)?;
        verified += 1;
    }
    if verified == 0 {
        return Err("bounded scope must positively exercise a selected same-owner emitter".into());
    }
    real_archive_verification::print_line(&format!(
        "countermeasure source witness: {type_name}, {verified} emitter routes, {signatures} signal fields, three container counters per route"
    ));
    Ok(())
}

fn compare_projectile<'source>(
    catalog: &Value,
    ammo: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<(&'static str, usize)> {
    let projectile = ammo
        .field("projectileParams")
        .ok_or("original projectile field")?;
    let projectile = original::structure(projectile).ok_or("original projectile Structure")?;
    assert_eq!(
        projectile.type_name(),
        "CounterMeasureProjectileParams",
        "original projectile type"
    );
    let raw = projectile
        .field("typeParams")
        .ok_or("original subtype selector")?;
    let body = original::structure(raw).ok_or("original countermeasure type parameters")?;
    let expected = match body.type_name() {
        "CounterMeasureChaffParams" => "chaff",
        "CounterMeasureFlareParams" => "flare",
        _ => return Err("sample must exercise Chaff or Flare parameters".into()),
    };
    let definition = proof::rows(catalog, AMMO)?
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64)
                == Some(u64::from(ammo.record_index()))
        })
        .ok_or("selected original ammunition definition")?;
    let carrier = definition
        .pointer("/projectileParams/value/kind/value/typeParams")
        .ok_or("canonical countermeasure parameters")?;
    assert_eq!(
        proof::text(carrier, "/value/type")?,
        expected,
        "original countermeasure subtype"
    );
    trace.part(carrier, raw, "$type", "selector")?;
    let signals = carrier
        .pointer("/value/value")
        .ok_or("original signature cells")?;
    let count = compare_signals(signals, body, trace)?;
    Ok((expected, count))
}

fn compare_signals<'source>(
    signals: &Value,
    body: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult<usize> {
    let fields = [
        ("StartInfrared", "startInfrared"),
        ("EndInfrared", "endInfrared"),
        ("StartElectromagnetic", "startElectromagnetic"),
        ("EndElectromagnetic", "endElectromagnetic"),
        ("StartCrossSection", "startCrossSection"),
        ("EndCrossSection", "endCrossSection"),
        ("StartDecibel", "startDecibel"),
        ("EndDecibel", "endDecibel"),
    ];
    for (original, wire) in fields {
        let raw @ Original::Single(number) = body.field(original).ok_or("original signal field")?
        else {
            return Err("expected original signal Single".into());
        };
        let carrier = signals.get(wire).ok_or("canonical signal field")?;
        assert_eq!(
            u32::from_str_radix(proof::text(carrier, "/value/rawBits")?, 16)?,
            number.to_bits(),
            "exact authored signal bits"
        );
        trace.check(carrier, raw, "payload")?;
    }
    Ok(fields.len())
}

fn compare_route<'source>(
    catalog: &Value,
    row: &Value,
    owner: &RecordTree<'source>,
    ammo: &RecordTree<'source>,
    owner_trace: &Trace<'_, 'source>,
    ammo_trace: &Trace<'_, 'source>,
) -> TestResult {
    let route = row.get("route").ok_or("sourced route")?;
    let (weapon_slot, weapon) = original::unique(owner, WEAPON).ok_or("one original weapon")?;
    let (container_slot, container) =
        original::unique(owner, CONTAINER).ok_or("one original ammo container")?;
    assert_eq!(
        proof::number(row, "/componentOrdinal/value")?,
        u64::try_from(weapon_slot)?,
        "physical original weapon slot"
    );
    assert_eq!(
        proof::number(route, "/value/containerComponentOrdinal/value")?,
        u64::try_from(container_slot)?,
        "physical original container slot"
    );
    let body = original::structure(container).ok_or("original ammo container structure")?;
    let weapon_body = original::structure(weapon).ok_or("original weapon structure")?;
    let role = proof::text(route, "/value/ammoRole/value")?;
    let binding = match role {
        "primary" => "ammoParamsRecord",
        "secondary" => "secondaryAmmoParamsRecord",
        _ => return Err("unexpected ammo role".into()),
    };
    let raw @ Original::Reference(target) = body.field(binding).ok_or("original Ammo reference")?
    else {
        return Err("expected original ammo Reference".into());
    };
    assert_eq!(
        target.record_index(),
        ammo.record_index(),
        "canonical Ammo target follows original physical link"
    );
    let action = launcher_role(weapon_body, role)
        .ok_or("original projectile launcher with the selected role")?;
    owner_trace.check(route, action, "payload")?;
    owner_trace.check(route, raw, "selector")?;
    let projectile = ammo
        .field("projectileParams")
        .ok_or("original countermeasure projectile field")?;
    ammo_trace.part(route, projectile, "$type", "selector")?;
    compare_container(catalog, owner, container_slot, body, owner_trace)
}

fn launcher_role<'read, 'source>(
    weapon: &'read StructValue<'source>,
    role: &str,
) -> Option<&'read Original<'source>> {
    original::array(weapon.field("fireActions")?)?
        .iter()
        .find_map(|action| {
            let action = original::structure(action)?;
            let launcher = original::structure(action.field("launchParams")?)?;
            if launcher.type_name() != "SProjectileLauncher" {
                return None;
            }
            let raw = launcher.field("projectileType")?;
            match raw {
                Original::Enum(name) if name.eq_ignore_ascii_case(role) => Some(raw),
                _ => None,
            }
        })
}

fn compare_container<'source>(
    catalog: &Value,
    owner: &RecordTree<'source>,
    slot: usize,
    body: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let slot = u64::try_from(slot)?;
    let row = proof::rows(catalog, CONTAINERS)?
        .iter()
        .find(|row| {
            row.pointer("/ownerRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(owner.record_index()))
                && row
                    .pointer("/componentOrdinal/value")
                    .and_then(Value::as_u64)
                    == Some(slot)
        })
        .ok_or("canonical original ammo container row")?;
    for name in ["initialAmmoCount", "maxAmmoCount", "maxRestockCount"] {
        let raw = body.field(name).ok_or("original distinct ammo count")?;
        let actual = match raw {
            Original::Int32(number) => i64::from(*number),
            Original::UInt32(number) => i64::from(*number),
            _ => return Err("unexpected original count width".into()),
        };
        let carrier = row.get(name).ok_or("canonical ammo count")?;
        assert_eq!(
            carrier["value"].as_i64(),
            Some(actual),
            "count preserves its original quantity"
        );
        trace.check(carrier, raw, "payload")?;
    }
    Ok(())
}
