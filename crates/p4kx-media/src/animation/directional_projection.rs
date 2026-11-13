// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::model::CafImagePath;

use super::super::directional_paths::DirectionalPath;
use super::super::image_payload::ImageDirectionalPath;
use super::super::origin as proof;
use super::super::payload::{self as wire, AnimationPayload};
use super::ImageSource;
use crate::{MediaError, MediaWriter, Status, Subresource};

impl ImageSource<'_, '_> {
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
}
