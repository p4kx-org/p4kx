// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank_wem_evidence::{BankWemEvidence, BankWemRead, BankWemReadMode};
use p4kx_extraction::wem_payload_evidence::{EvidenceError, MetadataField, PayloadField, put};
use p4kx_extraction::{Bank, FieldOrigins, SourceInventory};

use super::{WemJson, describe};

const FIELDS: &[(&str, PayloadField)] = &[
    ("class", PayloadField::Class),
    ("prefetch", PayloadField::Prefetch),
    ("error", PayloadField::Status),
    ("codec", PayloadField::Metadata(MetadataField::Codec)),
    ("codec_name", PayloadField::Metadata(MetadataField::Codec)),
    (
        "sample_count",
        PayloadField::Metadata(MetadataField::SampleCount),
    ),
    (
        "duration_seconds",
        PayloadField::Metadata(MetadataField::Duration),
    ),
];

pub(super) struct Listing {
    pub(super) wems: Vec<WemJson>,
    pub(super) field_origins: FieldOrigins,
    pub(super) provenance: SourceInventory,
}

pub(super) fn list(bank: &Bank) -> Result<Listing, EvidenceError> {
    let evidence = BankWemEvidence::new(bank)?;
    let mut field_origins = FieldOrigins::new();
    evidence.header("", &mut field_origins)?;
    put(&mut field_origins, "", "wems", evidence.membership()?)?;
    let wems = bank
        .soundbank()
        .wems()
        .enumerate()
        .map(|(index, wem)| {
            let read = BankWemRead::read(wem, BankWemReadMode::SoundCandidates);
            evidence.row(&format!("/wems/{index}"), &read, FIELDS, &mut field_origins)?;
            Ok(describe(&read))
        })
        .collect::<Result<_, EvidenceError>>()?;
    Ok(Listing {
        wems,
        field_origins,
        provenance: evidence.finish()?,
    })
}
