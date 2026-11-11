// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{
    BankContainerChildren, BankContainerFields, BankContainerRead, BankHircContainerNode,
    ContainerKind,
};
use p4kx_extraction::{DecimalU64, HircKind};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_bank_container_node;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_payload::BankEntryRange;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_random_sequence;
use crate::audio_bank_switch_container;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_hirc_container_origins.rs"]
mod origins;

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-container";
pub(crate) const CHILD_NAMESPACE: &str = "audio-bank-hirc-container-child";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum ContainerPayload {
    BankHircContainer(ContainerRecord),
    BankHircContainerChild(ChildRecord),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContainerRecord {
    layout: Layout,
    container_kind: Option<Kind>,
    parent_id: Option<u32>,
    mode_code: Option<u8>,
    child_count: Option<u32>,
    node_range: Option<BankEntryRange>,
    children_range: Option<BankEntryRange>,
    suffix_range: Option<BankEntryRange>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "HircContainerLayout")]
enum Layout {
    Decoded,
    Malformed,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "HircContainerKind")]
enum Kind {
    Random,
    Sequence,
    Switch,
    Blend,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "HircContainerChildRecord")]
pub(crate) struct ChildRecord {
    ordinal: DecimalU64,
    child_id: u32,
    id_range: BankEntryRange,
}

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    mut container: BankHircContainerNode<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = origins::ContainerSites::new(container.object(), version, container.fields());
    header(
        projection,
        &sites,
        container.status(),
        container.fields(),
        writer,
    )?;
    children(projection, &sites, container.children(), writer)?;
    audio_bank_container_node::project(projection, version, &mut container, writer)?;
    match container.object().kind(version.version) {
        HircKind::RandomSequenceContainer => {
            audio_bank_random_sequence::project(projection, version, &mut container, writer)
        }
        HircKind::SwitchContainer => {
            audio_bank_switch_container::project(projection, version, &mut container, writer)
        }
        _ => Ok(()),
    }
}

fn header(
    projection: &BankProjection<'_>,
    sites: &origins::ContainerSites,
    disposition: &BankContainerRead,
    fields: &BankContainerFields,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (layout, status, issue) = interpretation(disposition)?;
    let payload = ContainerRecord::read(layout, fields)?;
    sites
        .header(projection.input, issue.is_some(), writer)?
        .write(
            writer,
            &Subresource::new(NAMESPACE, sites.object_indices().to_vec()),
            status,
            &ContainerPayload::BankHircContainer(payload),
            issue.as_slice(),
        )
}

fn children<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &origins::ContainerSites,
    observed: Option<BankContainerChildren<'_, R>>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if let Some(children) = observed {
        for child in children {
            let child = child.map_err(|cause| projection.replay_failure(cause))?;
            let ordinal = u64::from(child.ordinal);
            let payload = ChildRecord {
                ordinal: DecimalU64::new(ordinal),
                child_id: child.child_id.value,
                id_range: entry_range(&child.child_id.range),
            };
            sites.child(projection.input, &child, writer)?.write(
                writer,
                &Subresource::new(CHILD_NAMESPACE, sites.child_indices(ordinal).to_vec()),
                Status::Complete,
                &ContainerPayload::BankHircContainerChild(payload),
                &[],
            )?;
        }
    }
    Ok(())
}

fn interpretation(
    status: &BankContainerRead,
) -> Result<(Layout, Status, Option<MediaIssue>), MediaError> {
    match status {
        BankContainerRead::Decoded => Ok((Layout::Decoded, Status::Complete, None)),
        BankContainerRead::Malformed(error) => Ok((
            Layout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-container",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        )),
        BankContainerRead::Unsupported(error) => Ok((
            Layout::Unsupported,
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-container",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
        )),
        _ => Err(MediaError::Contract(
            "unexpected HIRC container read state".into(),
        )),
    }
}

impl ContainerRecord {
    fn read(layout: Layout, fields: &BankContainerFields) -> Result<Self, MediaError> {
        Ok(Self {
            layout,
            container_kind: fields.kind.map(Kind::try_from).transpose()?,
            parent_id: fields.node.parent_id.as_ref().map(|word| word.value),
            mode_code: fields.mode.as_ref().map(|word| word.value),
            child_count: fields.child_count.as_ref().map(|word| word.value),
            node_range: fields.node_range.as_ref().map(entry_range),
            children_range: fields.children.as_ref().map(entry_range),
            suffix_range: fields.suffix.as_ref().map(entry_range),
        })
    }
}

impl TryFrom<ContainerKind> for Kind {
    type Error = MediaError;

    fn try_from(kind: ContainerKind) -> Result<Self, Self::Error> {
        match kind {
            ContainerKind::Random => Ok(Self::Random),
            ContainerKind::Sequence => Ok(Self::Sequence),
            ContainerKind::Switch => Ok(Self::Switch),
            ContainerKind::Blend => Ok(Self::Blend),
            _ => Err(MediaError::Contract(
                "unexpected HIRC container kind".into(),
            )),
        }
    }
}

const fn entry_range(range: &Range<u64>) -> BankEntryRange {
    BankEntryRange::new(range.start, range.end)
}
