// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankNodeAdvancedFields;

use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_playback_payload::{AdvancedObservations, NodePlaybackPayload};
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-node-advanced-settings";

pub(crate) fn summary(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    original: Option<&BankNodeAdvancedFields>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::AdvancedSettings);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(fields) = original {
        observations(&sites, &mut prepared, fields)?
    } else {
        AdvancedObservations::default()
    };
    let (payload, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &Coverage::read(
            original.and_then(|fields| fields.start),
            original.and_then(BankNodeAdvancedFields::complete_range),
        ),
        observed,
        "audio.bank.invalid-node-advanced-settings",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(NAMESPACE, sites.indices().to_vec()),
        status,
        &NodePlaybackPayload::Advanced(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankNodeAdvancedFields,
) -> Result<AdvancedObservations, MediaError> {
    Ok(AdvancedObservations {
        flags: sites.scalar(
            prepared,
            "observed/flags",
            fields.flags.as_ref(),
            "nodeAdvancedFlags",
        )?,
        virtual_queue_behavior: sites.scalar(
            prepared,
            "observed/virtual_queue_behavior",
            fields.virtual_queue_behavior.as_ref(),
            "nodeAdvancedVirtualQueueBehavior",
        )?,
        max_instances: sites.scalar(
            prepared,
            "observed/max_instances",
            fields.max_instances.as_ref(),
            "nodeAdvancedMaxInstances",
        )?,
        below_threshold_behavior: sites.scalar(
            prepared,
            "observed/below_threshold_behavior",
            fields.below_threshold_behavior.as_ref(),
            "nodeAdvancedBelowThresholdBehavior",
        )?,
        behavior_flags: sites.scalar(
            prepared,
            "observed/behavior_flags",
            fields.behavior_flags.as_ref(),
            "nodeAdvancedBehaviorFlags",
        )?,
    })
}
