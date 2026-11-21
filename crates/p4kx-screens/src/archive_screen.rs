// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_dataforge::{DataForge, DataForgeError};
use p4kx_extraction::{
    Archive, ArchiveImages, ArchiveWriteProtection, CancelToken, DataForgeEntry, FileReceipt,
    Language, OriginOutputAddress, ScreenTexts, SourceEvidenceError, decode_line_breaks,
    write_origin_companion,
};
use p4kx_gfx::Localise;
use thiserror::Error;

use crate::archive_screen_images::ImageReads;
use crate::archive_screen_origin::{ScreenRenderOrigin, ScreenSource};
use crate::compose::TextSources;
use crate::error::ScreenError;
use crate::font_file::ScreenFontFile;
use crate::images::ImageSources;
use crate::still::{Still, StillRequest};

/// An Archive and its DataForge input for one or more screen renders.
pub struct ArchiveScreen<'a> {
    archive: &'a Archive,
    source: &'a DataForgeEntry,
    forge: DataForge<'a>,
    font: ScreenFontFile,
}

/// A screen rendered from the Archive and the original inputs used by its output.
#[derive(Debug)]
pub struct ArchiveScreenRender {
    still: Still,
    origin: ScreenRenderOrigin,
}

/// Why a screen could not be read, drawn or published.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ArchiveScreenError {
    /// The DataForge payload could not be opened.
    #[error(transparent)]
    DataForge(#[from] DataForgeError),
    /// The screen could not be composed or encoded.
    #[error(transparent)]
    Draw(#[from] ScreenError),
    /// The original input inventory could not be built.
    #[error(transparent)]
    Evidence(#[from] SourceEvidenceError),
    /// The artifact or its origin companion could not be published.
    #[error(transparent)]
    Publish(#[from] io::Error),
}

impl<'a> ArchiveScreen<'a> {
    /// Opens the supplied DataForge payload for use with the Archive’s screen assets.
    ///
    /// # Errors
    ///
    /// Returns a DataForge format error.
    pub fn new(
        archive: &'a Archive,
        source: &'a DataForgeEntry,
    ) -> Result<Self, ArchiveScreenError> {
        let forge = source.open()?;
        let font = ScreenFontFile::read(&forge);
        Ok(Self {
            archive,
            source,
            forge,
            font,
        })
    }

    /// Loads the selected font movie and optional language from the same Archive.
    #[must_use]
    pub fn load_texts(&self, language: Option<Language>) -> ScreenTexts {
        ScreenTexts::load_observed(self.archive, &self.font.path, language)
    }

    /// Renders the screen and registers the original sources used by its output.
    /// External fonts retain their own evidence; the authored selector requires a matching load.
    ///
    /// # Errors
    ///
    /// Returns composition, cancellation or source-evidence errors.
    pub fn render(
        &self,
        texts: &ScreenTexts,
        request: &StillRequest<'_>,
        cancel: &CancelToken,
        warnings: &mut Vec<String>,
    ) -> Result<ArchiveScreenRender, ArchiveScreenError> {
        warnings.extend_from_slice(texts.warnings());
        let ini = texts.parsed(warnings);
        let lookup = |key: &str| {
            ini.as_ref()?
                .resolve(key)
                .map(|text| decode_line_breaks(text).into_owned())
        };
        let images = ArchiveImages::new(self.archive);
        let reads = ImageReads::default();
        let read_texture = |path: &str, side: u32, pixels: u64| {
            reads
                .texture(&images, path, side, pixels)
                .map_err(|error| describe(&error))
        };
        let read_vector = |path: &str| {
            reads
                .vector(&images, path)
                .map_err(|error| describe(&error))
        };
        let text_sources = texts.fonts().map(|fonts| TextSources {
            fonts,
            localise: ini.as_ref().map(|_| &lookup as Localise<'_>),
        });
        let image_sources = ImageSources {
            texture: &read_texture,
            vector: &read_vector,
        };
        let drawn = Still::render_observed(
            &self.forge,
            text_sources,
            Some(image_sources),
            request,
            &|| cancel.is_cancelled(),
        )?;
        let origin = ScreenRenderOrigin::register(
            &ScreenSource {
                dataforge: self.source,
                font: self
                    .font
                    .record_index
                    .filter(|_| texts.uses_font_selection(self.archive, &self.font.path)),
                texts,
                ini: ini.as_ref(),
            },
            &drawn,
            &reads,
            request.raster.size,
        )?;
        Ok(ArchiveScreenRender {
            still: drawn.still,
            origin,
        })
    }
}

impl ArchiveScreenRender {
    /// Returns the rendered image or document and its composition report.
    #[must_use]
    pub const fn still(&self) -> &Still {
        &self.still
    }

    /// Returns the original input evidence for this render.
    #[must_use]
    pub const fn origin(&self) -> &ScreenRenderOrigin {
        &self.origin
    }

    /// Publishes the image or document with its byte-bound sibling receipt.
    ///
    /// # Errors
    ///
    /// Returns an encoding, output protection or publication error.
    pub fn publish(
        &self,
        output: &Path,
        protection: &ArchiveWriteProtection,
        cancel: &CancelToken,
    ) -> Result<PathBuf, ArchiveScreenError> {
        let bytes = self.still.encode()?;
        let receipt = FileReceipt::new_with_cancel(
            output,
            Some(protection),
            Some(protection),
            cancel.clone(),
        )?;
        receipt.publish(
            |writer| writer.write_all(&bytes),
            |writer, binding, ()| {
                write_origin_companion(
                    writer,
                    binding,
                    &self.origin.provenance,
                    [(&OriginOutputAddress::OpaquePayload, &self.origin.artifact)],
                )
            },
        )?;
        Ok(receipt.path().to_path_buf())
    }
}

fn describe(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}
