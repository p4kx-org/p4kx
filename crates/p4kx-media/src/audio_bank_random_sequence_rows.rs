// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankRandomSequenceItem, BankRandomSequenceRows};

use crate::audio_bank_node_declaration_payload::NodeReferenceStatus;
use crate::audio_bank_node_declarations::Declarations;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_random_sequence_payload::{
    ITEM_NAMESPACE, RandomSequenceItemRecord, RandomSequencePayload,
};
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) fn project<R: Read + Seek>(
    declarations: &Declarations<'_, '_>,
    rows: BankRandomSequenceRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::RandomSequence);
    for row in rows {
        let row = row.map_err(|cause| declarations.bank.replay_failure(cause))?;
        emit(declarations, &sites, &row, writer)?;
    }
    Ok(())
}

fn emit(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    row: &BankRandomSequenceItem,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(declarations.bank.input, writer, Some(&row.checked_range))?;
    let record = read(sites, &mut prepared, row)?;
    let (status, issue) = if matches!(record, RandomSequenceItemRecord::Malformed { .. }) {
        sites.issues(&mut prepared, &row.checked_range)?;
        (
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-random-sequence-item",
                "HIRC",
                Severity::Error,
                "the original playlist item ends before the weight word fits",
            )),
        )
    } else {
        (Status::Complete, None)
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            ITEM_NAMESPACE,
            vec![section, object, u64::from(row.ordinal)],
        ),
        status,
        &RandomSequencePayload::Item(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    row: &BankRandomSequenceItem,
) -> Result<RandomSequenceItemRecord, MediaError> {
    let range = &row.play_id.range;
    sites.field(prepared, "play_id", range, "randomSequencePlayId")?;
    sites.converted(prepared, "ordinal", range, "randomSequencePlaylistPosition")?;
    sites.converted(
        prepared,
        "layout",
        &row.checked_range,
        "randomSequencePlaylistCoverage",
    )?;
    sites.converted(
        prepared,
        "reference_resolution",
        range,
        "randomSequenceReferenceUnchecked",
    )?;
    let ordinal = row.ordinal;
    let play_id = (&row.play_id).into();
    let reference_resolution = NodeReferenceStatus::Unchecked;
    let Some(bits) = row.weight_bits.as_ref() else {
        return Ok(RandomSequenceItemRecord::Malformed {
            ordinal,
            play_id,
            reference_resolution,
        });
    };
    let weight = row
        .signed_weight()
        .ok_or_else(|| MediaError::Contract("missing checked playlist weight".into()))?;
    sites.field(
        prepared,
        "weight_bits",
        &bits.range,
        "randomSequenceWeightBits",
    )?;
    sites.converted(
        prepared,
        "weight",
        &bits.range,
        "randomSequenceSignedWeight",
    )?;
    Ok(RandomSequenceItemRecord::Decoded {
        ordinal,
        play_id,
        weight_bits: bits.into(),
        weight,
        reference_resolution,
    })
}
