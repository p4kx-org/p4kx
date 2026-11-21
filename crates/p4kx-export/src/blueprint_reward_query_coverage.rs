// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::BlueprintRewardSource;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::source_value_catalog::SourceRecordReference;

pub(super) fn incomplete(row: &BlueprintRewardSource<'_>) -> bool {
    header_incomplete(row.source())
        || [
            row.source_ordinal().origin(),
            row.pool().origin(),
            row.pool_storage().origin(),
            row.contract_id().origin(),
            row.contract_name().origin(),
            row.chance().origin(),
        ]
        .into_iter()
        .any(|origin| !origin.source_set_complete())
        || row
            .pool()
            .value()
            .is_some_and(|reference| reference_incomplete(reference))
}

fn header_incomplete(header: &CanonicalRecordHeader<'_>) -> bool {
    [
        header.guid.origin(),
        header.record_name().origin(),
        header.class_name().origin(),
        header.record_path().origin(),
        header.record_index().origin(),
        header.source_type().origin(),
        header.file_name().origin(),
        header.metadata_tag().origin(),
    ]
    .into_iter()
    .any(|origin| !origin.source_set_complete())
}

fn reference_incomplete(reference: &SourceRecordReference<'_>) -> bool {
    [
        reference.guid.origin(),
        reference.target_record_index().origin(),
        reference.record_path().origin(),
        reference.target_type().origin(),
    ]
    .into_iter()
    .any(|origin| !origin.source_set_complete())
}
