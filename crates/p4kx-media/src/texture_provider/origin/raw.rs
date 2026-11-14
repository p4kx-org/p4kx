// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_preview::{self, DdsSourceWord};
use p4kx_extraction::{DdsRawHeader, SourceUseRole};

use super::{TextureHeaderOrigins, put};
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

impl TextureHeaderOrigins<'_> {
    pub(super) fn raw(
        &self,
        raw: &DdsRawHeader,
        base: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let magic = u64::from(raw.magic_present()) * 4;
        let mut prefix = writer.capture_origins();
        self.add_prefix(raw, &mut prefix, writer)?;
        put(
            prepared,
            &format!("{base}/magic_present"),
            prefix.finish(),
            writer,
        )?;
        let mut layout = writer.capture_origins();
        self.add_prefix(raw, &mut layout, writer)?;
        self.add_bytes(
            &mut layout,
            magic + 80,
            4,
            "pixelFormat.fourCc",
            SourceUseRole::Selector,
            writer,
        )?;
        let layout = layout.finish();
        put(
            prepared,
            &format!("{base}/header_length"),
            layout.clone(),
            writer,
        )?;
        if raw.dx10().is_none() {
            put(prepared, &format!("{base}/dx10"), layout, writer)?;
        }
        self.raw_words(raw, base, prepared, writer)
    }

    fn raw_words(
        &self,
        raw: &DdsRawHeader,
        base: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for word in texture_preview::raw_dds_source_words(raw.magic_present(), raw.dx10().is_some())
        {
            self.raw_word(raw, base, &word, prepared, writer)?;
        }
        Ok(())
    }
    fn raw_word(
        &self,
        raw: &DdsRawHeader,
        base: &str,
        word: &DdsSourceWord,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut capture = writer.capture_origins();
        self.add_prefix(raw, &mut capture, writer)?;
        if word.dx10 {
            self.add_bytes(
                &mut capture,
                u64::from(raw.magic_present()) * 4 + 80,
                4,
                "pixelFormat.fourCc",
                SourceUseRole::Selector,
                writer,
            )?;
        }
        self.add_bytes(
            &mut capture,
            word.offset,
            word.length,
            word.label,
            SourceUseRole::Payload,
            writer,
        )?;
        put(
            prepared,
            &format!("{base}/{}", word.field),
            capture.finish(),
            writer,
        )
    }
}
