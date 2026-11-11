// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{
    BankContainerRead, BankHircActorMixer, BankHircObject, BankHircSoundNode, BankNodePropertyKind,
    BankSoundContinuation, BankSoundRead,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_property_rows::{self, PropertyRows, PropertyScope};
use crate::audio_bank_node_property_summary::{self, SourceNode};
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaWriter};

pub(crate) const SUMMARY_NAMESPACE: &str = "audio-bank-hirc-node-properties";
pub(crate) const PROPERTY_NAMESPACE: &str = "audio-bank-hirc-node-property";

const LISTS: [BankNodePropertyKind; 2] = [
    BankNodePropertyKind::Properties,
    BankNodePropertyKind::RangedModifiers,
];

pub(crate) struct NodePropertyProjection<'context, 'source> {
    pub(crate) bank: &'context BankProjection<'source>,
    pub(crate) sites: NodeSites,
    pub(crate) version: u32,
    pub(crate) body: Range<u64>,
}

impl<'context, 'source> NodePropertyProjection<'context, 'source> {
    const fn new(
        bank: &'context BankProjection<'source>,
        object: &BankHircObject,
        version: VersionSite,
    ) -> Self {
        Self {
            bank,
            sites: NodeSites::new(object, version, NodeKind::Properties),
            version: version.version,
            body: object.body_offset()..object.body_end_exclusive(),
        }
    }
}

pub(crate) fn sound<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    node: &mut BankHircSoundNode<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sound = node.sound();
    let projection = NodePropertyProjection::new(bank, &sound.object, version);
    let error = match &sound.status {
        BankSoundRead::Decoded(_) => None,
        BankSoundRead::Malformed(error) => Some(error),
        BankSoundRead::Unsupported(_) => {
            return audio_bank_node_property_summary::unsupported(&projection, writer);
        }
        _ => {
            return Err(MediaError::Contract(
                "unexpected Sound property source disposition".into(),
            ));
        }
    };
    if matches!(sound.fields.continuation, BankSoundContinuation::Absent) {
        return audio_bank_node_property_summary::absent(&projection, writer);
    }
    let original = sound
        .fields
        .node
        .as_ref()
        .and_then(|node| node.base.as_ref())
        .map(|base| SourceNode {
            routing: &base.fields,
            properties: &base.properties,
        });
    audio_bank_node_property_summary::supported(&projection, original.as_ref(), error, writer)?;
    for kind in LISTS {
        if let Some(rows) = node.properties(kind) {
            let properties = PropertyRows::new(bank, &projection.sites, PropertyScope::Node);
            audio_bank_node_property_rows::emit(&properties, kind, rows, writer)?;
        }
    }
    Ok(())
}

pub(crate) fn actor_mixer<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    actor: &mut BankHircActorMixer<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let projection = NodePropertyProjection::new(bank, actor.object(), version);
    let error = match actor.status() {
        BankContainerRead::Decoded => None,
        BankContainerRead::Malformed(error) => Some(error),
        BankContainerRead::Unsupported(_) => {
            return audio_bank_node_property_summary::unsupported(&projection, writer);
        }
        _ => {
            return Err(MediaError::Contract(
                "unexpected ActorMixer property source disposition".into(),
            ));
        }
    };
    let fields = actor.fields();
    let original = SourceNode {
        routing: &fields.node,
        properties: &fields.properties,
    };
    audio_bank_node_property_summary::supported(&projection, Some(&original), error, writer)?;
    for kind in LISTS {
        if let Some(rows) = actor.properties(kind) {
            let properties = PropertyRows::new(bank, &projection.sites, PropertyScope::Node);
            audio_bank_node_property_rows::emit(&properties, kind, rows, writer)?;
        }
    }
    Ok(())
}

pub(crate) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let projection = NodePropertyProjection::new(bank, object, version);
    audio_bank_node_property_summary::unsupported(&projection, writer)
}
