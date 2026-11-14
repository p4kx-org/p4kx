// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::texture_preview::ObservedDdnaDds;

use super::{Outcome, TextureOutput};
use crate::texture_ddna::{self, SmoothnessWritten};
use crate::texture_source_metadata::metadata_error;
use crate::{
    AlphaMeaning, ColourSpace, DdnaTexturePair, DdsTextureAsset, DdsTextureFallback, PackageResult,
    TextureEncoding, TextureFormat,
};

impl TextureOutput<'_, '_> {
    pub(super) fn write_ddna_pair(
        &self,
        observed: ObservedDdnaDds<'_>,
    ) -> PackageResult<Result<Outcome, DdsTextureFallback>> {
        if let Some(reason) = texture_ddna::unsupported(&observed.normal.read) {
            return Ok(Err(reason));
        }
        let paired = Self {
            texture: self.texture,
            id: self.smoothness_id,
            smoothness_id: self.smoothness_id,
            folder: self.folder,
            context: self.context,
        };
        let width = observed.normal.read.mip.width();
        let height = observed.normal.read.mip.height();
        let pairing = DdnaTexturePair::bind(&observed, self.smoothness_id.to_owned())?;
        let coverage = observed.normal.read.mip.alpha_coverage(0);
        let normal = self
            .write(TextureFormat::Dds, |writer| {
                observed.normal.read.write_dds(writer)
            })?
            .map_err(metadata_error)?;
        let smoothness = paired
            .write(TextureFormat::Dds, |writer| {
                observed.smoothness.read.write_dds(writer)
            })?
            .map_err(metadata_error)?;
        let dds_asset =
            DdsTextureAsset::bind_pair_member(observed.normal, normal.clone(), coverage)?;
        let smoothness = SmoothnessWritten {
            id: self.smoothness_id.to_owned(),
            source: format!("{}.a", self.texture.path),
            asset: DdsTextureAsset::bind_pair_member(observed.smoothness, smoothness, None)?,
            pairing,
        };
        Ok(Ok(Outcome::Written {
            width,
            height,
            artifact: normal,
            colour_space: ColourSpace::Linear,
            encoding: TextureEncoding::Data,
            alpha: AlphaMeaning::None,
            source_metadata: None,
            format: TextureFormat::Dds,
            dds_asset: Some(Box::new(dds_asset)),
            dds_fallback: None,
            smoothness: Some(Box::new(smoothness)),
        }))
    }
}
