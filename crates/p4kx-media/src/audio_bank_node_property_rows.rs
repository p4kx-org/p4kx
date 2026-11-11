// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{
    BankNodePropertyKind, BankNodePropertyRow, BankNodePropertyRows, BankNodePropertyValue,
};

use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_node_properties::{NodePropertyProjection, PROPERTY_NAMESPACE};
use crate::audio_bank_node_property_payload::{
    NodePropertyPayload, NodePropertyRecord, NodePropertyWords,
};
use crate::audio_bank_node_property_summary;
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(crate) fn emit<R: Read + Seek>(
    projection: &NodePropertyProjection<'_, '_>,
    kind: BankNodePropertyKind,
    rows: BankNodePropertyRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let list = match kind {
        BankNodePropertyKind::Properties => 0,
        BankNodePropertyKind::RangedModifiers => 1,
        _ => {
            return Err(MediaError::Contract(
                "unexpected node property list kind".into(),
            ));
        }
    };
    let count = rows.extent().declared_count.range.clone();
    for row in rows {
        let row = row.map_err(|cause| projection.bank.replay_failure(cause))?;
        emit_row(projection, list, &count, &row, writer)?;
    }
    Ok(())
}

fn emit_row(
    projection: &NodePropertyProjection<'_, '_>,
    list: u64,
    count: &Range<u64>,
    row: &BankNodePropertyRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = &projection.sites;
    let mut prepared = sites.record(projection.bank.input, writer, Some(&row.id.range))?;
    sites.field(
        &mut prepared,
        "property_id",
        &row.id.range,
        "nodePropertyId",
    )?;
    sites.converted(
        &mut prepared,
        "ordinal",
        &row.id.range,
        "nodePropertyPosition",
    )?;
    sites.converted(&mut prepared, "words/kind", count, "nodePropertyWordLayout")?;
    let words = observed_words(sites, &mut prepared, &row.value)?;
    let data = NodePropertyRecord {
        ordinal: row.ordinal,
        property_id: (&row.id).into(),
        words,
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            PROPERTY_NAMESPACE,
            vec![section, object, list, u64::from(row.ordinal)],
        ),
        Status::Complete,
        &NodePropertyPayload::BankHircNodeProperty(&data),
        &[],
    )
}

fn observed_words(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    value: &BankNodePropertyValue,
) -> Result<NodePropertyWords, MediaError> {
    Ok(match value {
        BankNodePropertyValue::Scalar(bits) => NodePropertyWords::Scalar {
            scalar_bits: audio_bank_node_property_summary::word(
                sites,
                prepared,
                "words/scalar_bits",
                bits.as_ref(),
                "nodePropertyScalarBits",
            )?,
        },
        BankNodePropertyValue::Ranged {
            minimum, maximum, ..
        } => NodePropertyWords::Ranged {
            minimum_bits: audio_bank_node_property_summary::word(
                sites,
                prepared,
                "words/minimum_bits",
                minimum.as_ref(),
                "nodePropertyMinimumBits",
            )?,
            maximum_bits: audio_bank_node_property_summary::word(
                sites,
                prepared,
                "words/maximum_bits",
                maximum.as_ref(),
                "nodePropertyMaximumBits",
            )?,
        },
        _ => {
            return Err(MediaError::Contract(
                "unexpected node property word layout".into(),
            ));
        }
    })
}
