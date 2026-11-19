// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{
    AmmoDefinition, AmmoSelection, ProjectileAmmoOwner, ProjectileAmmoSnapshot,
    WeaponProjectileAmmoSelection, with_projectile_ammo_from,
};
use p4kx_export::{Kind, is_kind_leftover};
use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeOrigin, InputScope, SourceEncryption, SourceInput,
    SourceUses, Value as Original,
};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};
use serde_json::Value;

use super::original::{self, OriginalAction, Witness};
use super::{TestResult, Trace, cell, proof, with_original_witness};
use crate::real_archive_verification;

const ROUTES: &str = "/projections/weaponProjectileAmmoSelections/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn projectile_ammunition_sdk_matches_original_sources() -> TestResult {
    let paths = std::iter::once(real_archive_path()?).chain(real_archive_b_path());
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    for (index, path) in paths.enumerate() {
        let archive =
            Archive::open_with_key(path, Some(key.clone())).map_err(|_| "open input Archive")?;
        with_original_witness(&archive, verify_sdk)?;
        real_archive_verification::print_line(&format!(
            "projectile SDK source witness {}: one original action and reached Ammo",
            index + 1
        ));
    }
    Ok(())
}

fn verify_sdk(archive: &Archive, entry: &DataForgeEntry, witness: &Witness<'_>) -> TestResult {
    let build = archive.build().ok_or("complete actual Build identity")?;
    if [
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    ]
    .into_iter()
    .any(|value| value.trim().is_empty())
    {
        return Err("the source proof requires a complete Build identity".into());
    }
    let owner = witness.owner.record_index();
    with_projectile_ammo_from(archive, entry, owner, None, |snapshot| {
        if snapshot.build() != Some(build) {
            return Err("the SDK must preserve the exact input Build".into());
        }
        inspect(&snapshot, entry, witness)
    })
    .map_err(|_| "finish Archive-bound projectile SDK read")?
}

fn inspect(
    snapshot: &ProjectileAmmoSnapshot<'_>,
    entry: &DataForgeEntry,
    witness: &Witness<'_>,
) -> TestResult {
    let [owner] = snapshot.rows() else {
        return Err("one physical weapon in the SDK result".into());
    };
    check_scope(snapshot, owner, witness)?;
    let DataForgeOrigin::Archive(origin) = entry.origin() else {
        return Err("the SDK must use the original Archive DataForge Entry".into());
    };
    let encrypted = match entry.origin().encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    let provenance = serde_json::to_value(snapshot.source_inventory())?;
    let uses = proof::indexed(&provenance, "/uses", "id")?;
    let sources = SourceUses::new();
    let scope = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let weapon_trace = Trace::of(&witness.owner, &uses, &scope, encrypted);
    let ammo_trace = Trace::of(&witness.ammo, &uses, &scope, encrypted);
    let action = source_action(witness)?;
    let ordinal = weapon_trace.ordinal(action.action)?;
    let (route, ammo) = selected_rows(owner, witness, ordinal)?;
    let route = wire(route)?;
    let ammo = wire(ammo)?;
    check_original_route(&route, witness, &action, &weapon_trace)?;
    check_original_ammo(&ammo, witness, &ammo_trace)
}

fn check_scope(
    snapshot: &ProjectileAmmoSnapshot<'_>,
    owner: &ProjectileAmmoOwner<'_>,
    witness: &Witness<'_>,
) -> TestResult {
    let coverage = snapshot.coverage();
    if !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !snapshot.requested_languages().is_empty()
        || !snapshot.available_languages().is_empty()
    {
        return Err("the selected SDK owner and empty localization scope must be readable".into());
    }
    let index = witness.ammo.record_index();
    if owner.dependency_coverage.unreadable_bodies.contains(&index)
        || witness
            .magazine
            .is_some_and(|index| owner.dependency_coverage.unreadable_bodies.contains(&index))
    {
        return Err("the original witness dependencies must be readable".into());
    }
    Ok(())
}

fn source_action<'tree, 'source>(
    witness: &'tree Witness<'source>,
) -> TestResult<OriginalAction<'tree, 'source>> {
    let (_, weapon) = original::unique(&witness.owner, original::WEAPON)
        .ok_or("the original physical weapon component")?;
    let slot = original::structure(weapon)
        .and_then(|weapon| weapon.field("fireActions"))
        .and_then(original::array)
        .and_then(|slots| slots.get(witness.mode))
        .ok_or("the original fire-action occurrence")?;
    original::action_inputs(slot).ok_or_else(|| "the original direct projectile action".into())
}

fn selected_rows<'row, 'source>(
    owner: &'row ProjectileAmmoOwner<'source>,
    witness: &Witness<'_>,
    ordinal: usize,
) -> TestResult<(
    &'row WeaponProjectileAmmoSelection,
    &'row AmmoDefinition<'source>,
)> {
    let owner_index = witness.owner.record_index();
    let component = u32::try_from(witness.component)?;
    let ordinal = u64::try_from(ordinal)?;
    let mut routes = owner.selections.iter().filter(|row| {
        row.owner().record_index().value() == Some(&owner_index)
            && row.owner().component_ordinal().value() == Some(&component)
            && row
                .action_source_value_ordinal()
                .value()
                .is_some_and(|value| value.get() == ordinal)
    });
    let (Some(route), None) = (routes.next(), routes.next()) else {
        return Err("one SDK route for the exact original action occurrence".into());
    };
    let ammo_index = witness.ammo.record_index();
    let mut definitions = owner
        .ammo_definitions
        .iter()
        .filter(|row| row.source().record_index().value() == Some(&ammo_index));
    let (Some(ammo), None) = (definitions.next(), definitions.next()) else {
        return Err("one SDK definition for the physical Ammo target".into());
    };
    let expected_role = if is_kind_leftover(
        Kind::AmmoDefinitions,
        witness.ammo.record_path(),
        witness.ammo.name(),
    ) {
        AmmoSelection::Dependency
    } else {
        AmmoSelection::Selected
    };
    if ammo.selection() != expected_role
        || ammo.source().guid().value().copied() != Some(witness.ammo.guid())
    {
        return Err("SDK Ammo admission and physical source identity must agree".into());
    }
    Ok((route, ammo))
}

fn check_original_ammo<'source>(
    definition: &Value,
    witness: &Witness<'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let inputs = original::ammo_inputs(&witness.ammo).ok_or("preserved original Ammo inputs")?;
    for (field, raw) in [("speed", inputs.speed), ("lifetime", inputs.lifetime)] {
        let Original::Single(value) = raw else {
            return Err("the original Ammo input must be a Single".into());
        };
        let projected = cell(definition, field)?;
        if proof::text(projected, "/value/rawBits")? != format!("{:08x}", value.to_bits()) {
            return Err("the SDK Ammo input must preserve original scalar bits".into());
        }
        trace.check(projected, raw, "payload")?;
    }
    trace.header(cell(definition, "recordIndex")?, "recordIndex", "selector")
}

pub(super) fn route_row<'catalog>(
    catalog: &'catalog Value,
    witness: &Witness<'_>,
    ordinal: &str,
) -> TestResult<&'catalog Value> {
    let owner = u64::from(witness.owner.record_index());
    let component = u64::try_from(witness.component)?;
    let mut routes = proof::rows(catalog, ROUTES)?.iter().filter(|row| {
        row.pointer("/ownerRecordIndex/value")
            .and_then(Value::as_u64)
            == Some(owner)
            && row
                .pointer("/componentOrdinal/value")
                .and_then(Value::as_u64)
                == Some(component)
            && row
                .pointer("/actionSourceValueOrdinal/value")
                .and_then(Value::as_str)
                == Some(ordinal)
    });
    let (Some(route), None) = (routes.next(), routes.next()) else {
        return Err("one producer route for the exact original action occurrence".into());
    };
    Ok(route)
}

pub(super) fn check_original_route<'source>(
    route: &Value,
    witness: &Witness<'source>,
    action: &OriginalAction<'_, 'source>,
    trace: &Trace<'_, 'source>,
) -> TestResult {
    let state = proof::text(route, "/selection/value/type")?;
    if let Some(magazine) = witness.magazine {
        if state != "selectedLinkedMagazine"
            || proof::number(route, "/selection/value/magazineRecordIndex/value")?
                != u64::from(magazine)
        {
            return Err("the linked route must retain the original magazine owner".into());
        }
    } else if state != "selected" {
        return Err("the inline route must retain the owning weapon".into());
    }
    let role = match action.role {
        Original::Enum("Primary") => "primary",
        Original::Enum("Secondary") => "secondary",
        _ => return Err("the original role must be a known selector".into()),
    };
    if proof::text(route, "/selection/value/role/value")? != role
        || proof::number(route, "/selection/value/ammoRecordIndex/value")?
            != u64::from(witness.ammo.record_index())
        || proof::number(route, "/selection/value/containerComponentOrdinal/value")?
            != u64::try_from(witness.container)?
    {
        return Err("the route must retain the exact authored role and physical targets".into());
    }
    trace.check(cell(route, "selection")?, action.role, "payload")
}

fn wire(row: &impl serde::Serialize) -> TestResult<Value> {
    let bytes = serde_json::to_vec(row)?;
    Ok(serde_json::from_slice(&bytes)?)
}
