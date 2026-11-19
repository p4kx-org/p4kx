// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForgeEntry, DataForgeOrigin, InputScope, RecordTree,
    SourceEncryption, SourceInput, SourceScope, SourceUses, StructValue, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::real_archive_item_durability::{Trace, indexed, rows, text};

#[path = "flight_witness_placements.rs"]
mod placements;
#[path = "flight_witness_selection.rs"]
mod selection;

#[path = "flight_sdk_source_witness.rs"]
mod sdk;

use selection::{Selection, SelectorFile};

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) const CONTROLLER_TYPE: &str = "SCItemFlightControllerParams";
pub(super) const IFCS_TYPE: &str = "IFCSParams";
pub(super) const THRUSTER_TYPE: &str = "SCItemThrusterParams";
const CONTROLLERS: &str = "/configurations/flightControllers/rows";
const IFCS: &str = "/configurations/ifcs/rows";
const THRUSTERS: &str = "/configurations/thrusters/rows";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and P4KX_FLIGHT_CONFIGURATION_WITNESS before --ignored"]
fn installed_flight_configurations_match_original_components_and_loadouts() -> Result {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let fixture = SelectorFile::read()?;
    let sources = SourceUses::with_compact_coordinates();
    let origin = EntryOrigin::read(&sources, entry.origin())?;

    let controller = Selection::read(&forge, &fixture.controller)?;
    let catalog = controller.build(&archive, &entry)?;
    check_controller_configuration(&catalog, &controller.tree, &origin)?;
    drop(catalog);
    for (path, kind, installed) in [
        (fixture.spaceship, "spaceship", Some(&controller.tree)),
        (fixture.ground, "groundVehicle", None),
    ] {
        let selection = Selection::read(&forge, &path)?;
        let catalog = selection.build(&archive, &entry)?;
        placements::check_vehicle(&archive, &forge, &catalog, &selection, kind, installed)?;
        watch
            .check()
            .map_err(|_| "Archive changed after vehicle comparison")?;
    }
    for (path, role) in [
        (fixture.main, fixture.main_role),
        (fixture.maneuver, fixture.maneuver_role),
    ] {
        let selection = Selection::read(&forge, &path)?;
        let catalog = selection.build(&archive, &entry)?;
        check_thruster(&catalog, &selection.tree, &origin, &role)?;
        watch
            .check()
            .map_err(|_| "Archive changed after thruster comparison")?;
    }
    Ok(())
}

struct EntryOrigin {
    scope: SourceScope,
    encrypted: Option<bool>,
}

impl EntryOrigin {
    fn read(sources: &SourceUses, origin: &DataForgeOrigin) -> Result<Self> {
        let DataForgeOrigin::Archive(entry) = origin else {
            return Err("expected physical Archive DataForge".into());
        };
        let scope = sources
            .observe(SourceInput::Archive(entry), InputScope::Payload)
            .map_err(|_| "admit physical DataForge Entry")?;
        let encrypted = match origin.encryption() {
            SourceEncryption::Encrypted => Some(true),
            SourceEncryption::Unencrypted => Some(false),
            _ => None,
        };
        Ok(Self { scope, encrypted })
    }
}

fn check_controller_configuration(
    catalog: &Value,
    tree: &RecordTree<'_>,
    origin: &EntryOrigin,
) -> Result {
    let uses = indexed(catalog, "/provenance/uses", "id")?;
    let trace = Trace::of(tree, &uses, &origin.scope, origin.encrypted);
    check_definitions(
        catalog,
        tree,
        &trace,
        CONTROLLER_TYPE,
        CONTROLLERS,
        |row, original| check_controller(row, original, &trace),
    )?;
    check_definitions(catalog, tree, &trace, IFCS_TYPE, IFCS, |row, original| {
        check_ifcs(row, original, &trace)
    })?;
    Ok(())
}

fn check_controller<'source>(
    row: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> Result {
    single(
        row,
        original,
        trace,
        "fuelWarningDisplayTime",
        "/fuelWarningDisplayTime",
    )?;
    let collision = container(
        row,
        original,
        trace,
        "collisionDetection",
        "/collisionDetection",
    )?;
    let output = row
        .pointer("/collisionDetection/value")
        .ok_or("collision settings")?;
    single(
        output,
        collision,
        trace,
        "collisionWarnSpeed",
        "/collisionWarnSpeed",
    )
}

fn check_ifcs<'source>(
    row: &Value,
    original: &StructValue<'source>,
    trace: &Trace<'_, 'source>,
) -> Result {
    single(row, original, trace, "maxSpeed", "/limits/maxSpeed")?;
    for name in ["afterburner", "afterburnerNew"] {
        let pointer = format!("/dynamics/{name}");
        let burn = container(row, original, trace, name, &pointer)?;
        let output = row
            .pointer(&format!("{pointer}/value"))
            .ok_or("afterburner settings")?;
        single(
            output,
            burn,
            trace,
            "afterburnerRampUpTime",
            "/afterburnerRampUpTime",
        )?;
    }
    Ok(())
}

fn check_thruster(
    catalog: &Value,
    tree: &RecordTree<'_>,
    origin: &EntryOrigin,
    role: &str,
) -> Result {
    let uses = indexed(catalog, "/provenance/uses", "id")?;
    let trace = Trace::of(tree, &uses, &origin.scope, origin.encrypted);
    check_definitions(
        catalog,
        tree,
        &trace,
        THRUSTER_TYPE,
        THRUSTERS,
        |row, original| {
            single(row, original, &trace, "thrustCapacity", "/thrustCapacity")?;
            single(
                row,
                original,
                &trace,
                "thrustCapacityNew",
                "/thrustCapacityNew",
            )?;
            let raw = original
                .field("thrusterType")
                .ok_or("original thruster role")?;
            let Original::Enum(raw_role) = raw else {
                return Err("original thruster role is not an Enum".into());
            };
            let cell = row.get("thrusterType").ok_or("exported thruster role")?;
            assert!(
                *raw_role == role
                    && text(cell, "/value")? == *raw_role
                    && text(cell, "/state")? == "present",
                "selected thruster preserves its exact authored role"
            );
            trace.check(cell, raw, "payload")
        },
    )?;
    Ok(())
}

fn check_definitions<'source>(
    catalog: &Value,
    tree: &RecordTree<'source>,
    trace: &Trace<'_, 'source>,
    kind: &str,
    table: &str,
    mut check: impl FnMut(&Value, &StructValue<'source>) -> Result,
) -> Result {
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("source Entity Components".into());
    };
    let mut verified = false;
    let rows = rows(catalog, table)?;
    let guid = tree.guid().to_string();
    for (slot, raw) in components.iter().enumerate() {
        let Original::Struct(structure) = raw else {
            continue;
        };
        if structure.type_name() != kind {
            continue;
        }
        let slot = u64::try_from(slot)?;
        let mut matching = rows.iter().filter(|row| {
            row.pointer("/ownerRecordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(tree.record_index()))
                && row
                    .pointer("/componentOrdinal/value")
                    .and_then(Value::as_u64)
                    == Some(slot)
        });
        let row = matching
            .next()
            .ok_or("missing physical configuration row")?;
        assert!(matching.next().is_none(), "one row per exact physical slot");
        assert!(
            text(row, "/ownerGuid/value")? == guid,
            "physical owner GUID"
        );
        assert!(
            text(row, "/sourceType/value")? == kind,
            "physical source type"
        );
        trace.header(
            row.get("ownerGuid").ok_or("sourced owner GUID")?,
            "guid",
            "payload",
        )?;
        trace.check(
            row.get("componentOrdinal")
                .ok_or("sourced component ordinal")?,
            raw,
            "conversion",
        )?;
        check(row, structure)?;
        verified = true;
    }
    if !verified {
        return Err("no selected component of the expected original type".into());
    }
    Ok(())
}

fn single(
    row: &Value,
    original: &StructValue<'_>,
    trace: &Trace<'_, '_>,
    name: &str,
    pointer: &str,
) -> Result {
    let raw = original.field(name).ok_or("original Single declaration")?;
    let Original::Single(value) = raw else {
        return Err("original field has unexpected storage".into());
    };
    let cell = row.pointer(pointer).ok_or("exported field cell")?;
    assert!(text(cell, "/state")? == "present", "{name} is present");
    assert!(
        u32::from_str_radix(text(cell, "/value/rawBits")?, 16)? == value.to_bits(),
        "{name} retains exact original Single bits"
    );
    trace.check(cell, raw, "payload")
}

fn container<'raw, 'source>(
    row: &Value,
    original: &'raw StructValue<'source>,
    trace: &Trace<'_, 'source>,
    name: &str,
    path: &str,
) -> Result<&'raw StructValue<'source>> {
    let raw = original.field(name).ok_or("original class declaration")?;
    let Original::Struct(value) = raw else {
        return Err("original field has unexpected Class storage".into());
    };
    let cell = row.pointer(path).ok_or("exported class cell")?;
    assert!(text(cell, "/state")? == "present", "{name} is present");
    assert!(
        text(cell, "/value/sourceType/value")? == value.type_name(),
        "{name} retains concrete source type"
    );
    trace.part(cell, raw, "$type", "selector")?;
    Ok(value)
}
