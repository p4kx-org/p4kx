// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircSemantic, BankNodeFxKind, BankNodePropertyKind, BankNodePropertyRows,
    BankObservedHeader, HircKind,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::bank::Bank;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

#[path = "hirc_node_source_positioning.rs"]
mod positioning;
#[path = "hirc_node_source_reader.rs"]
mod reader;
#[path = "hirc_node_source_routing.rs"]
mod routing;
#[path = "hirc_node_source_rtpc.rs"]
mod rtpc;
#[path = "hirc_node_source_state.rs"]
mod state;

use self::positioning::{Positioning, PositioningRows, PositioningSchema};
use self::reader::NodeReader;
use self::routing::{NodeAdvancedSettings, NodeAux, NodeFx};
use self::rtpc::{RtpcGroup, RtpcSchema};
use self::state::{InlineStateGroup, InlineStateSchema};
use super::Status;
use super::node_property::{PropertyExtent, PropertyWords};
use super::source_fields::origins::ObjectOrigins;

pub(super) type NodeSourceSchema =
    NodeDocument<SourceInventory, PositioningSchema, InlineStateSchema, RtpcSchema>;

/// Original shared HIRC node declarations for one selected physical occurrence.
#[derive(Debug)]
pub struct NodeSourceFields<'bank> {
    bank: &'bank Bank,
    index: usize,
    status: Status,
}

impl<'bank> NodeSourceFields<'bank> {
    pub(super) fn new(
        bank: &'bank Bank,
        index: usize,
        kind: HircKind,
        status: Status,
    ) -> Option<Self> {
        (bank.soundbank().header().has_shared_node_layout()
            && matches!(
                kind,
                HircKind::Sound
                    | HircKind::ActorMixer
                    | HircKind::RandomSequenceContainer
                    | HircKind::SwitchContainer
                    | HircKind::LayerContainer
                    | HircKind::MusicSegment
                    | HircKind::MusicTrack
                    | HircKind::MusicSwitch
                    | HircKind::MusicPlaylist
            ))
        .then_some(Self {
            bank,
            index,
            status,
        })
    }
}

impl Serialize for NodeSourceFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let origins = ObjectOrigins::shared_node(self.bank).map_err(S::Error::custom)?;
        self.bank
            .soundbank()
            .with_hirc_source_fields(self.index, |header, mut observed| {
                project(serializer, &origins, &header, &mut observed, self.status)
            })
            .map_err(S::Error::custom)?
            .ok_or_else(|| S::Error::custom("the selected original node is absent"))?
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "SharedNodeSourceFields")]
pub(super) struct NodeDocument<Inventory, Positioning, State, Rtpc> {
    version: SourcedValue<u32>,
    object_status: SourcedValue<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    override_bus_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    priority_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<PropertyList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ranged_modifiers: Option<PropertyList>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effects: Option<NodeFx>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata_effects: Option<NodeFx>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aux: Option<NodeAux>,
    #[serde(skip_serializing_if = "Option::is_none")]
    advanced: Option<NodeAdvancedSettings>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<PositioningSchema>")]
    positioning: Option<Positioning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<InlineStateSchema>")]
    inline_state: Option<State>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<RtpcSchema>")]
    rtpc: Option<Rtpc>,
    #[schemars(with = "SourceInventory")]
    source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "NodePropertyList")]
struct PropertyList {
    coverage: PropertyExtent,
    rows: Vec<PropertyWords>,
}

fn project<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    observed: &mut BankHircSemantic<'_, R>,
    status: Status,
) -> Result<S::Ok, S::Error> {
    let object_status = origins
        .coverage(status, header, observed.object())
        .map_err(S::Error::custom)?;
    let node = NodeReader::new(observed)
        .ok_or_else(|| S::Error::custom("unexpected original node layout"))?;
    let node = RefCell::new(node);
    let mut reader = node.try_borrow_mut().map_err(S::Error::custom)?;
    let parent = reader.parent();
    let parent_id = parent.and_then(|parent| parent.parent_id.as_ref());
    let override_bus_id = parent.and_then(|parent| parent.override_bus_id());
    let priority_flags = parent.and_then(|parent| parent.priority_flags.as_ref());
    let document = NodeDocument {
        version: origins.version(header).map_err(S::Error::custom)?,
        object_status,
        parent_id: origins
            .optional(parent_id, "nodeParentId")
            .map_err(S::Error::custom)?,
        override_bus_id: origins
            .optional(override_bus_id, "nodeOverrideBusId")
            .map_err(S::Error::custom)?,
        priority_flags: origins
            .optional(priority_flags, "nodePriorityFlags")
            .map_err(S::Error::custom)?,
        properties: property_list::<_, S::Error>(
            origins,
            reader.properties(BankNodePropertyKind::Properties),
        )?,
        ranged_modifiers: property_list::<_, S::Error>(
            origins,
            reader.properties(BankNodePropertyKind::RangedModifiers),
        )?,
        effects: NodeFx::read::<_, S::Error>(origins, &mut reader, BankNodeFxKind::Effects)?,
        metadata_effects: NodeFx::read::<_, S::Error>(
            origins,
            &mut reader,
            BankNodeFxKind::Metadata,
        )?,
        aux: NodeAux::read::<_, S::Error>(origins, &mut reader)?,
        advanced: NodeAdvancedSettings::read(
            origins,
            reader.playback().map(|fields| &fields.advanced),
        )
        .map_err(S::Error::custom)?,
        positioning: positioning::<_, S::Error>(origins, &node, &reader)?,
        inline_state: reader.states().is_some().then_some(InlineStateGroup {
            origins,
            node: &node,
        }),
        rtpc: reader.rtpc().is_some().then_some(RtpcGroup {
            origins,
            node: &node,
        }),
        source_inventory: origins,
    };
    drop(reader);
    document.serialize(serializer)
}

fn positioning<'view, 'source, 'input, R, E: serde::ser::Error>(
    origins: &'view ObjectOrigins,
    node: &'view RefCell<NodeReader<'source, 'input, R>>,
    reader: &NodeReader<'_, '_, R>,
) -> Result<Option<Positioning<PositioningRows<'view, 'source, 'input, R>>>, E> {
    reader
        .playback()
        .map(|fields| &fields.positioning)
        .filter(|fields| fields.start.is_some())
        .map(|fields| Positioning::read(origins, fields, PositioningRows { origins, node }))
        .transpose()
        .map_err(E::custom)
}

fn property_list<R: Read + Seek, E: serde::ser::Error>(
    origins: &ObjectOrigins,
    rows: Option<BankNodePropertyRows<'_, R>>,
) -> Result<Option<PropertyList>, E> {
    rows.map(|rows| {
        let coverage = PropertyExtent::read(origins, rows.extent()).map_err(E::custom)?;
        let rows = rows
            .map(|row| PropertyWords::read(origins, &row.map_err(E::custom)?))
            .collect::<Result<_, E>>()?;
        Ok(PropertyList { coverage, rows })
    })
    .transpose()
}
