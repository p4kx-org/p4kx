// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::BankError;
use p4kx_extraction::bank::{
    BankContainerRead, BankHircActorMixer, BankHircObject, BankHircSoundNode, BankNodeAuxFields,
    BankNodeFxKind, BankNodeFxSlots, BankNodeParentFields, BankNodePlaybackFields,
    BankNodeUserAuxBuses, BankPositioningRows, BankPositioningTableKind, BankSoundContinuation,
    BankSoundRead,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_advanced;
use crate::audio_bank_node_aux;
use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_fx;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_positioning;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_positioning_rows;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status};

pub(crate) const LISTS: [BankNodeFxKind; 2] = [BankNodeFxKind::Effects, BankNodeFxKind::Metadata];

pub(crate) enum Owner<'node, 'input, R> {
    Sound(&'node mut BankHircSoundNode<'input, R>),
    ActorMixer(&'node mut BankHircActorMixer<'input, R>),
}

pub(crate) struct Declarations<'context, 'source> {
    pub(crate) bank: &'context BankProjection<'source>,
    object: BankHircObject,
    version: VersionSite,
}

struct Source<'fields> {
    fields: Option<&'fields BankNodeParentFields>,
    aux: Option<&'fields BankNodeAuxFields>,
    playback: Option<&'fields BankNodePlaybackFields>,
    disposition: Disposition<'fields>,
}

#[derive(Clone, Copy)]
pub(crate) enum Disposition<'source> {
    Observed(Option<&'source BankError>),
    Absent,
    Unsupported,
}

pub(crate) enum Coverage {
    Complete(Range<u64>),
    Truncated(u64),
    Unavailable,
}

impl Coverage {
    pub(crate) const fn read(start: Option<u64>, complete: Option<Range<u64>>) -> Self {
        match (start, complete) {
            (_, Some(range)) => Self::Complete(range),
            (Some(start), None) => Self::Truncated(start),
            (None, None) => Self::Unavailable,
        }
    }
}

impl<R: Read + Seek> Owner<'_, '_, R> {
    const fn object(&self) -> BankHircObject {
        match self {
            Self::Sound(node) => node.sound().object,
            Self::ActorMixer(node) => *node.object(),
        }
    }

    fn source(&self) -> Result<Source<'_>, MediaError> {
        match self {
            Self::Sound(node) => sound_source(node),
            Self::ActorMixer(node) => {
                let fields = node.fields();
                let disposition = match node.status() {
                    BankContainerRead::Decoded => Disposition::Observed(None),
                    BankContainerRead::Malformed(error) => Disposition::Observed(Some(error)),
                    BankContainerRead::Unsupported(_) => Disposition::Unsupported,
                    _ => {
                        return Err(MediaError::Contract(
                            "unexpected ActorMixer disposition".into(),
                        ));
                    }
                };
                Ok(Source {
                    fields: Some(&fields.node),
                    aux: Some(&fields.aux),
                    playback: Some(&fields.playback),
                    disposition,
                })
            }
        }
    }

    fn user_aux_buses(
        &mut self,
        bank: &BankProjection<'_>,
    ) -> Result<Option<BankNodeUserAuxBuses>, MediaError> {
        match self {
            Self::Sound(node) => node.user_aux_buses(),
            Self::ActorMixer(node) => node.user_aux_buses(),
        }
        .map_err(|cause| bank.replay_failure(cause))
    }

    fn positioning_rows(
        &mut self,
        kind: BankPositioningTableKind,
    ) -> Option<BankPositioningRows<'_, R>> {
        match self {
            Self::Sound(node) => node.positioning_rows(kind),
            Self::ActorMixer(node) => node.positioning_rows(kind),
        }
    }

    fn slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        match self {
            Self::Sound(node) => node.fx_slots(kind),
            Self::ActorMixer(node) => node.fx_slots(kind),
        }
    }
}

fn sound_source<'node, R>(
    node: &'node BankHircSoundNode<'_, R>,
) -> Result<Source<'node>, MediaError> {
    let sound = node.sound();
    let disposition = match &sound.status {
        BankSoundRead::Decoded(_)
            if matches!(sound.fields.continuation, BankSoundContinuation::Absent) =>
        {
            Disposition::Absent
        }
        BankSoundRead::Decoded(_) => Disposition::Observed(None),
        BankSoundRead::Malformed(error) => Disposition::Observed(Some(error)),
        BankSoundRead::Unsupported(_) => Disposition::Unsupported,
        _ => {
            return Err(MediaError::Contract(
                "unexpected Sound node disposition".into(),
            ));
        }
    };
    let base = sound
        .fields
        .node
        .as_ref()
        .and_then(|node| node.base.as_ref());
    Ok(Source {
        fields: base.map(|base| &base.fields),
        aux: base.map(|base| &base.aux),
        playback: base.map(|base| &base.playback),
        disposition,
    })
}

pub(crate) fn emit<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    mut owner: Owner<'_, '_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations {
        bank,
        object: owner.object(),
        version,
    };
    let buses = owner.user_aux_buses(bank)?;
    let source = owner.source()?;
    for kind in LISTS {
        audio_bank_node_fx::summary(
            &declarations,
            kind,
            source.disposition,
            source.fields.map(|fields| fields.fx(kind)),
            writer,
        )?;
    }
    audio_bank_node_aux::summary(
        &declarations,
        source.disposition,
        source.aux,
        buses.as_ref(),
        writer,
    )?;
    playback_summaries(&declarations, source.disposition, source.playback, writer)?;
    for kind in LISTS {
        if let Some(slots) = owner.slots(kind) {
            audio_bank_node_fx::slots(&declarations, kind, slots, writer)?;
        }
    }
    for kind in audio_bank_node_positioning::TABLES {
        if let Some(rows) = owner.positioning_rows(kind) {
            audio_bank_positioning_rows::emit(&declarations, kind, rows, writer)?;
        }
    }
    Ok(())
}

fn playback_summaries(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    playback: Option<&BankNodePlaybackFields>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_node_positioning::summary(
        declarations,
        disposition,
        playback.map(|fields| &fields.positioning),
        writer,
    )?;
    audio_bank_node_advanced::summary(
        declarations,
        disposition,
        playback.map(|fields| &fields.advanced),
        writer,
    )
}

pub(crate) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declarations = Declarations {
        bank,
        object: *object,
        version,
    };
    for kind in LISTS {
        audio_bank_node_fx::summary(&declarations, kind, Disposition::Unsupported, None, writer)?;
    }
    audio_bank_node_aux::summary(&declarations, Disposition::Unsupported, None, None, writer)?;
    playback_summaries(&declarations, Disposition::Unsupported, None, writer)
}

impl Declarations<'_, '_> {
    pub(crate) const fn sites(&self, kind: NodeKind) -> NodeSites {
        NodeSites::new(&self.object, self.version, kind)
    }

    pub(crate) fn summary<T>(
        &self,
        sites: &NodeSites,
        prepared: &mut BankOrigins<'_, '_, '_>,
        disposition: Disposition<'_>,
        coverage: &Coverage,
        observed: T,
        issue_code: &'static str,
    ) -> Result<(NodeDeclaration<T>, Status, Option<MediaIssue>), MediaError> {
        let version = self.version.version;
        let body = self.object.body_offset()..self.object.body_end_exclusive();
        let truncated;
        let (payload, extent, issue) = match (disposition, coverage) {
            (Disposition::Observed(_), Coverage::Complete(range)) => (
                NodeDeclaration::Decoded { version, observed },
                Some(range),
                None,
            ),
            (Disposition::Observed(Some(error)), Coverage::Truncated(start)) => {
                truncated = *start..body.end;
                (
                    NodeDeclaration::Malformed { version, observed },
                    Some(&truncated),
                    Some(MediaIssue::new(
                        issue_code,
                        "HIRC",
                        Severity::Error,
                        error.to_string(),
                    )),
                )
            }
            (Disposition::Observed(Some(_)), Coverage::Unavailable) => {
                (NodeDeclaration::Unavailable { version }, Some(&body), None)
            }
            (Disposition::Observed(None), _) => {
                return Err(MediaError::Contract(
                    "missing checked node declaration coverage".into(),
                ));
            }
            (Disposition::Absent, _) => (NodeDeclaration::Absent { version }, Some(&body), None),
            (Disposition::Unsupported, _) => (NodeDeclaration::Unsupported { version }, None, None),
        };
        sites.summary(prepared, extent, issue.is_some())?;
        let status = if issue.is_some() {
            Status::Partial
        } else {
            Status::Complete
        };
        Ok((payload, status, issue))
    }
}
