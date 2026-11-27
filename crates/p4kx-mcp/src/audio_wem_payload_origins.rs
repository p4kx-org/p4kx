// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::wem_payload_evidence::{MetadataField, PayloadField};

pub(super) use p4kx_extraction::wem_payload_evidence::{DidxWord, PayloadPlan};
pub(crate) use p4kx_extraction::wem_payload_evidence::{EvidenceError, metadata, put, range_use};

pub(super) const MCP_FIELDS: &[(&str, PayloadField)] = &[
    ("prefetch", PayloadField::Prefetch),
    ("plugin_media", PayloadField::Status),
    ("error", PayloadField::Status),
    ("codec", PayloadField::Metadata(MetadataField::Codec)),
    ("codec_name", PayloadField::Metadata(MetadataField::Codec)),
    ("channels", PayloadField::Metadata(MetadataField::Channels)),
    (
        "sample_rate",
        PayloadField::Metadata(MetadataField::SampleRate),
    ),
    (
        "sample_count",
        PayloadField::Metadata(MetadataField::SampleCount),
    ),
    (
        "duration_seconds",
        PayloadField::Metadata(MetadataField::Duration),
    ),
    ("looping", PayloadField::Metadata(MetadataField::Loop)),
];
