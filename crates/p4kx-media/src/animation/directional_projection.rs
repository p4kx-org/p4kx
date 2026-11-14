// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::CafImagePath;

use super::super::directional_paths::{DirectionalPath, DirectionalPaths};
use super::super::directional_records::{DirectionalRecord, DirectionalRecords};
use super::super::image_payload::{ImageDirectionalPath, ImageDirectionalRecord};
use super::super::origin as proof;
use super::super::payload::{self as wire, AnimationPayload};
use super::{ImageSource, Layout};
use crate::{MediaError, MediaWriter, Status, Subresource};

impl ImageSource<'_, '_> {
    pub(super) fn directional<'a>(
        &self,
        bytes: &'a [u8],
        start: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Layout<'a>, MediaError> {
        if let Some(records) = DirectionalRecords::new(bytes, start) {
            let budget = &self.context.directory.input_state()?.budget;
            if records.visit(|_, _| budget.metadata(0, 1, writer.limits(), writer.checkpoint))? {
                return Ok(Layout::DirectionalRecords(records));
            }
        }
        let Some(paths) = DirectionalPaths::new(bytes, start) else {
            return Ok(Layout::Directional);
        };
        let Some(count) = paths.declared() else {
            return Ok(Layout::Directional);
        };
        if count == 0 || !paths.begins_with_path() {
            return Ok(Layout::Directional);
        }
        let mut observed = 0;
        paths.visit(self.context, writer, |_, _| {
            observed += 1;
            Ok(())
        })?;
        if observed == count {
            Ok(Layout::DirectionalPaths { paths, count })
        } else {
            Ok(Layout::Directional)
        }
    }

    pub(super) fn write_directional(
        &self,
        indices: [u64; 2],
        coverage_start: u64,
        path: DirectionalPath<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.origins.prepared()?;
        let proof = self.origins.payload(
            path.range.clone(),
            "directionalPathField",
            SourceUseRole::Payload,
            writer,
        )?;
        let order = self.origins.payload(
            coverage_start..path.range.end,
            "directionalPathOrder",
            SourceUseRole::Selector,
            writer,
        )?;
        proof::control(&mut prepared, order, writer)?;
        proof::at(
            &mut prepared,
            "/data/payload/path_field",
            proof.clone(),
            writer,
        )?;
        proof::at(
            &mut prepared,
            "/data/payload/path_field/basis",
            self.origins.independent(),
            writer,
        )?;
        proof::at(&mut prepared, "/data/payload/path", proof.clone(), writer)?;
        let linked = self.path(
            CafImagePath::Text(path.text),
            "aim",
            &proof,
            &mut prepared,
            writer,
        )?;
        prepared.write(
            writer,
            &Subresource::new(wire::IMAGE_DIRECTIONAL_PATH, indices.to_vec()),
            Status::Complete,
            &AnimationPayload::ImageDirectionalPath(ImageDirectionalPath {
                path_field: wire::entry_range(path.range),
                path: linked,
            }),
            &[],
        )
    }
    pub(super) fn write_directional_records(
        &self,
        chunk: usize,
        records: DirectionalRecords<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let budget = &self.context.directory.input_state()?.budget;
        let complete = records.visit(|ordinal, record| {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            let identity = [chunk as u64, ordinal as u64];
            self.write_directional_record(identity, &record, writer)?;
            if let Some(path) = record.path {
                self.write_directional(identity, records.header().start, path, writer)?;
            }
            Ok::<_, MediaError>(())
        })?;
        if !complete {
            return Err(MediaError::Contract(
                "validated directional framing became incomplete".into(),
            ));
        }
        Ok(())
    }

    fn write_directional_record(
        &self,
        indices: [u64; 2],
        record: &DirectionalRecord<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.origins.prepared()?;
        let words = record.range.start..record.path_field.start;
        let framing = self.origins.payload(
            words.clone(),
            "directionalRecordFraming",
            SourceUseRole::Selector,
            writer,
        )?;
        proof::control(&mut prepared, framing, writer)?;
        for (member, range) in [
            ("range", words.start..words.start + 4),
            ("byte_length", words.start..words.start + 4),
            ("header_word", words.start + 4..words.end),
            ("path_field", record.path_field.clone()),
            (
                "aim_path_crc32",
                record.path_field.end..record.opaque_tail.start,
            ),
            ("opaque_tail", words.start..words.start + 4),
        ] {
            let evidence = self
                .origins
                .payload(range, member, SourceUseRole::Payload, writer)?;
            proof::at(
                &mut prepared,
                &format!("/data/payload/{member}"),
                evidence,
                writer,
            )?;
        }
        for member in ["range", "path_field", "opaque_tail"] {
            proof::at(
                &mut prepared,
                &format!("/data/payload/{member}/basis"),
                self.origins.independent(),
                writer,
            )?;
        }
        prepared.write(
            writer,
            &Subresource::new(wire::IMAGE_DIRECTIONAL_RECORD, indices.to_vec()),
            Status::Complete,
            &AnimationPayload::ImageDirectionalRecord(ImageDirectionalRecord {
                range: wire::entry_range(record.range.clone()),
                byte_length: record.byte_length,
                header_word: record.header_word,
                path_field: wire::entry_range(record.path_field.clone()),
                aim_path_crc32: record.aim_path_crc32,
                opaque_tail: wire::entry_range(record.opaque_tail.clone()),
            }),
            &[],
        )
    }
}
