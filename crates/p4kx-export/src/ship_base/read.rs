// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "observe.rs"]
mod observation;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutEntry, LoadoutResolver, OriginTally, PartThreshold, PortNode,
    PortSource, UnreadableValue, ValueOrigin, VehicleImplementation, VehicleTotals,
};
use p4kx_extraction::{Record, RecordTree, TreeBuilder};

use super::cargo::{CargoReader, rounded};
use super::model::{
    Hull, HullThreshold, Implementation, LoadoutPort, Mass, MassBasis, PartialPartValue, ShipBase,
    ShipRead, Source, SourceKind, SourceShare, Sourced, UnreadablePartValue, VehicleKind,
};
use crate::CatalogFailure;
use crate::Kind;
use crate::digest::Digest;
use crate::paint_port::VehiclePorts;
use crate::record_fields::{class_name, component, integer, number, structure, text};
use crate::selection::starts_with_folder;

const SIZE_FIELD: &str = "Components/SAttachableComponentParams/AttachDef/Size";
const PHYSICS_MASS_FIELD: &str = "Components/SEntityPhysicsControllerParams/PhysType/Mass";
const LOADOUT_FIELD: &str = "Components/SEntityComponentDefaultLoadoutParams/loadout";
const XML_LOADOUT_FIELD: &str = "Loadout/Items";
const PORT_FIELD: &str = "Components/SItemPortContainerComponentParams/Ports";
const PARTS_MASS: &str = "Parts//@mass";
const PARTS_DAMAGE: &str = "Parts//@damageMax";

/// Reads the base data of ships: mass, hull, cargo, size and default Loadout.
///
/// The reader keeps the Records its [`LoadoutResolver`] and its cargo reader have read, so
/// reading many ships with one reader reads each shared Entity once.
pub struct ShipReader<'f> {
    resolver: LoadoutResolver<'f>,
    trees: TreeBuilder<'f>,
    cargo: CargoReader<'f>,
}

impl std::fmt::Debug for ShipReader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShipReader").finish_non_exhaustive()
    }
}

impl<'f> ShipReader<'f> {
    /// Reads the base data of the ship `record`.
    ///
    /// # Errors
    ///
    /// Returns a [`CatalogFailure`] when the Record is no Entity or cannot be read; a vehicle
    /// implementation or a container that cannot be read is an entry of `issues` instead.
    pub fn read(&mut self, record: &Record<'f>) -> Result<ShipBase, CatalogFailure> {
        let tree = self
            .trees
            .build(record)
            .map_err(|error| CatalogFailure::of(record, &error))?;
        self.read_tree(record, &tree)
    }

    /// Reads the base data of the ship `record` like [`read`](Self::read) from `tree`, the Record
    /// tree already built from it, so that a caller holding it does not build it again.
    ///
    /// # Errors
    ///
    /// As [`read`](Self::read), except that the Record tree is not built here.
    pub fn read_tree(
        &mut self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
    ) -> Result<ShipBase, CatalogFailure> {
        self.read_with_digest(record, tree).map(|read| read.base)
    }

    /// As [`read_tree`](Self::read_tree), with the digest of the vehicle implementation.
    pub(crate) fn read_with_digest(
        &mut self,
        record: &Record<'f>,
        tree: &RecordTree<'f>,
    ) -> Result<ShipRead, CatalogFailure> {
        let node = self
            .resolver
            .resolve_tree(record, tree)
            .map_err(|error| CatalogFailure::of(record, &error))?;
        let vehicle = node.vehicle();
        let totals = vehicle.map(VehicleImplementation::totals);
        let mut issues: Vec<String> = node.issues().iter().map(ToString::to_string).collect();
        issues.extend(
            vehicle
                .iter()
                .flat_map(|v| v.issues())
                .map(ToString::to_string),
        );
        let entries = node.loadout();
        let digest = vehicle
            .filter(|v| v.issues().is_empty())
            .map(structure_digest);
        let base = ShipBase {
            guid: record.guid(),
            class_name: class_name(tree).to_owned(),
            record_path: tree.record_path().to_owned(),
            implementation: vehicle.map(implementation),
            vehicle_kind: vehicle_kind(tree.record_path(), vehicle),
            size: size(tree),
            mass: mass(tree, vehicle, totals.as_ref()),
            hull: vehicle.zip(totals.as_ref()).and_then(|(v, t)| hull(v, t)),
            cargo: self.cargo.read(&entries, &mut self.trees),
            loadout_source: node.loadout_file().map_or_else(
                || record_source(tree.record_path(), LOADOUT_FIELD),
                |path| Source {
                    kind: SourceKind::LoadoutXml,
                    file: path.to_owned(),
                    field: XML_LOADOUT_FIELD.to_owned(),
                },
            ),
            loadout: loadout(&entries),
            issues,
        };
        let definition = text(
            component(tree, "VehicleComponentParams"),
            "vehicleDefinition",
        )
        .map(str::trim)
        .filter(|definition| !definition.is_empty());
        let paint_ports = VehiclePorts::of(vehicle, definition, node.issues());
        Ok(ShipRead {
            base,
            implementation: digest,
            paint_ports,
        })
    }
}

const fn structure_digest(vehicle: &VehicleImplementation) -> Digest {
    Digest::from_bytes(vehicle.structure_digest())
}

const SPACESHIP_FOLDER: usize = 0;
const GROUND_FOLDER: usize = 1;

/// Returns whether `record_path` is under the folder of the `ships` kind at `index` of its rule.
fn in_folder(record_path: &str, index: usize) -> bool {
    Kind::Ships
        .records()
        .prefixes
        .get(index)
        .is_some_and(|prefix| starts_with_folder(record_path, prefix))
}

/// Decides the kind from the folder of the record path and the wheels and treads of the vehicle
/// implementation: a ground vehicle is one in the ground folder, a spaceship one in the spaceship
/// folder without a wheel or tread; a spaceship folder with wheels or treads is no evidence either
/// way. The folders are matched like the selection of the `ships` kind matches its prefixes.
fn vehicle_kind(record_path: &str, vehicle: Option<&VehicleImplementation>) -> Option<VehicleKind> {
    let rolling = vehicle.is_some_and(|vehicle| {
        vehicle.wheels().next().is_some() || vehicle.treads().next().is_some()
    });
    if in_folder(record_path, GROUND_FOLDER) {
        Some(VehicleKind::GroundVehicle)
    } else if in_folder(record_path, SPACESHIP_FOLDER) && !rolling {
        Some(VehicleKind::Spaceship)
    } else {
        None
    }
}

fn implementation(vehicle: &VehicleImplementation) -> Implementation {
    Implementation {
        file: vehicle.path().to_owned(),
        modification: vehicle.modification().map(str::to_owned),
        patch_file: vehicle.patch_path().map(str::to_owned),
    }
}

fn size(tree: &RecordTree<'_>) -> Option<Sourced<i64>> {
    let definition = structure(component(tree, "SAttachableComponentParams"), "AttachDef");
    Some(Sourced {
        value: integer(definition, "Size")?,
        source: Source {
            kind: SourceKind::Record,
            file: tree.record_path().to_owned(),
            field: SIZE_FIELD.to_owned(),
        },
    })
}

fn mass(
    tree: &RecordTree<'_>,
    vehicle: Option<&VehicleImplementation>,
    totals: Option<&VehicleTotals>,
) -> Option<Mass> {
    if let (Some(vehicle), Some(totals)) = (vehicle, totals)
        && totals.mass_parts() > 0
        && totals.mass() > 0.0
        && let kg = rounded(totals.mass())
        && kg.is_finite()
    {
        return Some(Mass {
            kg,
            basis: MassBasis::Parts,
            parts: totals.mass_parts(),
            sources: shares(vehicle, totals.mass_origins(), PARTS_MASS),
            unreadable: unreadable(totals, &["mass"]),
            partial: partial(totals, &["mass"]),
            cleaned: totals.cleaned_mass(),
        });
    }
    let physics = structure(
        component(tree, "SEntityPhysicsControllerParams"),
        "PhysType",
    );
    let kg = number(physics, "Mass").filter(|kg| *kg > 0.0)?;
    let kg = rounded(f64::from(kg));
    if !kg.is_finite() {
        return None;
    }
    Some(Mass {
        kg,
        basis: MassBasis::Physics,
        parts: 0,
        sources: vec![SourceShare {
            source: Source {
                kind: SourceKind::Record,
                file: tree.record_path().to_owned(),
                field: PHYSICS_MASS_FIELD.to_owned(),
            },
            values: 1,
        }],
        unreadable: totals.map_or_else(Vec::new, |totals| unreadable(totals, &["mass"])),
        partial: Vec::new(),
        cleaned: 0,
    })
}

fn hull(vehicle: &VehicleImplementation, totals: &VehicleTotals) -> Option<Hull> {
    if totals.health_parts() == 0 {
        return None;
    }
    let health = rounded(totals.health());
    if !health.is_finite() {
        return None;
    }
    Some(Hull {
        health,
        parts: totals.health_parts(),
        sources: shares(vehicle, totals.health_origins(), PARTS_DAMAGE),
        destruction: thresholds(totals.destruction()),
        detach: thresholds(totals.detach()),
        unreadable: unreadable(totals, &["damageMax", "detachRatio"]),
        partial: partial(totals, &["damageMax", "detachRatio"]),
        cleaned: totals.cleaned_damage(),
    })
}

fn thresholds(found: &[PartThreshold]) -> Vec<HullThreshold> {
    found
        .iter()
        .map(|threshold| HullThreshold {
            part: threshold.part().to_owned(),
            health: rounded(threshold.health()),
            damage: rounded(threshold.damage()),
            origin: kind(threshold.origin()),
        })
        .collect()
}

fn unreadable(totals: &VehicleTotals, attributes: &[&str]) -> Vec<UnreadablePartValue> {
    totals
        .unreadable()
        .iter()
        .filter(|value| attributes.contains(&value.attribute()))
        .map(|value: &UnreadableValue| UnreadablePartValue {
            part: value.part().to_owned(),
            attribute: value.attribute(),
            raw: value.raw().to_owned(),
        })
        .collect()
}

fn partial(totals: &VehicleTotals, attributes: &[&str]) -> Vec<PartialPartValue> {
    totals
        .partial()
        .iter()
        .filter(|value| attributes.contains(&value.attribute()))
        .map(|value| PartialPartValue {
            part: value.part().to_owned(),
            attribute: value.attribute(),
            raw: value.raw().to_owned(),
            read: value.read(),
        })
        .collect()
}

fn record_source(file: &str, field: &str) -> Source {
    Source {
        kind: SourceKind::Record,
        file: file.to_owned(),
        field: field.to_owned(),
    }
}

fn supplier(port: &PortNode) -> Option<Source> {
    let origin = port.origin()?;
    Some(if origin.is_file() {
        Source {
            kind: SourceKind::LoadoutXml,
            file: origin.path().to_owned(),
            field: XML_LOADOUT_FIELD.to_owned(),
        }
    } else if port.source() == PortSource::PortDefault {
        record_source(origin.path(), PORT_FIELD)
    } else {
        record_source(origin.path(), LOADOUT_FIELD)
    })
}

const fn kind(origin: ValueOrigin) -> SourceKind {
    match origin {
        ValueOrigin::Base => SourceKind::VehicleXml,
        ValueOrigin::Patch => SourceKind::PatchXml,
        ValueOrigin::Elem => SourceKind::Modification,
    }
}

fn shares(vehicle: &VehicleImplementation, tally: OriginTally, field: &str) -> Vec<SourceShare> {
    [ValueOrigin::Base, ValueOrigin::Patch, ValueOrigin::Elem]
        .into_iter()
        .filter(|origin| tally.count(*origin) > 0)
        .map(|origin| {
            let (file, field) = match origin {
                ValueOrigin::Base => (vehicle.path().to_owned(), field.to_owned()),
                ValueOrigin::Patch => (
                    vehicle
                        .patch_path()
                        .unwrap_or_else(|| vehicle.path())
                        .to_owned(),
                    field.to_owned(),
                ),
                ValueOrigin::Elem => (
                    vehicle.path().to_owned(),
                    format!(
                        "Modifications/Modification[@name={}]/Elems",
                        vehicle.modification().unwrap_or_default()
                    ),
                ),
            };
            SourceShare {
                source: Source {
                    kind: kind(origin),
                    file,
                    field,
                },
                values: tally.count(origin),
            }
        })
        .collect()
}

fn loadout(entries: &[LoadoutEntry<'_>]) -> Vec<LoadoutPort> {
    entries
        .iter()
        .map(|entry| LoadoutPort {
            port: entry.path().to_owned(),
            class_name: entry.class_name().map(str::to_owned),
            guid: entry.guid(),
            supplied_by: supplier(entry.port()),
            origin: entry.port().source().code(),
            docked_vehicle: entry.entity().is_some_and(EntityNode::is_docked_vehicle),
            unresolved: match entry.port().attachment() {
                Attachment::Unresolved(unresolved) => Some(unresolved.error().to_string()),
                _ => None,
            },
        })
        .collect()
}
