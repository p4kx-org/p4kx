// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::Duration;

use p4kx_gltf::MaterialMode;

use super::ModelCase;
use super::measurement::Measured;
use crate::media_fixtures::{self, Check, SourceCheck};

const MAX_SKIPPED_SHARE_PERCENT: usize = 10;
const MAX_BOX_SHARE_PERCENT: usize = 15;
const MAX_ORIGIN_SHARE_PERCENT: usize = 60;
const MIN_NEAR_HULL_SHARE_PERCENT: usize = 85;
const MIN_PLACED_FOR_SPATIAL_CHECK: usize = 10;
const MAX_TEXTURE_PIXELS: u64 = 400 * 1024 * 1024;
const MAX_GLB_BYTES: u64 = 1 << 30;
const MAX_BUILD: Duration = Duration::from_secs(600);

pub(super) fn verify(
    case: &ModelCase,
    mode: MaterialMode,
    measured: &Measured,
) -> media_fixtures::Result<()> {
    media_fixtures::require(measured.parts >= case.least_parts, Check::MinimumParts)?;
    media_fixtures::require(measured.modelled > 0, SourceCheck::ModelPresence)?;
    media_fixtures::require(measured.misparented == 0, Check::ParentTransform)?;
    media_fixtures::require(
        measured.skipped * 100 <= (measured.parts + measured.skipped) * MAX_SKIPPED_SHARE_PERCENT,
        Check::SkippedShare,
    )?;
    media_fixtures::require(
        measured.boxes * 100 <= measured.placed_children.max(1) * MAX_BOX_SHARE_PERCENT,
        Check::BoxShare,
    )?;
    placement(measured)?;
    if case.shares_meshes {
        media_fixtures::require(measured.summary.shared_meshes > 0, Check::MeshSharing)?;
    }
    media_fixtures::require(measured.summary.bytes <= MAX_GLB_BYTES, Check::OutputBytes)?;
    media_fixtures::require(
        measured.summary.texture_pixels <= MAX_TEXTURE_PIXELS,
        Check::TexturePixels,
    )?;
    media_fixtures::require(
        measured.built + measured.written <= MAX_BUILD,
        Check::Duration,
    )?;
    if mode == MaterialMode::Textures {
        media_fixtures::require(measured.summary.textures > 0, Check::Images)?;
        media_fixtures::require(
            measured.summary.texture_fallbacks.len() * 2
                <= measured.summary.textures + measured.summary.texture_fallbacks.len(),
            Check::TextureFallbacks,
        )?;
    }
    Ok(())
}

fn placement(measured: &Measured) -> media_fixtures::Result<()> {
    if measured.placed_children >= MIN_PLACED_FOR_SPATIAL_CHECK {
        media_fixtures::require(
            measured.origin * 100 <= measured.placed_children * MAX_ORIGIN_SHARE_PERCENT,
            Check::OriginShare,
        )?;
        media_fixtures::require(
            measured.near_hull * 100 >= measured.placed_children * MIN_NEAR_HULL_SHARE_PERCENT,
            Check::HullShare,
        )?;
    }
    Ok(())
}
