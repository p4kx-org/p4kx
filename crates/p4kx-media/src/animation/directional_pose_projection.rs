// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;

use super::super::super::directional_pose_blocks::{
    DirectionalPoseBlock, DirectionalPoseBlocks, PoseArray,
};
use super::super::super::directional_records::DirectionalRecord;
use super::super::super::image_payload::{
    ImageDirectionalPoseArray, ImageDirectionalPoseBlock, ImageDirectionalPoseIndex,
};
use super::super::super::origin as proof;
use super::super::super::payload::{self as wire, AnimationPayload};
use super::ImageSource;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Status, Subresource};

impl ImageSource<'_, '_> {
    pub(super) fn write_directional_poses(
        &self,
        indices: [u64; 2],
        original: &DirectionalRecord<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(blocks) = DirectionalPoseBlocks::new(original) else {
            return Ok(());
        };
        let budget = &self.context.directory.input_state()?.budget;
        if !blocks.visit(|_, _| budget.metadata(0, 1, writer.limits(), writer.checkpoint))? {
            return Ok(());
        }
        self.write_pose_index(indices, blocks, writer)?;
        let proved = blocks.visit(|ordinal, block| {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            self.write_pose_block(indices, ordinal, block, blocks.header().start, writer)
        })?;
        if !proved {
            return Err(MediaError::Contract(
                "validated directional pose framing became incomplete".into(),
            ));
        }
        Ok(())
    }

    fn write_pose_index(
        &self,
        indices: [u64; 2],
        blocks: DirectionalPoseBlocks<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.origins.prepared()?;
        let header = blocks.header();
        let extensions = blocks.extensions();
        let fixed = blocks.fixed_ranges();
        let framing = self.origins.payload(
            header.clone(),
            "directionalPoseHeader",
            SourceUseRole::Selector,
            writer,
        )?;
        proof::control(&mut prepared, framing, writer)?;
        for (field, range) in [
            ("header", header.clone()),
            ("first_orientation", fixed.first_orientation.clone()),
            ("second_orientation", fixed.second_orientation.clone()),
            ("polar_grid", fixed.polar_grid.clone()),
            ("pose_count", header.start..header.start + 4),
            ("extension_count", header.start + 4..header.start + 8),
            ("header_word", header.start + 8..header.end),
            ("extensions", header.start..extensions.end),
        ] {
            self.pose_origin(&mut prepared, field, range, writer)?;
        }
        for field in [
            "header/basis",
            "extensions/basis",
            "first_orientation/basis",
            "second_orientation/basis",
            "polar_grid/basis",
        ] {
            proof::at(
                &mut prepared,
                &format!("/data/payload/{field}"),
                self.origins.independent(),
                writer,
            )?;
        }
        prepared.write(
            writer,
            &Subresource::new(wire::IMAGE_DIRECTIONAL_POSE_INDEX, indices.to_vec()),
            Status::Complete,
            &AnimationPayload::ImageDirectionalPoseIndex(ImageDirectionalPoseIndex {
                first_orientation: wire::entry_range(fixed.first_orientation),
                second_orientation: wire::entry_range(fixed.second_orientation),
                polar_grid: wire::entry_range(fixed.polar_grid),
                header: wire::entry_range(header),
                pose_count: blocks.pose_count,
                extension_count: blocks.extension_count,
                header_word: blocks.header_word,
                extensions: wire::entry_range(extensions),
            }),
            &[],
        )
    }

    fn write_pose_block(
        &self,
        [chunk, record]: [u64; 2],
        ordinal: u32,
        block: DirectionalPoseBlock,
        start: u64,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = self.origins.prepared()?;
        let order = self.origins.payload(
            start..block.entries3.count_range.end,
            "directionalPoseBlockOrder",
            SourceUseRole::Selector,
            writer,
        )?;
        proof::control(&mut prepared, order, writer)?;
        for (field, array) in [("entries4", &block.entries4), ("entries3", &block.entries3)] {
            self.pose_origin(
                &mut prepared,
                &format!("{field}/count"),
                array.count_range.clone(),
                writer,
            )?;
            self.pose_origin(
                &mut prepared,
                &format!("{field}/count_range"),
                array.count_range.clone(),
                writer,
            )?;
            self.pose_origin(
                &mut prepared,
                &format!("{field}/values"),
                array.count_range.start..array.values.end,
                writer,
            )?;
            for name in ["count_range", "values"] {
                proof::at(
                    &mut prepared,
                    &format!("/data/payload/{field}/{name}/basis"),
                    self.origins.independent(),
                    writer,
                )?;
            }
        }
        prepared.write(
            writer,
            &Subresource::new(
                wire::IMAGE_DIRECTIONAL_POSE_BLOCK,
                vec![chunk, record, u64::from(ordinal)],
            ),
            Status::Complete,
            &AnimationPayload::ImageDirectionalPoseBlock(ImageDirectionalPoseBlock {
                entries4: pose_array(block.entries4),
                entries3: pose_array(block.entries3),
            }),
            &[],
        )
    }

    fn pose_origin(
        &self,
        prepared: &mut PreparedRecordOrigins,
        member: &str,
        range: Range<u64>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let origin = self
            .origins
            .payload(range, member, SourceUseRole::Payload, writer)?;
        proof::at(prepared, &format!("/data/payload/{member}"), origin, writer)
    }
}

const fn pose_array(array: PoseArray) -> ImageDirectionalPoseArray {
    ImageDirectionalPoseArray {
        count: array.count,
        count_range: wire::entry_range(array.count_range),
        values: wire::entry_range(array.values),
    }
}
