// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::DdsHeader;
use thiserror::Error;

use super::MAX_MODEL_TEXTURE_PIXELS;

#[derive(Debug, Clone, Copy)]
pub(super) struct ModelMipSelection {
    pub(super) level: u32,
    pub(super) size: (u32, u32),
    pub(super) cause: MipSelectionCause,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MipSelectionCause {
    Requested,
    Primary,
    Tail,
    Both,
}

#[derive(Debug, Error)]
pub(super) enum ModelMipError {
    #[error("the texture is a cubemap, array or volume, not one image")]
    Topology,
    #[error("the alpha tail is {}x{} at this mip, the texture {}x{}", tail.0, tail.1, primary.0, primary.1)]
    MismatchedDimensions {
        primary: (u32, u32),
        tail: (u32, u32),
    },
    #[error(
        "the texture has {0} pixels at this mip, more than the limit of {MAX_MODEL_TEXTURE_PIXELS}; use a higher --texture-mip"
    )]
    TooLarge(u64),
}

pub(super) fn select(
    primary: &DdsHeader,
    tail: Option<&DdsHeader>,
    requested: u32,
) -> Result<ModelMipSelection, ModelMipError> {
    if primary.surfaces() != 1 || primary.depth() != 1 {
        return Err(ModelMipError::Topology);
    }
    let primary_last = primary.mip_count() - 1;
    let tail_last = tail.map(|header| header.mip_count() - 1);
    let last = tail_last.map_or(primary_last, |tail| primary_last.min(tail));
    let level = requested.min(last);
    let size = dimensions(primary, level);
    if let Some(tail) = tail {
        let tail = dimensions(tail, level);
        if tail != size {
            return Err(ModelMipError::MismatchedDimensions {
                primary: size,
                tail,
            });
        }
    }
    let pixels = u64::from(size.0) * u64::from(size.1);
    if pixels > MAX_MODEL_TEXTURE_PIXELS {
        return Err(ModelMipError::TooLarge(pixels));
    }
    let cause = if level == requested {
        MipSelectionCause::Requested
    } else {
        match tail_last {
            Some(tail) if tail < primary_last => MipSelectionCause::Tail,
            Some(tail) if tail == primary_last => MipSelectionCause::Both,
            _ => MipSelectionCause::Primary,
        }
    };
    Ok(ModelMipSelection { level, size, cause })
}

fn dimensions(header: &DdsHeader, level: u32) -> (u32, u32) {
    let side = |full: u32| (full >> level).max(1);
    (side(header.width()), side(header.height()))
}
