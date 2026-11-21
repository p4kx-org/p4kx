// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankActionRead, BankHircAction, BankHircActionProperties, BankNodePropertyKind,
    BankPlayBankDecision, BankPlayProperties,
};

use crate::audio_bank_action_property_payload::{
    ActionPropertyObservations, ActionPropertyPayload, SUMMARY_NAMESPACE,
};
use crate::audio_bank_hirc_action;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_property_rows::{self, PropertyRows, PropertyScope};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_property_labels;
use crate::audio_bank_property_list;
use crate::hirc_object_index::HircObjectIndex;
use crate::{MediaError, MediaWriter, Subresource};

const LISTS: [BankNodePropertyKind; 2] = [
    BankNodePropertyKind::Properties,
    BankNodePropertyKind::RangedModifiers,
];

pub(crate) fn play<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    action: &mut BankHircActionProperties<'_, R>,
    objects: &HircObjectIndex,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = action.action();
    audio_bank_hirc_action::project(
        bank,
        &original.object,
        version,
        &original.status,
        objects,
        writer,
    )?;
    let declarations = Declarations::new(bank, &original.object, version);
    let disposition = match &original.status {
        BankActionRead::Decoded { .. } => Disposition::Observed(None),
        BankActionRead::Malformed { error, .. } => Disposition::Observed(Some(error)),
        _ => {
            return Err(MediaError::Contract(
                "unexpected Play property read state".into(),
            ));
        }
    };
    let sites = declarations.sites(NodeKind::ActionProperties);
    summary(
        &declarations,
        &sites,
        disposition,
        Some(action.fields()),
        writer,
    )?;
    let projection = PropertyRows::new(bank, &sites, PropertyScope::Action);
    for kind in LISTS {
        if let Some(rows) = action.properties(kind) {
            audio_bank_node_property_rows::emit(&projection, kind, rows, writer)?;
        }
    }
    Ok(())
}

pub(crate) fn unobserved(
    bank: &BankProjection<'_>,
    version: VersionSite,
    action: &BankHircAction,
    objects: &HircObjectIndex,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_hirc_action::project(
        bank,
        &action.object,
        version,
        &action.status,
        objects,
        writer,
    )?;
    let disposition = match &action.status {
        BankActionRead::NotAction => return Ok(()),
        BankActionRead::Unsupported { .. } => Disposition::Unsupported,
        BankActionRead::Decoded { fields, .. } | BankActionRead::Malformed { fields, .. } => {
            match &fields.play_bank {
                BankPlayBankDecision::VersionGated => Disposition::Unsupported,
                BankPlayBankDecision::NonPlay => Disposition::Absent,
                BankPlayBankDecision::Unreached
                    if !BankPlayProperties::supports_version(version.version) =>
                {
                    Disposition::Unsupported
                }
                BankPlayBankDecision::Unreached => {
                    let BankActionRead::Malformed { error, .. } = &action.status else {
                        return Err(MediaError::Contract(
                            "missing original Action property disposition".into(),
                        ));
                    };
                    Disposition::Observed(Some(error))
                }
                _ => {
                    return Err(MediaError::Contract(
                        "missing original Play property rows".into(),
                    ));
                }
            }
        }
        _ => {
            return Err(MediaError::Contract(
                "unexpected Action property read state".into(),
            ));
        }
    };
    let declarations = Declarations::new(bank, &action.object, version);
    let sites = declarations.sites(NodeKind::ActionProperties);
    summary(&declarations, &sites, disposition, None, writer)
}

fn summary(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    disposition: Disposition<'_>,
    original: Option<&BankPlayProperties>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(fields) = original {
        observations(sites, &mut prepared, fields)?
    } else {
        ActionPropertyObservations::default()
    };
    let coverage = Coverage::read(
        original.map(|fields| fields.start),
        original.and_then(BankPlayProperties::complete_range),
    );
    let (payload, status, issue) = declarations.summary(
        sites,
        &mut prepared,
        disposition,
        &coverage,
        observed,
        "audio.bank.invalid-action-properties",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(SUMMARY_NAMESPACE, sites.indices().to_vec()),
        status,
        &ActionPropertyPayload::Summary(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankPlayProperties,
) -> Result<ActionPropertyObservations, MediaError> {
    Ok(ActionPropertyObservations {
        target_flags: sites.scalar(
            prepared,
            "observed/target_flags",
            fields.target_flags.as_ref(),
            "actionTargetFlags",
        )?,
        fade_flags: sites.scalar(
            prepared,
            "observed/fade_flags",
            fields.fade_flags.as_ref(),
            "actionFadeFlags",
        )?,
        properties: audio_bank_property_list::record(
            sites,
            prepared,
            "observed/properties",
            fields.lists.get(BankNodePropertyKind::Properties),
            &audio_bank_property_labels::ACTION,
        )?,
        ranged_modifiers: audio_bank_property_list::record(
            sites,
            prepared,
            "observed/ranged_modifiers",
            fields.lists.get(BankNodePropertyKind::RangedModifiers),
            &audio_bank_property_labels::ACTION,
        )?,
    })
}
