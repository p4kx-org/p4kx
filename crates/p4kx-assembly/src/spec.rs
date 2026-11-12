// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_dataforge::{Guid, RecordTree, StructValue};

use crate::LoadoutXmlClassUse;
use crate::armour::{ZoneList, read_zones};
use crate::geometry::GeometryComponent;
use crate::loadout_path::PathRead;
use crate::loadout_reference::LoadoutReference;
use crate::loadout_xml_use::XmlRoute;
use crate::manual_loadout::ManualLoadout;
use crate::merge::port_key;
use crate::outfit_read::OutfitInfo;
use crate::port::{PortDefinition, PortDefinitionPosition, read_definition};
use crate::read::{indexed_struct_items, struct_field, text, unsigned};
pub(crate) use crate::spec_loadout::read_loadout;
use crate::target_spec;
pub(crate) use crate::target_spec::TargetSpec;
use crate::{
    Geometry, GeometryVariant, ItemFacts, LoadoutPath, LoadoutPathOperation, MaterialVariant,
    ObjectContainerRef,
};

const GEOMETRY_RESOURCE: &str = "SGeometryResourceParams";
const DEFAULT_LOADOUT: &str = "SEntityComponentDefaultLoadoutParams";
const PORT_CONTAINER: &str = "SItemPortContainerComponentParams";
const OUTFIT_LOADOUT: &str = "SItemPortLoadoutOutfitParams";
const VEHICLE_PARAMS: &str = "VehicleComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const INVENTORY_CONTAINER: &str = "SCItemInventoryContainerComponentParams";
const DOCKED_VEHICLE_TYPE: &str = "NOITEM_Vehicle";

/// A Loadout as an Entity Record or a Loadout entry states it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LoadoutSpec {
    /// Entries in the Record.
    Manual(ManualLoadout),
    /// An XML Loadout file, by the path the Record names.
    Xml(LoadoutPath),
    /// Loadouts laid over each other in order, an outfit (`SItemPortLoadoutOutfitParams`): its
    /// base Loadout, then the Loadouts of the outfits that always apply. An entry without an
    /// Entity adds its nested entries to those of the earlier entry of the same port.
    Layered(Arc<[Self]>),
    /// A Loadout generated at random from `SLoadoutAssortment` Records: the Records by GUID.
    Assortments(Arc<[LoadoutReference]>),
}

/// What supplied the attachment of a port: an Entity Record, or an XML Loadout file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadoutOrigin {
    /// The Loadout entries or the port definitions of this Entity Record, by record path.
    Record(Arc<str>),
    /// The XML Loadout file with this path, the `loadoutPath` a Record names or an include.
    File(Arc<str>),
}

impl LoadoutOrigin {
    /// Returns the record path or the file path.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            Self::Record(path) | Self::File(path) => path,
        }
    }

    /// Returns whether the origin is an XML Loadout file.
    #[must_use]
    pub const fn is_file(&self) -> bool {
        matches!(self, Self::File(_))
    }
}

/// One entry of a Loadout: a port, what it holds and the Loadout given to that.
#[derive(Debug, Clone)]
pub(crate) struct EntrySpec {
    pub(crate) origin: LoadoutOrigin,
    pub(crate) port: String,
    pub(crate) target: TargetSpec,
    pub(crate) loadout: Option<LoadoutSpec>,
    pub(crate) xml_route: Option<Arc<XmlRoute>>,
    pub(crate) xml_use: Option<LoadoutXmlClassUse>,
}

/// One item port an Entity Record defines.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PortSpec {
    pub(crate) name: String,
    pub(crate) definition: Arc<PortDefinition>,
    pub(crate) default_item: TargetSpec,
}

/// The vehicle implementation a Record names and the Modification it selects.
#[derive(Debug, Clone)]
pub(crate) struct VehicleSpec {
    pub(crate) definition: LoadoutPath,
    pub(crate) modification: LoadoutPath,
    pub(crate) class_name: Option<LoadoutPath>,
    pub(crate) landing_system: Option<LoadoutReference>,
    pub(crate) component: usize,
}

impl PartialEq for VehicleSpec {
    fn eq(&self, other: &Self) -> bool {
        self.definition == other.definition
            && self.modification == other.modification
            && self.landing_system == other.landing_system
    }
}

impl Eq for VehicleSpec {}

/// What the Loadout tree needs of one Entity Record.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct EntitySpec {
    pub(crate) record_index: Option<u32>,
    pub(crate) guid: Guid,
    pub(crate) record_path: String,
    pub(crate) origin: LoadoutOrigin,
    pub(crate) name: String,
    pub(crate) ports: Vec<PortSpec>,
    pub(crate) port_flags: Vec<String>,
    pub(crate) port_index: HashMap<String, usize>,
    pub(crate) loadout: Option<LoadoutSpec>,
    pub(crate) outfit: Option<OutfitInfo>,
    pub(crate) geometry: Option<Geometry>,
    pub(crate) variants: Vec<GeometryVariant>,
    pub(crate) vehicle: Option<VehicleSpec>,
    pub(crate) docked_vehicle: bool,
    pub(crate) attach_type: String,
    pub(crate) attach_sub_type: String,
    pub(crate) attachable_component: Option<usize>,
    pub(crate) attach_tags: Vec<String>,
    pub(crate) material_variants: Vec<MaterialVariant>,
    pub(crate) size: Option<u32>,
    pub(crate) container: Option<Guid>,
    pub(crate) object_containers: Arc<[ObjectContainerRef]>,
    pub(crate) zones: ZoneList,
    pub(crate) animation_database: Option<Arc<str>>,
}

impl EntitySpec {
    /// Reads the ports and the default Loadout from the components of an Entity's tree.
    pub(crate) fn read(tree: &RecordTree<'_>, paths: &PathRead<'_>) -> Self {
        let components = components_of(tree);
        let record: Arc<str> = Arc::from(tree.record_path());
        let (loadout, outfit) = read_default_loadout(&components, &record, paths);
        let (ports, port_flags) = read_ports(tree, paths);
        let mut port_index = HashMap::with_capacity(ports.len());
        for (at, port) in ports.iter().enumerate() {
            port_index.entry(port_key(&port.name)).or_insert(at);
        }
        let geometry_component =
            component_slots(tree).find(|(_, component)| component.type_name() == GEOMETRY_RESOURCE);
        let position = |ordinal| GeometryComponent {
            record: tree.record_index(),
            ordinal,
        };
        let geometry = geometry_component
            .and_then(|(ordinal, component)| Geometry::read(component, position(ordinal)));
        let variants = geometry_component
            .map(|(ordinal, component)| Geometry::read_variants(component, position(ordinal)))
            .unwrap_or_default();
        let vehicle = component_slots(tree)
            .find(|(_, component)| component.type_name() == VEHICLE_PARAMS)
            .map(|(ordinal, component)| read_vehicle(component, ordinal, tree, paths));
        let attachable = attachable_component(tree);
        let definition = attachable.and_then(|(_, component)| struct_field(component, "AttachDef"));
        let attach_type = definition.map_or("", |definition| text(definition, "Type"));
        Self {
            attach_sub_type: definition
                .map_or("", |definition| text(definition, "SubType"))
                .to_owned(),
            attach_tags: definition
                .map(|definition| tag_words(text(definition, "Tags")))
                .unwrap_or_default(),
            material_variants: geometry_component
                .map(|(_, component)| MaterialVariant::read(component))
                .unwrap_or_default(),
            size: definition.and_then(|definition| unsigned(definition, "Size")),
            docked_vehicle: is_docked_vehicle_type(attach_type),
            attach_type: attach_type.to_owned(),
            attachable_component: attachable.map(|(ordinal, _)| ordinal),
            container: inventory_container(&components),
            object_containers: crate::object_containers::read(&components).into(),
            zones: read_zones(&components),
            animation_database: crate::creature_animation::database_of(&components),
            record_index: Some(tree.record_index()),
            guid: tree.guid(),
            record_path: tree.record_path().to_owned(),
            origin: LoadoutOrigin::Record(record),
            name: tree.name().to_owned(),
            ports,
            port_flags,
            port_index,
            loadout,
            outfit,
            geometry,
            variants,
            vehicle,
        }
    }

    /// Returns the first port this Entity defines with the name `name`, ignoring ASCII case.
    pub(crate) fn port(&self, name: &str) -> Option<&PortSpec> {
        self.port_index
            .get(&port_key(name))
            .and_then(|at| self.ports.get(*at))
    }

    /// Returns what the Entity offers a port.
    pub(crate) fn item_facts(&self) -> ItemFacts {
        ItemFacts {
            class_name: crate::entities::class_name_of(&self.name)
                .unwrap_or(&self.name)
                .to_owned(),
            item_type: self.attach_type.clone(),
            sub_type: self.attach_sub_type.clone(),
            size: self.size,
        }
    }

    /// Returns the targets the port defaults of the Entity name.
    pub(crate) fn default_targets(&self) -> impl Iterator<Item = &TargetSpec> {
        self.ports.iter().map(|port| &port.default_item)
    }

    /// Returns the entries of the Entity's own Loadout when they are written out in the Record.
    pub(crate) fn manual_entries(&self) -> Option<&[EntrySpec]> {
        match &self.loadout {
            Some(LoadoutSpec::Manual(entries)) => Some(entries),
            _ => None,
        }
    }
}

/// Reads the default Loadout among `components` and, when it is an outfit, what the report shows.
fn read_default_loadout(
    components: &[&StructValue<'_>],
    record: &Arc<str>,
    paths: &PathRead<'_>,
) -> (Option<LoadoutSpec>, Option<OutfitInfo>) {
    let default_loadout = components
        .iter()
        .find(|component| component.type_name() == DEFAULT_LOADOUT)
        .and_then(|component| struct_field(component, "loadout"));
    match default_loadout {
        Some(loadout) if loadout.type_name() == OUTFIT_LOADOUT => {
            let read = crate::outfit_read::read(loadout, record, true, paths);
            (read.loadout, read.info)
        }
        Some(loadout) => (read_loadout(loadout, record, paths), None),
        None => (None, None),
    }
}

/// Reads the item ports of the port containers among `components`, and the flags the containers
/// give all their ports.
fn read_ports(tree: &RecordTree<'_>, paths: &PathRead<'_>) -> (Vec<PortSpec>, Vec<String>) {
    let record_index = tree.record_index();
    let containers =
        || component_slots(tree).filter(|(_, component)| component.type_name() == PORT_CONTAINER);
    let ports = containers()
        .flat_map(|(component_index, component)| {
            indexed_struct_items(component, "Ports").map(move |(port_index, port)| PortSpec {
                name: text(port, "Name").to_owned(),
                definition: Arc::new(read_definition(
                    port,
                    PortDefinitionPosition {
                        record: record_index,
                        component: component_index,
                        port: port_index,
                    },
                )),
                default_item: struct_field(port, "defaultItem")
                    .and_then(|item| {
                        item.field("entityClass")
                            .map(|value| target_spec::reference_target_at(item, value, paths))
                    })
                    .unwrap_or(TargetSpec::None),
            })
        })
        .collect();
    let flags = containers()
        .flat_map(|(_, component)| text(component, "PortFlags").split_whitespace())
        .map(str::to_owned)
        .collect();
    (ports, flags)
}

fn component_slots<'v, 'f>(
    tree: &'v RecordTree<'f>,
) -> impl Iterator<Item = (usize, &'v StructValue<'f>)> {
    indexed_struct_items(tree.root(), "Components")
}

fn components_of<'v, 'f>(tree: &'v RecordTree<'f>) -> Vec<&'v StructValue<'f>> {
    component_slots(tree)
        .map(|(_, component)| component)
        .collect()
}

/// Reads the main geometry an Entity Record names, without its ports and Loadout.
pub(crate) fn read_geometry(tree: &RecordTree<'_>) -> Option<Geometry> {
    let (ordinal, component) =
        component_slots(tree).find(|(_, component)| component.type_name() == GEOMETRY_RESOURCE)?;
    Geometry::read(
        component,
        GeometryComponent {
            record: tree.record_index(),
            ordinal,
        },
    )
}

/// Reads what an Entity Record offers a port, with its class name taken from `name`.
pub(crate) fn read_item_facts(tree: &RecordTree<'_>) -> ItemFacts {
    let definition =
        attachable_component(tree).and_then(|(_, component)| struct_field(component, "AttachDef"));
    let name = tree.name();
    ItemFacts {
        class_name: crate::entities::class_name_of(name)
            .unwrap_or(name)
            .to_owned(),
        item_type: definition
            .map_or("", |definition| text(definition, "Type"))
            .to_owned(),
        sub_type: definition
            .map_or("", |definition| text(definition, "SubType"))
            .to_owned(),
        size: definition.and_then(|definition| unsigned(definition, "Size")),
    }
}

fn read_vehicle(
    component: &StructValue<'_>,
    ordinal: usize,
    tree: &RecordTree<'_>,
    paths: &PathRead<'_>,
) -> VehicleSpec {
    VehicleSpec {
        component: ordinal,
        definition: paths.field(component, "vehicleDefinition", LoadoutPathOperation::Trim),
        modification: paths.field(component, "modification", LoadoutPathOperation::Trim),
        class_name: paths.record_name(tree),
        landing_system: match component
            .field("landingSystem")
            .map(|value| target_spec::reference_target_at(component, value, paths))
        {
            Some(TargetSpec::Reference(reference)) => Some(reference),
            _ => None,
        },
    }
}

fn attachable_component<'v, 'f>(tree: &'v RecordTree<'f>) -> Option<(usize, &'v StructValue<'f>)> {
    component_slots(tree).find(|(_, component)| component.type_name() == ATTACHABLE)
}

fn inventory_container(components: &[&StructValue<'_>]) -> Option<Guid> {
    components
        .iter()
        .find(|component| component.type_name() == INVENTORY_CONTAINER)
        .and_then(|component| component.field("containerParams"))
        .and_then(|value| match target_spec::reference_target(value) {
            TargetSpec::Reference(reference) => Some(reference.guid()),
            _ => None,
        })
}

/// Returns whether an `AttachDef.Type` marks a docked vehicle: it starts with `NOITEM_Vehicle`,
/// ignoring ASCII case.
fn is_docked_vehicle_type(attach_type: &str) -> bool {
    attach_type
        .as_bytes()
        .get(..DOCKED_VEHICLE_TYPE.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(DOCKED_VEHICLE_TYPE.as_bytes()))
}

/// Returns the words of a tag list as the game writes it: white space separated, a word such as
/// `$$hairVar_Brown++` stands for the tag `hairVar_Brown`.
fn tag_words(tags: &str) -> Vec<String> {
    tags.split_whitespace()
        .map(|word| word.trim_matches(['$', '+']))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}
