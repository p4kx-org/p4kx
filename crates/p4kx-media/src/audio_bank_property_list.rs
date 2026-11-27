// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::BankNodePropertyExtent;

use crate::MediaError;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_node_property_payload::NodePropertyListRecord;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_property_labels::PropertyLabels;

pub(crate) fn record(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    path: &str,
    extent: Option<&BankNodePropertyExtent>,
    labels: &PropertyLabels,
) -> Result<Option<NodePropertyListRecord>, MediaError> {
    let Some(extent) = extent else {
        return Ok(None);
    };
    let count = &extent.declared_count;
    sites.field(
        prepared,
        &format!("{path}/declared_count"),
        &count.range,
        labels.count,
    )?;
    sites.range(prepared, &format!("{path}/ids_range"), &extent.ids_range)?;
    let ids = count.range.start..extent.ids_range.end;
    sites.converted(
        prepared,
        &format!("{path}/checked_id_count"),
        &ids,
        labels.checked_ids,
    )?;
    let end = if let Some(words) = &extent.words_range {
        sites.range(prepared, &format!("{path}/words_range"), words)?;
        words.end
    } else {
        ids.end
    };
    let inspected = count.range.start..end;
    sites.converted(
        prepared,
        &format!("{path}/checked_word_count"),
        &inspected,
        labels.checked_words,
    )?;
    sites.converted(
        prepared,
        &format!("{path}/complete"),
        &inspected,
        labels.coverage,
    )?;
    Ok(Some(NodePropertyListRecord {
        declared_count: count.into(),
        checked_id_count: extent.checked_id_count(),
        checked_word_count: extent.checked_word_count(),
        ids_range: (&extent.ids_range).into(),
        words_range: extent.words_range.as_ref().map(Into::into),
        complete: extent.is_complete(),
    }))
}
