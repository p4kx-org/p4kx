// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankNodeFxKind, BankNodeFxList, BankNodeFxSlot, BankNodeFxSlots};

use crate::audio_bank_node_declaration_payload::{
    FxObservations, FxSlotRecord, NodeDeclarationPayload, NodeReferenceStatus,
};
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) const FX_NAMESPACE: &str = "audio-bank-hirc-node-fx";
pub(crate) const SLOT_NAMESPACE: &str = "audio-bank-hirc-node-fx-slot";

pub(crate) fn summary(
    declarations: &Declarations<'_, '_>,
    kind: BankNodeFxKind,
    disposition: Disposition<'_>,
    original: Option<BankNodeFxList<'_>>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::Fx);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(original) = original {
        observations(&sites, &mut prepared, kind, original)?
    } else {
        FxObservations::default()
    };
    let (payload, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &Coverage::read(
            original.and_then(|list| list.fields.start),
            original.and_then(BankNodeFxList::complete_range),
        ),
        observed,
        "audio.bank.invalid-node-fx",
    )?;
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(FX_NAMESPACE, vec![section, object, index(kind)?]),
        status,
        &NodeDeclarationPayload::Fx(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    kind: BankNodeFxKind,
    list: BankNodeFxList<'_>,
) -> Result<FxObservations, MediaError> {
    let (override_label, count_label) = match kind {
        BankNodeFxKind::Effects => ("nodeFxOverrideParentBits", "nodeFxCount"),
        BankNodeFxKind::Metadata => ("nodeMetadataOverrideParentBits", "nodeMetadataCount"),
        _ => {
            return Err(MediaError::Contract(
                "unexpected node effect list kind".into(),
            ));
        }
    };
    if let Some(range) = &list.fields.slots_range {
        sites.range(prepared, "observed/slots_range", range)?;
    }
    Ok(FxObservations {
        override_parent_bits: sites.scalar(
            prepared,
            "observed/override_parent_bits",
            list.fields.override_parent_bits.as_ref(),
            override_label,
        )?,
        declared_count: sites.scalar(
            prepared,
            "observed/declared_count",
            list.declared_count,
            count_label,
        )?,
        bypass_all_bits: sites.scalar(
            prepared,
            "observed/bypass_all_bits",
            list.fields.bypass_all_bits.as_ref(),
            "nodeFxBypassAllBits",
        )?,
        slots_range: list.fields.slots_range.as_ref().map(Into::into),
    })
}

pub(crate) fn slots<R: Read + Seek>(
    declarations: &Declarations<'_, '_>,
    kind: BankNodeFxKind,
    rows: BankNodeFxSlots<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::Fx);
    let group = index(kind)?;
    for slot in rows {
        let slot = slot.map_err(|cause| declarations.bank.replay_failure(cause))?;
        emit_slot(declarations, &sites, group, &slot, writer)?;
    }
    Ok(())
}

fn emit_slot(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    group: u64,
    slot: &BankNodeFxSlot,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, Some(&slot.index.range))?;
    let record = slot_record(sites, &mut prepared, slot)?;
    let (status, issue) = if matches!(record, FxSlotRecord::Malformed { .. }) {
        sites.issues(&mut prepared, &slot.checked_range)?;
        (
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-node-fx-slot",
                "HIRC",
                Severity::Error,
                "the original node effect slot ends before all fields fit",
            )),
        )
    } else {
        (Status::Complete, None)
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            SLOT_NAMESPACE,
            vec![section, object, group, u64::from(slot.ordinal)],
        ),
        status,
        &NodeDeclarationPayload::Slot(&record),
        issue.as_slice(),
    )
}

fn slot_record(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    slot: &BankNodeFxSlot,
) -> Result<FxSlotRecord, MediaError> {
    let range = &slot.index.range;
    sites.field(prepared, "slot_index", range, "nodeFxSlotIndex")?;
    sites.converted(prepared, "ordinal", range, "nodeFxSlotPosition")?;
    let effect = slot.effect_id.as_ref();
    let effect_id = sites.scalar(prepared, "effect_id", effect, "nodeFxReferenceId")?;
    let slot_bits = sites.scalar(prepared, "slot_bits", slot.bits.as_ref(), "nodeFxSlotBits")?;
    let reference_resolution = if let Some(id) = effect {
        sites.converted(
            prepared,
            "reference_resolution",
            &id.range,
            "nodeFxReferenceUnchecked",
        )?;
        Some(NodeReferenceStatus::Unchecked)
    } else {
        None
    };
    sites.converted(
        prepared,
        "layout",
        &slot.checked_range,
        "nodeFxSlotCoverage",
    )?;
    let ordinal = slot.ordinal;
    let slot_index = (&slot.index).into();
    match (effect_id, slot_bits, reference_resolution) {
        (Some(effect_id), Some(slot_bits), Some(reference_resolution)) => {
            Ok(FxSlotRecord::Decoded {
                ordinal,
                slot_index,
                effect_id,
                slot_bits,
                reference_resolution,
            })
        }
        (effect_id, None, reference_resolution) => Ok(FxSlotRecord::Malformed {
            ordinal,
            slot_index,
            effect_id,
            reference_resolution,
        }),
        _ => Err(MediaError::Contract(
            "missing earlier checked effect slot fields".into(),
        )),
    }
}

fn index(kind: BankNodeFxKind) -> Result<u64, MediaError> {
    match kind {
        BankNodeFxKind::Effects => Ok(0),
        BankNodeFxKind::Metadata => Ok(1),
        _ => Err(MediaError::Contract(
            "unexpected node effect list kind".into(),
        )),
    }
}
