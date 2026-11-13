// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_extraction::bank::{
    BankNodePropertyKind, BankNodePropertyRow, BankNodePropertyRows, BankNodePropertyValue,
};

use crate::audio_bank_action_property_payload::{
    ActionPropertyPayload, PROPERTY_NAMESPACE as ACTION_PROPERTY_NAMESPACE,
};
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_node_properties::PROPERTY_NAMESPACE;
use crate::audio_bank_node_property_payload::{
    NodePropertyPayload, NodePropertyRecord, NodePropertyWords,
};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_property_labels::{self, PropertyLabels};
#[path = "audio_bank_voice_property.rs"]
mod voice;

pub(super) use self::voice::PropertyInterpretation;
use crate::{MediaError, MediaWriter, Status, Subresource};

#[derive(Clone, Copy)]
pub(crate) enum PropertyScope {
    Node,
    Action,
}

impl PropertyScope {
    const fn labels(self) -> &'static PropertyLabels {
        match self {
            Self::Node => &audio_bank_property_labels::NODE,
            Self::Action => &audio_bank_property_labels::ACTION,
        }
    }
}

pub(crate) struct PropertyRows<'context, 'source> {
    bank: &'context BankProjection<'source>,
    sites: &'context NodeSites,
    scope: PropertyScope,
}

impl<'context, 'source> PropertyRows<'context, 'source> {
    pub(crate) const fn new(
        bank: &'context BankProjection<'source>,
        sites: &'context NodeSites,
        scope: PropertyScope,
    ) -> Self {
        Self { bank, sites, scope }
    }
}

pub(crate) fn emit<R: Read + Seek>(
    projection: &PropertyRows<'_, '_>,
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
    projection: &PropertyRows<'_, '_>,
    list: u64,
    count: &Range<u64>,
    row: &BankNodePropertyRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = projection.sites;
    let labels = projection.scope.labels();
    let mut prepared = sites.record(projection.bank.input, writer, Some(&row.id.range))?;
    sites.field(&mut prepared, "property_id", &row.id.range, labels.id)?;
    sites.converted(&mut prepared, "ordinal", &row.id.range, labels.position)?;
    sites.converted(&mut prepared, "words/kind", count, labels.word_layout)?;
    let words = observed_words(sites, &mut prepared, &row.value, labels)?;
    let interpretation = match projection.scope {
        PropertyScope::Node => voice::project(sites, &mut prepared, row)?,
        PropertyScope::Action => None,
    };
    let data = NodePropertyRecord {
        ordinal: row.ordinal,
        property_id: (&row.id).into(),
        words,
        interpretation,
    };
    let [section, object] = sites.indices();
    let indices = vec![section, object, list, u64::from(row.ordinal)];
    let prepared = prepared.finish();
    match projection.scope {
        PropertyScope::Node => prepared.write(
            writer,
            &Subresource::new(PROPERTY_NAMESPACE, indices),
            Status::Complete,
            &NodePropertyPayload::BankHircNodeProperty(&data),
            &[],
        ),
        PropertyScope::Action => prepared.write(
            writer,
            &Subresource::new(ACTION_PROPERTY_NAMESPACE, indices),
            Status::Complete,
            &ActionPropertyPayload::Property(&data),
            &[],
        ),
    }
}

fn observed_words(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    value: &BankNodePropertyValue,
    labels: &PropertyLabels,
) -> Result<NodePropertyWords, MediaError> {
    Ok(match value {
        BankNodePropertyValue::Scalar(bits) => NodePropertyWords::Scalar {
            scalar_bits: sites.scalar(
                prepared,
                "words/scalar_bits",
                bits.as_ref(),
                labels.scalar_bits,
            )?,
        },
        BankNodePropertyValue::Ranged {
            minimum, maximum, ..
        } => NodePropertyWords::Ranged {
            minimum_bits: sites.scalar(
                prepared,
                "words/minimum_bits",
                minimum.as_ref(),
                labels.minimum_bits,
            )?,
            maximum_bits: sites.scalar(
                prepared,
                "words/maximum_bits",
                maximum.as_ref(),
                labels.maximum_bits,
            )?,
        },
        _ => {
            return Err(MediaError::Contract(
                "unexpected node property word layout".into(),
            ));
        }
    })
}
