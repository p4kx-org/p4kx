// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;
use serde::{Serialize, Serializer};

use crate::digest::Digest;
use crate::paint_port::VehiclePorts;

/// The base data of a ship with what the catalog derives from its vehicle implementation and the
/// exported base data does not carry: the digest the variant groups compare and the paint ports.
#[derive(Debug)]
pub(crate) struct ShipRead {
    pub(crate) base: ShipBase,
    pub(crate) implementation: Option<Digest>,
    pub(crate) paint_ports: VehiclePorts,
}

fn guid<S: Serializer>(value: &Guid, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

#[expect(clippy::ref_option, reason = "serde passes the field by reference")]
fn optional_guid<S: Serializer>(value: &Option<Guid>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => serializer.collect_str(value),
        None => serializer.serialize_none(),
    }
}

/// What kind of file a value was read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    /// A DataForge Record; the file is its record path.
    Record,
    /// The vehicle implementation XML the ship names.
    VehicleXml,
    /// The patch file of the Modification the ship selects.
    PatchXml,
    /// An `Elem` of the Modification, which sits in the vehicle implementation XML.
    Modification,
    /// An XML Loadout file, named by a Record's `loadoutPath` or included by another one.
    LoadoutXml,
}

/// Where a value comes from: a file of the Archive and the path of the field in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Source {
    /// What the file is.
    pub kind: SourceKind,
    /// The record path of a Record, or the Entry path below `Data` of an XML file.
    pub file: String,
    /// The path of the field or attribute in the file, e.g.
    /// `Components/SAttachableComponentParams/AttachDef/Size`.
    pub field: String,
}

/// A value with the place it was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sourced<T> {
    /// The value.
    pub value: T,
    /// Where it was read.
    pub source: Source,
}

/// A source and how many values of a sum it supplied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceShare {
    /// Where the values were read.
    #[serde(flatten)]
    pub source: Source,
    /// How many values.
    pub values: usize,
}

/// A number attribute of a vehicle part that holds text but no number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UnreadablePartValue {
    /// The part's name.
    pub part: String,
    /// The attribute's name.
    pub attribute: &'static str,
    /// The value as stored.
    pub raw: String,
}

/// A number attribute of a vehicle part that was read as its numeric prefix, e.g. `12,5x` as 12,
/// and enters a sum or a threshold.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PartialPartValue {
    /// The part's name.
    pub part: String,
    /// The attribute's name.
    pub attribute: &'static str,
    /// The value as stored.
    pub raw: String,
    /// The number that was read from it.
    pub read: f64,
}

/// What the mass is the sum of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum MassBasis {
    /// The `mass` of the parts of the vehicle implementation.
    Parts,
    /// The mass of the Record's rigid physics controller, for a vehicle whose parts state none.
    Physics,
}

/// The mass of the hull in kilograms, without the Loadout.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mass {
    /// The mass, rounded to four decimals.
    pub kg: f64,
    /// What it is the sum of.
    pub basis: MassBasis,
    /// How many parts state a mass; 0 for the physics basis.
    pub parts: usize,
    /// Where the summed values were read.
    pub sources: Vec<SourceShare>,
    /// Part values that hold text but no number; they are not in the sum.
    pub unreadable: Vec<UnreadablePartValue>,
    /// Part values read as their numeric prefix that enter the sum.
    pub partial: Vec<PartialPartValue>,
    /// How many numbers had invisible format characters dropped before they were read.
    pub cleaned: usize,
}

/// A part that destroys the ship or detaches at a share of its hit points.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HullThreshold {
    /// The part's name.
    pub part: String,
    /// The part's `damageMax`, rounded to four decimals.
    pub health: f64,
    /// The damage at which it destroys the ship or detaches.
    pub damage: f64,
    /// What the part's `damageMax` was read from.
    pub origin: SourceKind,
}

/// The hull hit points and the thresholds of its parts.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hull {
    /// The sum of `damageMax` of the structural parts, rounded to four decimals.
    pub health: f64,
    /// How many parts the sum holds.
    pub parts: usize,
    /// Where the summed values were read.
    pub sources: Vec<SourceShare>,
    /// Parts that destroy the ship at a share of their hit points.
    pub destruction: Vec<HullThreshold>,
    /// Parts that detach at a share of their hit points and do not destroy the ship.
    pub detach: Vec<HullThreshold>,
    /// Part `damageMax` and `detachRatio` values that hold text but no number; they are not used.
    pub unreadable: Vec<UnreadablePartValue>,
    /// Part `damageMax` and `detachRatio` values read as their numeric prefix that enter the sum
    /// or a threshold.
    pub partial: Vec<PartialPartValue>,
    /// How many `damageMax` numbers had invisible format characters dropped before they were read.
    pub cleaned: usize,
}

/// One cargo grid reached through the default Loadout.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CargoGrid {
    /// The path of port names the grid is attached at.
    pub port: String,
    /// The class name of the grid's Entity.
    pub class_name: String,
    /// The GUID of the grid's Entity.
    #[serde(serialize_with = "guid")]
    pub guid: Guid,
    /// The GUID of the `InventoryContainer` Record, `null` when the Entity names none.
    #[serde(serialize_with = "optional_guid")]
    pub container: Option<Guid>,
    /// `open` for a grid of cells, `closed` for a capacity in cargo units, `null` when unknown.
    pub shape: Option<&'static str>,
    /// The capacity in standard cargo units; `null` when it cannot be read.
    pub scu: Option<f64>,
    /// The cells of 1.25 m an open grid holds when the count fits in `u64`.
    pub cells: Option<u64>,
    /// The interior size of an open grid in metres, `[x, y, z]`.
    pub dimensions: Option<[f64; 3]>,
    /// Where the capacity was read, `null` when it was not.
    pub source: Option<Source>,
    /// Why the capacity cannot be read.
    pub issue: Option<String>,
}

/// The cargo capacity of a ship.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cargo {
    /// The sum of the readable grids' capacity in standard cargo units, rounded to four
    /// decimals: 0 for a ship with no cargo grid, `null` when it has grids and none is readable.
    pub scu: Option<f64>,
    /// The sum of readable open grids' cells when each count and the sum fit in `u64`.
    pub cells: Option<u64>,
    /// How many grids have an `issue`; when it is not 0 the sums are a lower bound.
    pub grids_with_issue: usize,
    /// The grids, in port order.
    pub grids: Vec<CargoGrid>,
}

/// One port of the default Loadout that holds something.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadoutPort {
    /// The path of port names joined by `/`.
    pub port: String,
    /// The class name of the attached Entity, or the one the Loadout names but no Entity has.
    pub class_name: Option<String>,
    /// The GUID of the attached Entity's Record, or of the Record the Loadout names when the
    /// attachment cannot be resolved; `null` for a class name only.
    #[serde(serialize_with = "optional_guid")]
    pub guid: Option<Guid>,
    /// The Record or XML Loadout file that supplied the attachment, `null` for a port nothing
    /// names. For `port_default` it is the Entity Record that defines the port.
    pub supplied_by: Option<Source>,
    /// Where the attachment comes from: `loadout`, `override` or `port_default`.
    pub origin: &'static str,
    /// Whether the Entity is a docked vehicle, whose own ports are not listed.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub docked_vehicle: bool,
    /// Why the attachment cannot be resolved.
    pub unresolved: Option<String>,
}

/// The vehicle implementation a ship's values were read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// The Entry path of the implementation file as it was looked up.
    pub file: String,
    /// The name of the Modification the ship selects.
    pub modification: Option<String>,
    /// The Entry path of the Modification's patch file that was applied.
    pub patch_file: Option<String>,
}

/// Whether a vehicle is a spaceship or a ground vehicle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum VehicleKind {
    /// A vehicle of the folder `entities/spaceships` without wheels or treads.
    Spaceship,
    /// A vehicle of the folder `entities/groundvehicles`.
    GroundVehicle,
}

impl VehicleKind {
    /// Returns the stable code of the kind, e.g. `groundVehicle`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Spaceship => "spaceship",
            Self::GroundVehicle => "groundVehicle",
        }
    }
}

/// The base data of one ship: mass, hull, cargo, size and default Loadout, each with its source.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipBase {
    /// The GUID of the ship's Record.
    #[serde(serialize_with = "guid")]
    pub guid: Guid,
    /// The class name, the Record name after `EntityClassDefinition.`.
    pub class_name: String,
    /// The record path.
    pub record_path: String,
    /// The vehicle implementation file, `null` when the ship names none or it cannot be read.
    pub implementation: Option<Implementation>,
    /// Spaceship or ground vehicle, `null` when the record path and the vehicle implementation
    /// disagree or the path names neither folder.
    pub vehicle_kind: Option<VehicleKind>,
    /// The item size from `AttachDef.Size`.
    pub size: Option<Sourced<i64>>,
    /// The hull mass, `null` when neither the parts nor the physics controller state one.
    pub mass: Option<Mass>,
    /// The hull hit points, `null` without a vehicle implementation.
    pub hull: Option<Hull>,
    /// The cargo capacity.
    pub cargo: Cargo,
    /// Where the default Loadout is read: the XML Loadout file the Record's loadout component
    /// names, else that component's field in the Record.
    pub loadout_source: Source,
    /// The default Loadout, stopping at docked vehicles.
    pub loadout: Vec<LoadoutPort>,
    /// Why parts of the data are missing, e.g. an implementation file that cannot be read.
    pub issues: Vec<String>,
}
