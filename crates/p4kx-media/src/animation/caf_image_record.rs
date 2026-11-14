// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::CafImageRecord;

use super::super::image_payload::ImageRecord;
use super::super::origin as proof;
use super::super::payload::{self as wire, AnimationPayload};
use super::ImageSource;
use super::caf_image_clip::DbaMetadataCache;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource};

impl ImageSource<'_, '_> {
    pub(super) fn write_record(
        &self,
        indices: [u64; 2],
        record: CafImageRecord<'_>,
        metadata: &mut DbaMetadataCache,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        self.context.directory.input_state()?.budget.metadata(
            0,
            1,
            writer.limits(),
            writer.checkpoint,
        )?;
        let mut prepared = self.origins.prepared()?;
        let record_range = record.range();
        let field_range = record.path_field_range();
        let body = self.origins.payload(
            record_range.clone(),
            "cafImageRecord",
            SourceUseRole::Payload,
            writer,
        )?;
        let path = self.path_field_proof(&mut prepared, field_range.clone(), writer)?;
        proof::control(&mut prepared, body.clone(), writer)?;
        proof::at(&mut prepared, "/data/payload/range", body, writer)?;
        proof::at(
            &mut prepared,
            "/data/payload/range/basis",
            self.origins.independent(),
            writer,
        )?;
        let hashes = self.hashes(record, &mut prepared, writer)?;
        let dba = self.dba_binding(
            hashes.dba,
            record.path(),
            &path,
            metadata,
            &mut prepared,
            writer,
        )?;
        let path = self.path(record.path(), "caf", &path, &mut prepared, writer)?;
        prepared.write(
            writer,
            &Subresource::new(wire::IMAGE_RECORD, indices.to_vec()),
            Status::Complete,
            &AnimationPayload::ImageRecord(ImageRecord {
                range: wire::entry_range(record_range),
                path_field: wire::entry_range(field_range),
                caf_path_crc32: hashes.caf.value,
                dba_path_crc32: (hashes.dba.value != 0).then_some(hashes.dba.value),
                dba_entry: dba.entry,
                dba_clip: dba.clip,
                path,
            }),
            &[],
        )
    }

    fn path_field_proof(
        &self,
        prepared: &mut PreparedRecordOrigins,
        field_range: Range<u64>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let path = self.origins.payload(
            field_range,
            "cafImagePathField",
            SourceUseRole::Payload,
            writer,
        )?;
        proof::at(prepared, "/data/payload/path_field", path.clone(), writer)?;
        proof::at(prepared, "/data/payload/path", path.clone(), writer)?;
        proof::at(
            prepared,
            "/data/payload/path_field/basis",
            self.origins.independent(),
            writer,
        )?;
        Ok(path)
    }
}
