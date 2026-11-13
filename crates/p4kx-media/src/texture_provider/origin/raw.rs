// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DdsRawHeader, SourceUseRole};

use super::{TextureHeaderOrigins, put};
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

const LEGACY_WORDS: &[(&str, u64)] = &[
    ("size", 0),
    ("flags", 4),
    ("height", 8),
    ("width", 12),
    ("pitch_or_linear_size", 16),
    ("depth", 20),
    ("mip_map_count", 24),
    ("pixel_format/size", 72),
    ("pixel_format/flags", 76),
    ("pixel_format/rgb_bit_count", 84),
    ("pixel_format/r_bit_mask", 88),
    ("pixel_format/g_bit_mask", 92),
    ("pixel_format/b_bit_mask", 96),
    ("pixel_format/a_bit_mask", 100),
    ("caps", 104),
    ("caps2", 108),
    ("caps3", 112),
    ("caps4", 116),
    ("reserved2", 120),
];

const EXTENSION_WORDS: &[(&str, u64)] = &[
    ("dxgi_format", 124),
    ("resource_dimension", 128),
    ("misc_flag", 132),
    ("array_size", 136),
    ("misc_flags2", 140),
];

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
        let magic = u64::from(raw.magic_present()) * 4;
        for &(name, offset) in LEGACY_WORDS {
            self.raw_word(
                raw,
                &format!("{base}/legacy/{name}"),
                (name, magic + offset, 4),
                prepared,
                writer,
            )?;
        }
        for index in 0..11 {
            self.raw_word(
                raw,
                &format!("{base}/legacy/reserved1/{index}"),
                ("reserved1", magic + 28 + index * 4, 4),
                prepared,
                writer,
            )?;
        }
        for index in 0..4 {
            self.raw_word(
                raw,
                &format!("{base}/legacy/pixel_format/four_cc/{index}"),
                ("pixelFormat.fourCc", magic + 80 + index, 1),
                prepared,
                writer,
            )?;
        }
        if raw.dx10().is_some() {
            for &(name, offset) in EXTENSION_WORDS {
                self.raw_word(
                    raw,
                    &format!("{base}/dx10/{name}"),
                    (name, magic + offset, 4),
                    prepared,
                    writer,
                )?;
            }
        }
        Ok(())
    }

    fn raw_word(
        &self,
        raw: &DdsRawHeader,
        path: &str,
        (name, offset, length): (&str, u64, u64),
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut capture = writer.capture_origins();
        self.add_prefix(raw, &mut capture, writer)?;
        if path.contains("/dx10/") {
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
            offset,
            length,
            name,
            SourceUseRole::Payload,
            writer,
        )?;
        put(prepared, path, capture.finish(), writer)
    }
}
