// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{SocSourceValue, Transform};
use thiserror::Error;

use crate::{SocTilePortSite, SocTilePortTarget};

/// Authored and port-anchored light-group frames in one container occurrence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SocTileLightAnchor {
    authored: Transform,
    anchored: Transform,
}

impl SocTileLightAnchor {
    /// Anchors the group origin at the port without changing the group's basis.
    ///
    /// # Errors
    /// Returns the reason a unique port offset or a usable transform is unavailable.
    pub fn resolve(
        target: SocTilePortTarget<'_, '_>,
        owner_from_container: Option<Transform>,
        container_from_group: Option<Transform>,
    ) -> Result<Self, SocTileLightAnchorError> {
        let offset = owner_offset(target)?;
        let container_from_owner = owner_from_container
            .ok_or(SocTileLightAnchorError::MissingContainerFrame)?
            .inverse()
            .ok_or(SocTileLightAnchorError::InvalidContainerFrame)?;
        let authored = container_from_group
            .ok_or(SocTileLightAnchorError::MissingGroupFrame)?
            .finite()
            .ok_or(SocTileLightAnchorError::InvalidGroupFrame)?;
        let point = container_from_owner.rows.map(|row| {
            let [x, y, z, shift] = row.map(f64::from);
            let [px, py, pz] = offset;
            x.mul_add(px, y.mul_add(py, z.mul_add(pz, shift)))
        });
        let mut rows = authored.rows.map(|row| row.map(f64::from));
        for ([_, _, _, translation], anchor) in rows.iter_mut().zip(point) {
            *translation = anchor;
        }
        let anchored =
            Transform::from_rows_f64(rows).ok_or(SocTileLightAnchorError::UnrepresentableAnchor)?;
        Ok(Self { authored, anchored })
    }

    /// Returns the original group frame in the container.
    #[must_use]
    pub const fn authored(&self) -> Transform {
        self.authored
    }

    /// Returns the anchored group frame in the container.
    #[must_use]
    pub const fn anchored(&self) -> Transform {
        self.anchored
    }
}

/// Why a tile-attached light group could not be anchored.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum SocTileLightAnchorError {
    /// The group does not declare a port selector.
    #[error("tile attachment has no port selector")]
    AbsentSelector,
    /// The port selector cannot be interpreted.
    #[error("tile attachment port selector is invalid")]
    InvalidSelector,
    /// No port matches the selector in this container.
    #[error("tile attachment port selector is unmatched")]
    UnmatchedSelector,
    /// Multiple original ports match the selector.
    #[error("tile attachment port selector is ambiguous")]
    AmbiguousSelector,
    /// The selected port has no authored interaction offset.
    #[error("selected tile port has no interaction offset")]
    AbsentOffset,
    /// The selected port's offset is malformed or repeated.
    #[error("selected tile port interaction offset is invalid")]
    InvalidOffset,
    /// The container has no usable placement declaration.
    #[error("container placement frame is missing")]
    MissingContainerFrame,
    /// The container's placement cannot be inverted.
    #[error("container placement frame cannot be inverted")]
    InvalidContainerFrame,
    /// The light group has no usable frame declaration.
    #[error("light group frame is missing")]
    MissingGroupFrame,
    /// The light group's frame is not finite.
    #[error("light group frame is invalid")]
    InvalidGroupFrame,
    /// The corrected frame cannot be represented in the scene.
    #[error("anchored light group frame is not representable")]
    UnrepresentableAnchor,
}

const fn owner_offset(
    target: SocTilePortTarget<'_, '_>,
) -> Result<[f64; 3], SocTileLightAnchorError> {
    use SocTileLightAnchorError as Error;
    let site: &SocTilePortSite<'_> = match target {
        SocTilePortTarget::Absent => return Err(Error::AbsentSelector),
        SocTilePortTarget::Invalid => return Err(Error::InvalidSelector),
        SocTilePortTarget::Unmatched => return Err(Error::UnmatchedSelector),
        SocTilePortTarget::Ambiguous(_) => return Err(Error::AmbiguousSelector),
        SocTilePortTarget::Matched(site) => site,
    };
    match site.offset {
        SocSourceValue::Present(offset) => Ok(offset),
        SocSourceValue::Absent => Err(Error::AbsentOffset),
        _ => Err(Error::InvalidOffset),
    }
}
