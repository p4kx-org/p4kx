// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::Guid;

use crate::LoadoutEntry;
use crate::entities::class_name_of;
use crate::spec::LoadoutOrigin;
use crate::{
    BodyVariant, Geometry, Incompatibility, ItemFacts, LandingGear, LoadoutError,
    ObjectContainerRef, PortConstraints, PortDefinition, PortVisibility, VehicleImplementation,
    ZoneChunk,
};

/// How the geometry of an Entity in a Loadout tree was chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum GeometryChoice {
    /// The Record's main geometry, nothing chose another.
    #[default]
    Main,
    /// The flags of the port the Entity sits at chose a sub-geometry.
    Flag,
    /// The body of the character wearing the Entity chose a sub-geometry.
    Body(BodyVariant),
    /// The Entity is worn, has no sub-geometry per body and names only a world prop; the prop
    /// is not drawn and `fallback` holds it.
    WorldProp,
}

/// One Entity in a Loadout tree: its Record and its ports with what they hold.
#[derive(Debug, Clone, PartialEq)]
pub struct EntityNode {
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) name: String,
    pub(crate) ports: Vec<PortNode>,
    pub(crate) issues: Vec<LoadoutError>,
    pub(crate) notes: Vec<LoadoutError>,
    pub(crate) geometry: Option<Geometry>,
    pub(crate) fallback: Option<Geometry>,
    pub(crate) choice: GeometryChoice,
    pub(crate) vehicle: Option<Arc<VehicleImplementation>>,
    pub(crate) landing_gear: Arc<[LandingGear]>,
    pub(crate) docked_vehicle: bool,
    pub(crate) loadout_file: Option<Arc<str>>,
    pub(crate) expanded: bool,
    pub(crate) attach_type: String,
    pub(crate) attach_sub_type: String,
    pub(crate) size: Option<u32>,
    pub(crate) container: Option<Guid>,
    pub(crate) object_containers: Arc<[ObjectContainerRef]>,
    pub(crate) zones: Arc<[ZoneChunk]>,
    pub(crate) hidden_ports: Arc<[String]>,
}

impl EntityNode {
    /// Returns the names of the ports whose items are not drawn when the Entity is worn, from the
    /// `HiddenParts` of its Record: a hat hides the hair. The names are in lower case, as the
    /// reader of the Record lowers them; compare them ignoring case.
    #[must_use]
    pub fn hidden_ports(&self) -> &[String] {
        &self.hidden_ports
    }

    /// Returns what the Entity offers a port: its item type, sub-type and size.
    #[must_use]
    pub fn item_facts(&self) -> ItemFacts {
        ItemFacts {
            class_name: self.class_name().to_owned(),
            item_type: self.attach_type.clone(),
            sub_type: self.attach_sub_type.clone(),
            size: self.size,
        }
    }

    /// Returns the item type of the Entity's `AttachDef.Type`, e.g. `CargoGrid`; empty when the
    /// Record has none.
    #[must_use]
    pub fn attach_type(&self) -> &str {
        &self.attach_type
    }

    /// Returns the GUID of the `InventoryContainer` Record the Entity's
    /// `SCItemInventoryContainerComponentParams` name in `containerParams`, if it names one.
    #[must_use]
    pub const fn inventory_container(&self) -> Option<Guid> {
        self.container
    }

    /// Returns the vehicle implementation the Entity's Record names, with the Modification it
    /// selects applied, or `None` when the Record names none, the files were not given or the
    /// Entity is a docked vehicle the resolver stopped at.
    #[must_use]
    pub fn vehicle(&self) -> Option<&VehicleImplementation> {
        self.vehicle.as_deref()
    }

    /// Returns the landing gears of the `VehicleLandingGearSystem` Record the Entity's vehicle
    /// component names.
    #[must_use]
    pub fn landing_gear(&self) -> &[LandingGear] {
        &self.landing_gear
    }

    /// Returns whether the Entity is a docked vehicle: attached to a port and its
    /// `AttachDef.Type` starts with `NOITEM_Vehicle`, e.g. a snub craft in a ship's hangar port.
    /// The root of the tree is never one.
    #[must_use]
    pub const fn is_docked_vehicle(&self) -> bool {
        self.docked_vehicle
    }

    /// Returns the path of the XML Loadout file the Entity's own Loadout names
    /// (`SItemPortLoadoutXMLParams.loadoutPath`), or `None` for a Loadout in the Record.
    #[must_use]
    pub fn loadout_file(&self) -> Option<&str> {
        self.loadout_file.as_deref()
    }

    /// Returns whether the Entity's ports are built; `false` for a docked vehicle the resolver
    /// stopped at, whose ports are left out.
    #[must_use]
    pub const fn is_expanded(&self) -> bool {
        self.expanded
    }

    /// Returns the socpak containers the Entity's Record names: the vehicle's object containers
    /// with their bone and offset, then the item's own container.
    #[must_use]
    pub fn object_containers(&self) -> &[ObjectContainerRef] {
        &self.object_containers
    }

    /// Returns the geometry the Entity's Record names, or `None` when it names none.
    #[must_use]
    pub const fn geometry(&self) -> Option<&Geometry> {
        self.geometry.as_ref()
    }

    /// Returns the GUID of the Entity's Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path, without `.xml`.
    #[must_use]
    pub fn record_path(&self) -> &str {
        &self.record_path
    }

    /// Returns the Record name, e.g. `EntityClassDefinition.AEGS_Avenger_Titan`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the class name, the Record name after `EntityClassDefinition.`.
    #[must_use]
    pub fn class_name(&self) -> &str {
        class_name_of(&self.name).unwrap_or(&self.name)
    }

    /// Returns the ports: those the Loadouts name in Loadout order, then the defined ports no
    /// Loadout names in definition order.
    #[must_use]
    pub fn ports(&self) -> &[PortNode] {
        &self.ports
    }

    /// Returns why the Entity's own Loadout cannot be read in full, e.g. a missing XML Loadout.
    #[must_use]
    pub fn issues(&self) -> &[LoadoutError] {
        &self.issues
    }

    /// Returns what the Loadout of the Entity only notes: nothing was kept from being read, as in
    /// an undecided Requirement of a loadout assortment. One note per assortment.
    #[must_use]
    pub fn notes(&self) -> &[LoadoutError] {
        &self.notes
    }

    /// Returns every port in the tree below this Entity, depth first in port order, with its
    /// path of port names joined by `/`.
    #[must_use]
    pub fn walk(&self) -> Vec<(String, &PortNode)> {
        let mut ports = Vec::new();
        walk(self, "", &mut ports);
        ports
    }

    /// Returns the default Loadout as one entry per port holding something, an Entity or an
    /// attachment that cannot be resolved, with its path of port names joined by `/`, depth
    /// first in port order. Ports that hold nothing are left out; [`Self::walk`] lists them.
    #[must_use]
    pub fn loadout(&self) -> Vec<LoadoutEntry<'_>> {
        self.walk()
            .into_iter()
            .filter(|(_, port)| !matches!(port.attachment, Attachment::Empty))
            .map(|(path, port)| LoadoutEntry { path, port })
            .collect()
    }

    /// Returns every port in the tree that cannot be resolved, with its path of port names
    /// joined by `/`, depth first in port order.
    pub fn unresolved(&self) -> impl Iterator<Item = (String, &Unresolved)> {
        self.walk()
            .into_iter()
            .filter_map(|(path, port)| match &port.attachment {
                Attachment::Unresolved(unresolved) => Some((path, unresolved)),
                _ => None,
            })
    }
}

fn walk<'n>(node: &'n EntityNode, prefix: &str, ports: &mut Vec<(String, &'n PortNode)>) {
    walk_ports(&node.ports, prefix, ports);
}

fn walk_ports<'n>(list: &'n [PortNode], prefix: &str, ports: &mut Vec<(String, &'n PortNode)>) {
    for port in list {
        let path = if prefix.is_empty() {
            port.name.clone()
        } else {
            format!("{prefix}/{}", port.name)
        };
        ports.push((path.clone(), port));
        walk_ports(&port.nested, &path, ports);
        if let Attachment::Entity(child) = &port.attachment {
            walk(child, &path, ports);
        }
    }
}

/// One port of an Entity in a Loadout tree.
#[derive(Debug, Clone, PartialEq)]
pub struct PortNode {
    pub(crate) name: String,
    pub(crate) source: PortSource,
    pub(crate) definition: Option<Arc<PortDefinition>>,
    pub(crate) constraints: Option<Arc<PortConstraints>>,
    pub(crate) attachment: Attachment,
    pub(crate) nested: Vec<Self>,
    pub(crate) visibility: PortVisibility,
    pub(crate) origin: Option<LoadoutOrigin>,
}

impl PortNode {
    /// Returns what supplied the port's attachment: the Record or XML Loadout file of the
    /// Loadout entry, or the Record of the port definition for a port default. `None` for a port
    /// no entry or default names, and for a port the changes excluded or replaced.
    #[must_use]
    pub const fn origin(&self) -> Option<&LoadoutOrigin> {
        self.origin.as_ref()
    }

    /// Returns what the port takes: the types and sizes of its definition in the Entity Record,
    /// else of the item port of the vehicle implementation. `None` when neither states any.
    #[must_use]
    pub fn constraints(&self) -> Option<&PortConstraints> {
        self.constraints.as_deref()
    }

    /// Checks that an Entity fits the port; a port without [`Self::constraints`] takes any.
    ///
    /// # Errors
    ///
    /// Returns what does not fit.
    pub fn accepts(&self, item: &ItemFacts) -> Result<(), Incompatibility> {
        self.constraints()
            .map_or(Ok(()), |constraints| constraints.check(item))
    }

    /// Returns whether the port is shown: what the vehicle implementation's part of that name
    /// says ([`PortVisibility::Invisible`] when it carries the flag `invisible`), else
    /// [`PortVisibility::Invisible`] when the port definition carries that flag, else
    /// [`PortVisibility::Unspecified`].
    #[must_use]
    pub const fn visibility(&self) -> PortVisibility {
        self.visibility
    }

    /// Returns the ports of the nested Loadout of an empty port: a Loadout entry that names no
    /// Entity but carries entries of its own, e.g. the gear below a character's `Body_ItemPort`.
    /// Their source is [`PortSource::Override`]; empty for every other port.
    #[must_use]
    pub fn nested(&self) -> &[Self] {
        &self.nested
    }

    /// Returns the port name, as the Loadout or the port definition spells it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns where the port's attachment comes from.
    #[must_use]
    pub const fn source(&self) -> PortSource {
        self.source
    }

    /// Returns the port's definition in the Entity Record, or `None` when the Record defines no
    /// port of this name, e.g. a ship port from its vehicle definition.
    #[must_use]
    pub fn definition(&self) -> Option<&PortDefinition> {
        self.definition.as_deref()
    }

    /// Returns what the port holds.
    #[must_use]
    pub const fn attachment(&self) -> &Attachment {
        &self.attachment
    }
}

/// Where the attachment of a port comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PortSource {
    /// The Entity's own default Loadout.
    Loadout,
    /// The Loadout the entry attaching the Entity gives, which overrides the Entity's own.
    Override,
    /// The default item of the port definition, used when no Loadout names the port.
    PortDefault,
    /// Nothing names the port; it is defined but holds nothing.
    Unset,
    /// Only the Entity's vehicle implementation declares the port: no Loadout entry names it and
    /// the Record defines no port of that name. It holds nothing and can take an Entity through
    /// [`LoadoutChanges::replace`](crate::LoadoutChanges::replace).
    Empty,
    /// The port is left out by [`LoadoutChanges::exclude`](crate::LoadoutChanges::exclude); it
    /// holds nothing.
    Excluded,
    /// The port holds the Entity [`LoadoutChanges::replace`](crate::LoadoutChanges::replace)
    /// names, with that Entity's own Loadout.
    Replaced,
}

impl PortSource {
    /// Returns the stable code of the source, e.g. `override`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Loadout => "loadout",
            Self::Override => "override",
            Self::PortDefault => "port_default",
            Self::Unset => "unset",
            Self::Empty => "empty",
            Self::Excluded => "excluded",
            Self::Replaced => "replaced",
        }
    }
}

/// What a port holds.
#[derive(Debug, Clone, PartialEq)]
pub enum Attachment {
    /// Nothing.
    Empty,
    /// An Entity with its own ports.
    Entity(Box<EntityNode>),
    /// An Entity that cannot be resolved, with the reason.
    Unresolved(Unresolved),
}

/// An attachment that cannot be resolved: what was named and why it fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    pub(crate) target: Target,
    pub(crate) error: LoadoutError,
}

impl Unresolved {
    /// Returns what the Loadout names.
    #[must_use]
    pub const fn target(&self) -> &Target {
        &self.target
    }

    /// Returns why it cannot be resolved.
    #[must_use]
    pub const fn error(&self) -> &LoadoutError {
        &self.error
    }
}

/// What a Loadout names to attach to a port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// A Record by Reference.
    Reference(Guid),
    /// An Entity by class name.
    ClassName(String),
    /// A Reference that cannot be read.
    Unreadable,
}
