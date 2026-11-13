// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_preview::{self, PreviewHeaderEvidence};
use p4kx_extraction::{DdsHeaderField, DdsSourceInfo, FieldOrigins, SourceUseRole, SourceUses};

use super::insert;
use crate::PackageResult;
use crate::texture_source_metadata::metadata_error;

pub(super) fn capture(
    fields: &mut FieldOrigins,
    dds: &DdsSourceInfo,
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
) -> PackageResult<()> {
    let prefix = header
        .header_uses(&[DdsHeaderField::Prefix], SourceUseRole::Selector)
        .map_err(metadata_error)?;
    insert(fields, "/dds/raw/magic_present", uses.origin(&prefix, true))?;
    let legacy_profile = uses.shared_origin(&prefix, true);
    let mut profile = prefix;
    let magic = u64::from(dds.raw.magic_present) * 4;
    profile.push(
        header
            .byte_use(
                magic + 80,
                4,
                "pixel_format/four_cc",
                SourceUseRole::Selector,
            )
            .map_err(metadata_error)?,
    );
    let profile = uses.shared_origin(&profile, true);
    insert(
        fields,
        "/dds/raw/header_length",
        uses.origin_with_shared(&profile, &[], true)
            .map_err(metadata_error)?,
    )?;
    let dx10 = dds.raw.dx10.is_some();
    for word in texture_preview::raw_dds_source_words(dds.raw.magic_present, dx10) {
        let use_of_word = header
            .byte_use(word.offset, word.length, word.label, SourceUseRole::Payload)
            .map_err(metadata_error)?;
        let selector = if word.dx10 { &profile } else { &legacy_profile };
        let origin = uses
            .origin_with_shared(selector, &[use_of_word], true)
            .map_err(metadata_error)?;
        insert(fields, &format!("/dds/raw/{}", word.field), origin)?;
    }
    if !dx10 {
        insert(
            fields,
            "/dds/raw/dx10",
            uses.origin_with_shared(&profile, &[], true)
                .map_err(metadata_error)?,
        )?;
    }
    Ok(())
}
