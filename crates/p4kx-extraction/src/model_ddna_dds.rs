// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::texture_suffix;
use p4kx_texture::{DdsAlphaAssociation, DdsHeaderField, DdsRawHeader, DdsTransfer, DxgiFormat};

use super::dds::{self, ModelDdsRefusal, ObservedModelDds, SelectedHeader, TextureMember};
use super::mip_selection::{self, MipSelectionCause};
use super::{ArchiveTextures, archive_path};
use crate::{FieldOrigin, SourceEvidenceError, SourceUseRole, SourceUses};

/// Why the common normal and smoothness level differs from the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DdnaMipReason {
    /// Both inputs contain the requested level.
    Requested,
    /// The primary texture has the shorter chain.
    PrimaryChain,
    /// The smoothness texture has the shorter chain.
    SmoothnessChain,
    /// Both inputs stop at the same level.
    BothChains,
}

impl From<MipSelectionCause> for DdnaMipReason {
    fn from(cause: MipSelectionCause) -> Self {
        match cause {
            MipSelectionCause::Requested => Self::Requested,
            MipSelectionCause::Primary => Self::PrimaryChain,
            MipSelectionCause::Tail => Self::SmoothnessChain,
            MipSelectionCause::Both => Self::BothChains,
        }
    }
}

/// Verified, independently addressable compressed images of a DDNA texture.
#[derive(Debug)]
#[non_exhaustive]
pub struct ObservedDdnaDds<'archive> {
    /// Primary normal payload. Its consumer can validate format and normal conventions.
    pub normal: ObservedModelDds<'archive>,
    /// Separate linear smoothness payload stored in BC4 red samples.
    pub smoothness: ObservedModelDds<'archive>,
    /// Original requested mip, with zero denoting the largest.
    pub requested_mip: u32,
    /// Reason the exact same level was selected for both assets.
    pub reason: DdnaMipReason,
}

impl ObservedDdnaDds<'_> {
    /// Registers the original header inputs used jointly for the shared mip decision.
    ///
    /// # Errors
    /// Returns an evidence registration error.
    pub fn selection_origin(
        &self,
        inventory: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        self.related_origin(inventory, false)
    }

    /// Registers the selected payloads and source decisions that make the pair publishable.
    ///
    /// # Errors
    /// Returns an evidence registration error.
    pub fn pairing_origin(
        &self,
        inventory: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        self.related_origin(inventory, true)
    }

    fn related_origin(
        &self,
        inventory: &SourceUses,
        paired_payloads: bool,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        use DdsHeaderField::{
            AlphaAssociation, ArraySize, CryTransfer, Depth, DxgiTransfer, Faces, Format, Height,
            MipCount, Prefix, Width,
        };

        let normal = self.normal.evidence(inventory)?;
        let smoothness = self.smoothness.evidence(inventory)?;
        let fields = [
            Prefix, Format, Width, Height, MipCount, ArraySize, Faces, Depth,
        ];
        let mut contributors = Vec::new();
        for part in [&normal.color, &smoothness.color] {
            contributors.extend(part.header_uses(&fields, SourceUseRole::Selector)?);
            contributors.push(part.name_use(SourceUseRole::Selector)?);
        }
        contributors.extend(
            normal
                .color
                .header_uses(&[AlphaAssociation], SourceUseRole::Selector)?,
        );
        contributors.extend(smoothness.color.header_uses(
            &[AlphaAssociation, DxgiTransfer, CryTransfer],
            SourceUseRole::Selector,
        )?);
        if paired_payloads {
            contributors.extend(
                normal
                    .color
                    .header_uses(&[DxgiTransfer, CryTransfer], SourceUseRole::Selector)?,
            );
            normal.append_pixel_uses(&mut contributors)?;
            smoothness.append_pixel_uses(&mut contributors)?;
        }
        Ok(inventory.origin(
            &contributors,
            normal.is_complete() && smoothness.is_complete(),
        ))
    }
}

impl<'a> ArchiveTextures<'_, 'a> {
    /// Reads the exact normal and linear BC4 smoothness members at their common PNG level.
    ///
    /// # Errors
    /// Returns a refusal for a non-DDNA role, unsupported tail, or unusable physical inputs.
    pub fn load_ddna_dds(
        &self,
        path: &str,
        mip: u32,
    ) -> Result<ObservedDdnaDds<'a>, ModelDdsRefusal> {
        let is_dds = path
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("dds"));
        if !is_dds || !texture_suffix(path).eq_ignore_ascii_case("ddna") {
            return Err(ModelDdsRefusal::NotDdna);
        }
        let path = archive_path(path);
        let normal = dds::unique(self.index, &path)?.ok_or(ModelDdsRefusal::Missing)?;
        let smoothness =
            dds::unique(self.index, &format!("{path}.a"))?.ok_or(ModelDdsRefusal::Missing)?;
        let normal = SelectedHeader::read(self.index, normal)?;
        let smoothness = SelectedHeader::read(self.index, smoothness)?;
        validate_association(normal.raw.alpha_association())?;
        validate_association(smoothness.raw.alpha_association())?;
        validate_smoothness(&smoothness.raw)?;
        if smoothness.normalized.surfaces() != 1 || smoothness.normalized.depth() != 1 {
            return Err(ModelDdsRefusal::Topology);
        }
        let selected =
            mip_selection::select(&normal.normalized, Some(&smoothness.normalized), mip)?;
        let normal = dds::read_selected(
            self.index,
            &path,
            TextureMember::Primary,
            &normal,
            selected.level,
        )?;
        let smoothness = dds::read_selected(
            self.index,
            &path,
            TextureMember::AlphaTail,
            &smoothness,
            selected.level,
        )?;
        Ok(ObservedDdnaDds {
            normal,
            smoothness,
            requested_mip: mip,
            reason: selected.cause.into(),
        })
    }
}

fn validate_smoothness(raw: &DdsRawHeader) -> Result<(), ModelDdsRefusal> {
    const TEXTURE_2D: u32 = 3;
    let transfer = raw.transfer();
    if raw.dx10().is_some_and(|ext| {
        DxgiFormat(ext.dxgi_format) == DxgiFormat::BC4_UNORM && ext.resource_dimension == TEXTURE_2D
    }) && transfer.effective == DdsTransfer::Linear
        && !transfer.conflict
    {
        Ok(())
    } else {
        Err(ModelDdsRefusal::UnsupportedSmoothness)
    }
}

const fn validate_association(association: DdsAlphaAssociation) -> Result<(), ModelDdsRefusal> {
    match association {
        DdsAlphaAssociation::Unknown
        | DdsAlphaAssociation::Straight
        | DdsAlphaAssociation::Opaque
        | DdsAlphaAssociation::Custom => Ok(()),
        _ => Err(ModelDdsRefusal::AlphaAssociation),
    }
}
