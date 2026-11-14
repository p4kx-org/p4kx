// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dds_source_info::raw_dds_source_words;
use crate::texture_preview::PreviewHeaderEvidence;
use crate::{DdsHeaderField, DdsSourceInfo, FieldOrigins, SourceUseRole, SourceUses};

use super::DdsInfoOriginError;
use super::insert;

pub(super) fn capture(
    fields: &mut FieldOrigins,
    dds: &DdsSourceInfo,
    header: &PreviewHeaderEvidence,
    uses: &SourceUses,
    base: &str,
) -> Result<(), DdsInfoOriginError> {
    let prefix = header.header_uses(&[DdsHeaderField::Prefix], SourceUseRole::Selector)?;
    insert(
        fields,
        &format!("{base}/raw/magic_present"),
        uses.origin(&prefix, true),
    )?;
    let legacy_profile = uses.shared_origin(&prefix, true);
    let mut profile = prefix;
    let magic = u64::from(dds.raw.magic_present) * 4;
    profile.push(header.byte_use(
        magic + 80,
        4,
        "pixel_format/four_cc",
        SourceUseRole::Selector,
    )?);
    let profile = uses.shared_origin(&profile, true);
    insert(
        fields,
        &format!("{base}/raw/header_length"),
        uses.origin_with_shared(&profile, &[], true)?,
    )?;
    let dx10 = dds.raw.dx10.is_some();
    for word in raw_dds_source_words(dds.raw.magic_present, dx10) {
        let use_of_word =
            header.byte_use(word.offset, word.length, word.label, SourceUseRole::Payload)?;
        let selector = if word.dx10 { &profile } else { &legacy_profile };
        let origin = uses.origin_with_shared(selector, &[use_of_word], true)?;
        insert(fields, &format!("{base}/raw/{}", word.field), origin)?;
    }
    if !dx10 {
        insert(
            fields,
            &format!("{base}/raw/dx10"),
            uses.origin_with_shared(&profile, &[], true)?,
        )?;
    }
    Ok(())
}
