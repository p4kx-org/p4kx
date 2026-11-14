// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::BankError;
use p4kx_extraction::bank::{BankNodeParentFields, BankNodePropertyExtents, BankNodePropertyKind};

use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_node_properties::{NodePropertyProjection, SUMMARY_NAMESPACE};
use crate::audio_bank_node_property_payload::{
    NodePropertyObservations, NodePropertyPayload, NodePropertySummary,
};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_property_labels;
use crate::audio_bank_property_list;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) struct SourceNode<'source> {
    pub(crate) routing: &'source BankNodeParentFields,
    pub(crate) properties: &'source BankNodePropertyExtents,
}

impl SourceNode<'_> {
    fn checked_range(&self) -> Option<Range<u64>> {
        let bus = self.routing.override_bus_id()?;
        self.routing.parent_id.as_ref()?;
        self.routing.priority_flags.as_ref()?;
        let properties = self.properties.get(BankNodePropertyKind::Properties)?;
        let ranges = self.properties.get(BankNodePropertyKind::RangedModifiers)?;
        if properties.is_complete() && ranges.is_complete() {
            let end = ranges.words_range.as_ref()?.end;
            Some(bus.range.start..end)
        } else {
            None
        }
    }
}

pub(crate) fn supported(
    projection: &NodePropertyProjection<'_, '_>,
    original: Option<&SourceNode<'_>>,
    error: Option<&BankError>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = projection
        .sites
        .record(projection.bank.input, writer, None)?;
    let observed = if let Some(original) = original {
        observations(&projection.sites, &mut prepared, original)?
    } else {
        NodePropertyObservations::default()
    };
    let version = projection.version;
    let checked_range = original.and_then(SourceNode::checked_range);
    let (summary, status, issue, extent) = if let Some(range) = &checked_range {
        (
            NodePropertySummary::Decoded { version, observed },
            Status::Complete,
            None,
            range,
        )
    } else {
        let error = error
            .ok_or_else(|| MediaError::Contract("missing checked node property coverage".into()))?;
        let issue = MediaIssue::new(
            "audio.bank.invalid-node-properties",
            "HIRC",
            Severity::Error,
            error.to_string(),
        );
        (
            NodePropertySummary::Malformed { version, observed },
            Status::Partial,
            Some(issue),
            &projection.body,
        )
    };
    projection
        .sites
        .summary(&mut prepared, Some(extent), issue.is_some())?;
    write(
        projection,
        prepared.finish(),
        &summary,
        status,
        issue.as_slice(),
        writer,
    )
}

pub(crate) fn absent(
    projection: &NodePropertyProjection<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let summary = NodePropertySummary::Absent {
        version: projection.version,
    };
    unobserved(projection, &summary, Some(&projection.body), writer)
}

pub(crate) fn unsupported(
    projection: &NodePropertyProjection<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let summary = NodePropertySummary::Unsupported {
        version: projection.version,
    };
    unobserved(projection, &summary, None, writer)
}

fn unobserved(
    projection: &NodePropertyProjection<'_, '_>,
    summary: &NodePropertySummary,
    range: Option<&Range<u64>>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = projection
        .sites
        .record(projection.bank.input, writer, None)?;
    projection.sites.summary(&mut prepared, range, false)?;
    write(
        projection,
        prepared.finish(),
        summary,
        Status::Complete,
        &[],
        writer,
    )
}

fn write(
    projection: &NodePropertyProjection<'_, '_>,
    prepared: PreparedRecordOrigins,
    summary: &NodePropertySummary,
    status: Status,
    issues: &[MediaIssue],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    prepared.write(
        writer,
        &Subresource::new(SUMMARY_NAMESPACE, projection.sites.indices().to_vec()),
        status,
        &NodePropertyPayload::BankHircNodeProperties(summary),
        issues,
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    source: &SourceNode<'_>,
) -> Result<NodePropertyObservations, MediaError> {
    Ok(NodePropertyObservations {
        override_bus_id: sites.scalar(
            prepared,
            "observed/override_bus_id",
            source.routing.override_bus_id(),
            "nodeOverrideBusId",
        )?,
        parent_id: sites.scalar(
            prepared,
            "observed/parent_id",
            source.routing.parent_id.as_ref(),
            "nodeParentId",
        )?,
        priority_flags: sites.scalar(
            prepared,
            "observed/priority_flags",
            source.routing.priority_flags.as_ref(),
            "nodePriorityFlags",
        )?,
        properties: audio_bank_property_list::record(
            sites,
            prepared,
            "observed/properties",
            source.properties.get(BankNodePropertyKind::Properties),
            &audio_bank_property_labels::NODE,
        )?,
        ranged_modifiers: audio_bank_property_list::record(
            sites,
            prepared,
            "observed/ranged_modifiers",
            source.properties.get(BankNodePropertyKind::RangedModifiers),
            &audio_bank_property_labels::NODE,
        )?,
    })
}
