// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DdsHeaderField, DdsRawHeader, FieldPath, InputScope, SourceUseRole};

use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contract::OriginProfile;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::BuiltinOrigins;
use crate::{MediaError, MediaWriter, origin_error};

mod raw;

use DdsHeaderField::{ArraySize, Depth, Faces, Format, Height, MipCount, Prefix, Width};

pub(super) struct TextureHeaderOrigins<'a> {
    input: &'a MediaOriginInput,
    entry: u64,
    inspected: u64,
}

impl<'a> TextureHeaderOrigins<'a> {
    pub(super) const fn new(input: &'a MediaOriginInput, entry: usize, inspected: u64) -> Self {
        Self {
            input,
            entry: entry as u64,
            inspected,
        }
    }

    pub(super) fn valid(
        &self,
        raw: &DdsRawHeader,
        has_layout: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = self.prepared(writer)?;
        let base = "/data/payload/source_metadata";
        let independent = OriginContributions::independent(&self.input.owner);
        put(
            &mut prepared,
            &format!("{base}/schema_version"),
            independent,
            writer,
        )?;
        put(
            &mut prepared,
            &format!("{base}/header_source_bytes"),
            self.entry_field("uncompressedSize", SourceUseRole::Payload, writer)?,
            writer,
        )?;
        self.raw(raw, &format!("{base}/raw"), &mut prepared, writer)?;
        self.normalized(raw, base, &mut prepared, writer)?;
        let format = self.fields(raw, &[Format], writer)?.finish();
        put(
            &mut prepared,
            "/data/payload/decoder",
            format.clone(),
            writer,
        )?;
        let split = if has_layout {
            self.split_layout(raw, writer)?
        } else {
            format
        };
        put(&mut prepared, "/data/payload/split_parts", split, writer)?;
        Ok(prepared)
    }

    pub(super) fn invalid(
        &self,
        raw: Option<&DdsRawHeader>,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = self.prepared(writer)?;
        let size = self.entry_field("uncompressedSize", SourceUseRole::Payload, writer)?;
        put(
            &mut prepared,
            "/data/payload/header_source_bytes",
            size.clone(),
            writer,
        )?;
        let rejected = if let Some(raw) = raw {
            self.raw(raw, "/data/payload/raw", &mut prepared, writer)?;
            self.fields(
                raw,
                &[Format, Width, Height, Depth, MipCount, Faces, ArraySize],
                writer,
            )?
            .finish()
        } else {
            let mut capture = writer.capture_origins();
            if self.inspected != 0 {
                self.add_bytes(
                    &mut capture,
                    0,
                    self.inspected,
                    "structuralHeader",
                    SourceUseRole::Conversion,
                    writer,
                )?;
            }
            capture.include(&size, writer)?;
            let rejected = capture.finish();
            put(&mut prepared, "/data/payload/raw", rejected.clone(), writer)?;
            rejected
        };
        put(&mut prepared, "/issues", rejected, writer)?;

        Ok(prepared)
    }

    fn prepared(&self, writer: &MediaWriter<'_>) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = PreparedRecordOrigins::new(&self.input.owner);
        prepared.set_builtin(BuiltinOrigins {
            profile: OriginProfile::TextureHeader,
            input: self.input.clone(),
            scope: InputScope::Payload,
        })?;
        put(
            &mut prepared,
            "/data/payload/alpha_tail",
            self.entry_field("path", SourceUseRole::Selector, writer)?,
            writer,
        )?;
        put(
            &mut prepared,
            "/subresource/indices",
            OriginContributions::independent(&self.input.owner),
            writer,
        )?;
        Ok(prepared)
    }

    pub(super) fn split_layout(
        &self,
        raw: &DdsRawHeader,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = self.fields(
            raw,
            &[Format, Width, Height, Depth, MipCount, Faces, ArraySize],
            writer,
        )?;
        capture.include(
            &self.entry_field("uncompressedSize", SourceUseRole::Selector, writer)?,
            writer,
        )?;
        Ok(capture.finish())
    }

    pub(super) fn entry_field(
        &self,
        field: &str,
        role: SourceUseRole,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.add(
            self.input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[self.entry], field),
            role,
            writer,
        )?;
        Ok(capture.finish())
    }

    fn normalized(
        &self,
        raw: &DdsRawHeader,
        base: &str,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (path, field) in [
            ("width", Width),
            ("height", Height),
            ("depth", Depth),
            ("mip_count", MipCount),
            ("faces", Faces),
            ("cubemap", Faces),
            ("array_size", ArraySize),
            ("format", Format),
            ("reader_high_range", Format),
            ("block_layout", Format),
        ] {
            let proof = self.fields(raw, &[field], writer)?.finish();
            put(
                prepared,
                &format!("{base}/normalized/{path}"),
                proof,
                writer,
            )?;
        }
        Ok(())
    }

    pub(super) fn fields(
        &self,
        raw: &DdsRawHeader,
        fields: &[DdsHeaderField],
        writer: &MediaWriter<'_>,
    ) -> Result<OriginCapture, MediaError> {
        let mut capture = writer.capture_origins();
        self.add_prefix(raw, &mut capture, writer)?;
        for field in fields {
            for word in raw.words_for(*field) {
                self.add_bytes(
                    &mut capture,
                    word.offset,
                    word.length,
                    word.name,
                    SourceUseRole::Conversion,
                    writer,
                )?;
            }
        }
        Ok(capture)
    }

    fn add_prefix(
        &self,
        raw: &DdsRawHeader,
        capture: &mut OriginCapture,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for word in raw.words_for(Prefix) {
            self.add_bytes(
                capture,
                word.offset,
                word.length,
                word.name,
                SourceUseRole::Selector,
                writer,
            )?;
        }
        Ok(())
    }

    fn add_bytes(
        &self,
        capture: &mut OriginCapture,
        offset: u64,
        length: u64,
        field: &str,
        role: SourceUseRole,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        capture.add(
            self.input,
            InputScope::Payload,
            OriginSite::new("dds-header-bytes", &[offset, length], field),
            role,
            writer,
        )
    }
}

pub(super) fn put(
    prepared: &mut PreparedRecordOrigins,
    path: &str,
    evidence: OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let path = FieldPath::parse(path).map_err(|error| origin_error::address(&error))?;
    prepared.insert_subtree(&path, evidence, writer)
}
