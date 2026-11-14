// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_gltf::{GlbOptions, GlbSummary, GltfError, TextureLoader};

use crate::model_observed::{Observed, is_cancelled};
use crate::model_source::{ArchiveDefaultMaterial, ModelSource};
use crate::model_textures::ArchiveTextures;
use crate::output_file::{write_error, write_replacing_protected};
use crate::path_index::PathIndex;
use crate::texture_parts::is_same_file;
use crate::{AnimationSet, CancelToken, ConversionError};

impl ModelSource {
    /// Writes the model as a GLB file to `out`, replacing it; the file is written under a
    /// temporary name first.
    ///
    /// # Errors
    ///
    /// Returns [`ConversionError::OutputIsInput`] when `out` is one of the model's files or
    /// the Archive it was read from,
    /// [`ConversionError::Glb`] when the model cannot be exported with `options`, and
    /// [`ConversionError::Write`] when the file cannot be written.
    pub fn write_glb(
        &self,
        options: &GlbOptions,
        out: &Path,
    ) -> Result<GlbSummary, ConversionError> {
        self.write_glb_animated(&AnimationSet::default(), options, out)
    }

    /// Writes the model with the `animations` as a GLB file to `out`, replacing it; see
    /// [`Self::write_glb`].
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb`], and [`ConversionError::OutputIsInput`] also when `out` is a file
    /// an animation was read from.
    pub fn write_glb_animated(
        &self,
        animations: &AnimationSet,
        options: &GlbOptions,
        out: &Path,
    ) -> Result<GlbSummary, ConversionError> {
        self.write_glb_observed(
            animations,
            options,
            out,
            None,
            &CancelToken::new(),
            &|_, _| {},
        )
    }

    /// Writes the model as a GLB file to `out` like [`Self::write_glb`], taking the textures of
    /// [`p4kx_gltf::MaterialMode::Textures`] from the Entries of the Archive `index` is over;
    /// they are found there ignoring case and the kind of separator, and a `.tif` or `.tga` name
    /// in the material file is the `.dds` Entry.
    ///
    /// A texture that is missing, not an image, too large or undecodable leaves the material its
    /// plain colour and is listed in [`GlbSummary::texture_fallbacks`], never an error.
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb`].
    pub fn write_glb_textured(
        &self,
        index: &PathIndex<'_>,
        options: &GlbOptions,
        out: &Path,
    ) -> Result<GlbSummary, ConversionError> {
        self.write_glb_animated_textured(index, &AnimationSet::default(), options, out)
    }

    /// Writes the model with the `animations` and the textures of the Archive `index` is over as
    /// a GLB file to `out`; see [`Self::write_glb_animated`] and [`Self::write_glb_textured`].
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb_animated`].
    pub fn write_glb_animated_textured(
        &self,
        index: &PathIndex<'_>,
        animations: &AnimationSet,
        options: &GlbOptions,
        out: &Path,
    ) -> Result<GlbSummary, ConversionError> {
        self.write_glb_observed(
            animations,
            options,
            out,
            Some(&ArchiveTextures::new(index)),
            &CancelToken::new(),
            &|_, _| {},
        )
    }

    /// Writes the model as [`Self::write_glb`] does, taking the textures from `textures` when it
    /// is given, calling `progress` with the bytes written and the length of the file as it goes,
    /// and stopping once `cancel` is set.
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb`], and [`ConversionError::Cancelled`] when `cancel` was set; nothing is
    /// left at `out` then.
    pub fn write_glb_observed(
        &self,
        animations: &AnimationSet,
        options: &GlbOptions,
        out: &Path,
        textures: Option<&dyn TextureLoader>,
        cancel: &CancelToken,
        progress: &dyn Fn(u64, u64),
    ) -> Result<GlbSummary, ConversionError> {
        if is_same_file(out, self.inputs().iter().chain(&animations.inputs).cloned()) {
            return Err(ConversionError::OutputIsInput {
                path: out.to_path_buf(),
            });
        }
        if cancel.is_cancelled() {
            return Err(ConversionError::Cancelled);
        }
        write_replacing_protected(out, self.protection.context()?, Some(cancel), |writer| {
            let mut observed = Observed::new(writer, cancel, progress);
            let fallback = self
                .default_material()
                .and_then(ArchiveDefaultMaterial::material);
            let written = p4kx_gltf::write_glb_animated_with_fallback(
                self.model(),
                &animations.animations,
                options,
                textures,
                fallback,
                &mut observed,
            );
            written.map_err(|error| match error {
                GltfError::Write(source) if is_cancelled(&source) => ConversionError::Cancelled,
                GltfError::Write(source) => write_error(out, source),
                source => ConversionError::Glb {
                    file: self.name().to_owned(),
                    source,
                },
            })
        })
    }
}
