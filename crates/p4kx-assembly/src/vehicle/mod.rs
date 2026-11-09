// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The vehicle implementation of a ship: the XML file its `VehicleComponentParams` name, with the
//! Modification the Entity selects applied.

mod elem;
mod file_read;
mod gear;
mod item_port;
mod locate;
mod origin;
mod patch;
mod paths;
mod read;
mod structure;
mod totals;

use std::collections::HashMap;

use crate::merge::port_key;
use crate::{LoadoutError, LoadoutFiles};

pub use elem::ElemOverride;
pub(crate) use gear::gears;
pub(crate) use item_port::flags_contain;
pub use item_port::{VehicleItemPort, VehicleItemType};
pub(crate) use locate::{load, load_observed};
pub use origin::ValueOrigin;
pub use patch::ModificationReport;
pub use totals::{OriginTally, PartThreshold, PartialValue, UnreadableValue, VehicleTotals};

/// Whether a port shows its item: the vehicle implementation's part of that name decides, else
/// the flags of the port definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum PortVisibility {
    /// Neither the vehicle implementation nor the port definition states it.
    #[default]
    Unspecified,
    /// The part's item port is shown.
    Visible,
    /// The part's item port, or else the port definition, carries the flag `invisible`.
    Invisible,
}

impl PortVisibility {
    /// Returns the stable code of the visibility, e.g. `invisible`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unspecified => "unspecified",
            Self::Visible => "visible",
            Self::Invisible => "invisible",
        }
    }
}

/// What a part of the vehicle implementation is, by its `class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PartKind {
    /// A part that holds an item port, `class="ItemPort"`.
    ItemPort,
    /// A tread of a tracked vehicle, `class="Tread"`.
    Tread,
    /// A wheel, `class="SubPartWheel"`.
    Wheel,
    /// Any other part, e.g. `Animated`, `AnimatedJoint` or `Static`.
    Other,
}

/// One part of the vehicle implementation: the hull, a hardpoint, a tread or a wheel.
#[derive(Debug, Clone, PartialEq)]
pub struct VehiclePart {
    pub(crate) name: String,
    pub(crate) class: String,
    pub(crate) kind: PartKind,
    pub(crate) mass: Option<f64>,
    pub(crate) damage_max: Option<f64>,
    pub(crate) detach_ratio: Option<f64>,
    pub(crate) destroy_ratio: Option<f64>,
    pub(crate) attributes: Vec<(String, String)>,
    pub(crate) origins: Vec<(String, ValueOrigin)>,
    pub(crate) item_port: Option<VehicleItemPort>,
    pub(crate) tread_wheels: Vec<String>,
    pub(crate) children: Vec<Self>,
}

impl VehiclePart {
    /// Returns the part name; an item port carries it as its port name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the `class` as stored, e.g. `ItemPort` or `Animated`.
    #[must_use]
    pub fn class(&self) -> &str {
        &self.class
    }

    /// Returns what the part is.
    #[must_use]
    pub const fn kind(&self) -> PartKind {
        self.kind
    }

    /// Returns the part's mass in kilograms, or `None` when it gives none or an unreadable one.
    #[must_use]
    pub const fn mass(&self) -> Option<f64> {
        self.mass
    }

    /// Returns the damage the part takes before it is destroyed, or `None` when it gives none.
    #[must_use]
    pub const fn damage_max(&self) -> Option<f64> {
        self.damage_max
    }

    /// Returns the part of its damage at which it detaches, or `None` when it gives none.
    #[must_use]
    pub const fn detach_ratio(&self) -> Option<f64> {
        self.detach_ratio
    }

    /// Returns the share of `damageMax` at which the part destroys the whole vehicle: the
    /// `damageRatioMin` of its first damage behaviour in the group `Destroy`, 1 when that states
    /// none, or `None` when the part has no such behaviour.
    #[must_use]
    pub const fn destroy_ratio(&self) -> Option<f64> {
        self.destroy_ratio
    }

    /// Returns where the attribute `name` got its value, ignoring ASCII case:
    /// [`ValueOrigin::Base`] unless the Modification's patch file or an `Elem` set it.
    #[must_use]
    pub fn origin_of(&self, name: &str) -> ValueOrigin {
        self.origins
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map_or(ValueOrigin::Base, |(_, origin)| *origin)
    }

    /// Returns the raw value of the part's attribute `name`, ignoring ASCII case, or `None`
    /// when the part has none; for readers that apply their own rule to e.g. `mass`.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Returns the part's item port, or `None` when it holds none.
    #[must_use]
    pub const fn item_port(&self) -> Option<&VehicleItemPort> {
        self.item_port.as_ref()
    }

    /// Returns the part names of the wheels a tread runs on; empty for any other part.
    #[must_use]
    pub fn tread_wheels(&self) -> &[String] {
        &self.tread_wheels
    }

    /// Returns the parts below this one, in file order.
    #[must_use]
    pub fn children(&self) -> &[Self] {
        &self.children
    }
}

/// One landing gear of the `VehicleLandingGearSystem` Record an Entity's vehicle component names.
#[derive(Debug, Clone, PartialEq)]
pub struct LandingGear {
    pub(crate) bone: String,
    pub(crate) geometry: String,
    pub(crate) mass: Option<f64>,
}

impl LandingGear {
    /// Returns the name of the bone the gear moves.
    #[must_use]
    pub fn bone(&self) -> &str {
        &self.bone
    }

    /// Returns the path of the gear's geometry; empty when the Record gives none.
    #[must_use]
    pub fn geometry(&self) -> &str {
        &self.geometry
    }

    /// Returns the gear's mass in kilograms, or None when the Record gives none.
    #[must_use]
    pub const fn mass(&self) -> Option<f64> {
        self.mass
    }
}

/// The vehicle implementation of an Entity: its parts after the Modification is applied.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleImplementation {
    pub(crate) path: String,
    pub(crate) patch_path: Option<String>,
    pub(crate) modification: Option<String>,
    pub(crate) report: Option<ModificationReport>,
    pub(crate) elems: Vec<ElemOverride>,
    pub(crate) name: String,
    pub(crate) parts: Vec<VehiclePart>,
    pub(crate) port_paths: HashMap<String, Box<[usize]>>,
    pub(crate) issues: Vec<LoadoutError>,
    pub(crate) structure: [u8; 16],
}

impl VehicleImplementation {
    /// Reads the vehicle implementation of the Entity `class_name` from `files`, as its
    /// `VehicleComponentParams` name it: `definition` is the `vehicleDefinition` and
    /// `modification` the Modification to apply, if the Entity selects one.
    ///
    /// The file is `definition`; when `files` hold none of that path, the file of the same name
    /// in the implementation folder, then the file named after `class_name`. The patch file of
    /// the Modification is its `patchFile`, else `Modifications/<definition name>_<modification>`,
    /// else `Modifications/<class name>`. A patch file or Modification that cannot be found or
    /// read is an [issue](Self::issues); the implementation without it is still returned.
    ///
    /// # Errors
    ///
    /// Returns [`LoadoutError::MissingVehicleImplementation`] when no candidate file exists and
    /// [`LoadoutError::VehicleImplementation`] for unreadable or invalid `Vehicle` documents.
    pub fn read(
        files: &dyn LoadoutFiles,
        class_name: &str,
        definition: &str,
        modification: Option<&str>,
    ) -> Result<Self, LoadoutError> {
        load(
            files,
            class_name,
            definition,
            modification.unwrap_or_default().trim(),
        )
    }

    /// Returns the path of the implementation file as it was looked up: the candidate built from
    /// the Record's spelling with `/` as separator and without a leading `Data`.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Returns the path of the Modification's patch file that was applied, if one was.
    #[must_use]
    pub fn patch_path(&self) -> Option<&str> {
        self.patch_path.as_deref()
    }

    /// Returns the name of the Modification the Entity selects, if it selects one.
    #[must_use]
    pub fn modification(&self) -> Option<&str> {
        self.modification.as_deref()
    }

    /// Returns what applying the Modification did, or `None` when the Entity selects none.
    #[must_use]
    pub const fn modification_report(&self) -> Option<&ModificationReport> {
        self.report.as_ref()
    }

    /// Returns the `Elem` overrides of the Modification that were applied, in file order: those
    /// whose `idRef` some element has. Empty when the Entity selects no Modification.
    #[must_use]
    pub fn elems(&self) -> &[ElemOverride] {
        &self.elems
    }

    /// Returns the `Elem` that set the attribute `attribute` of the element with the `id` `id`,
    /// ignoring ASCII case in the attribute; the last one in file order, which is the one whose
    /// value stands.
    #[must_use]
    pub fn elem_overriding(&self, id: &str, attribute: &str) -> Option<&ElemOverride> {
        self.elems
            .iter()
            .rev()
            .find(|elem| elem.id_ref == id && elem.name.eq_ignore_ascii_case(attribute))
    }

    /// Returns the path of the file a value with `origin` was read from: the implementation file
    /// for [`ValueOrigin::Base`] and [`ValueOrigin::Elem`], whose `Elem` it holds, and the patch
    /// file for [`ValueOrigin::Patch`]. `None` when the origin is `Patch` and no patch file was
    /// applied.
    #[must_use]
    pub fn source_path(&self, origin: ValueOrigin) -> Option<&str> {
        match origin {
            ValueOrigin::Patch => self.patch_path(),
            ValueOrigin::Base | ValueOrigin::Elem => Some(self.path()),
        }
    }

    /// Returns the `name` of the `Vehicle` element.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the top-level parts, in file order; parts marked `skipPart` are left out.
    #[must_use]
    pub fn parts(&self) -> &[VehiclePart] {
        &self.parts
    }

    /// Returns why the implementation is not read in full, e.g. a patch file that is missing.
    #[must_use]
    pub fn issues(&self) -> &[LoadoutError] {
        &self.issues
    }

    /// Returns every part, depth first in file order.
    pub fn iter(&self) -> impl Iterator<Item = &VehiclePart> {
        let mut stack: Vec<&VehiclePart> = self.parts.iter().rev().collect();
        std::iter::from_fn(move || {
            let part = stack.pop()?;
            stack.extend(part.children.iter().rev());
            Some(part)
        })
    }

    /// Returns every id of a part or item port, the ones Modifications address.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.iter().flat_map(|part| {
            part.attribute("id")
                .into_iter()
                .chain(part.item_port.as_ref().and_then(VehicleItemPort::id))
        })
    }

    /// Returns the parts holding an item port.
    pub fn item_ports(&self) -> impl Iterator<Item = &VehiclePart> {
        self.iter().filter(|part| part.item_port.is_some())
    }

    /// Returns the treads.
    pub fn treads(&self) -> impl Iterator<Item = &VehiclePart> {
        self.iter().filter(|part| part.kind == PartKind::Tread)
    }

    /// Returns the wheels, the parts of class `SubPartWheel`.
    pub fn wheels(&self) -> impl Iterator<Item = &VehiclePart> {
        self.iter().filter(|part| part.kind == PartKind::Wheel)
    }

    /// Returns the visibility of the port `name`, matching the part names ignoring ASCII case;
    /// the first part of that name decides.
    #[must_use]
    pub fn port_visibility(&self, name: &str) -> PortVisibility {
        self.port_part(name)
            .and_then(|part| part.item_port.as_ref())
            .map(VehicleItemPort::visibility)
            .unwrap_or_default()
    }

    /// Returns the first part named `name` that holds an item port, ignoring ASCII case.
    pub(crate) fn port_part(&self, name: &str) -> Option<&VehiclePart> {
        let (first, rest) = self.port_paths.get(&port_key(name))?.split_first()?;
        let mut part = self.parts.get(*first)?;
        for index in rest {
            part = part.children.get(*index)?;
        }
        Some(part)
    }
}
