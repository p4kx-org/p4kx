// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_gfx::{GfxError, RasterOptions};

use super::ui_render_frame_evidence::BackendInputs;
use super::ui_render_summary::RenderSummaryOrigins;
use crate::output_file::write_error;
use crate::{
    ConversionError, FieldOrigin, FileReceipt, OriginOutputAddress, SourceInventory, UiMovie,
    UiRender, UiSvg, write_origin_companion,
};

/// A committed UI render, its byte-bound receipt, and source evidence for its summary.
#[derive(Debug)]
#[non_exhaustive]
pub struct PublishedUiRender<T> {
    /// The ordinary render result.
    pub render: T,
    /// The sibling companion that binds the committed artifact.
    pub origin_receipt: PathBuf,
    /// Actual contributors to the encoded artifact.
    pub artifact_origin: FieldOrigin,
    /// Direct source evidence for summary values.
    pub summary_origins: RenderSummaryOrigins,
    /// Original physical input and use inventory for the artifact and its summary.
    pub provenance: SourceInventory,
}

struct Prepared<T> {
    render: T,
    artifact: FieldOrigin,
    summary: RenderSummaryOrigins,
    inventory: SourceInventory,
}

impl UiMovie<'_> {
    /// Publishes a first-frame PNG and its byte-bound origin companion.
    ///
    /// # Errors
    ///
    /// Returns render, output protection, or publication errors.
    pub fn render_to_file_with_origin(
        &self,
        options: &RasterOptions,
        out: &Path,
    ) -> Result<PublishedUiRender<UiRender>, ConversionError> {
        self.check_output(out)?;
        let observed = self.render_with_bitmap_sources(options)?;
        self.publish_render(out, |writer| {
            observed
                .render
                .image
                .write_png(writer)
                .map_err(|error| self.render_io(error))?;
            Ok(Prepared {
                render: observed.render,
                artifact: observed.artifact_origin,
                summary: observed.summary,
                inventory: observed.provenance,
            })
        })
    }

    /// Publishes a first-frame SVG and its byte-bound origin companion.
    ///
    /// # Errors
    ///
    /// Returns render, output protection, or publication errors.
    pub fn render_svg_to_file_with_origin(
        &self,
        options: &RasterOptions,
        out: &Path,
    ) -> Result<PublishedUiRender<UiSvg>, ConversionError> {
        self.check_output(out)?;
        let observed = self.movie().assets_observed();
        let frame = self.compose_frame_with_assets(options, observed.assets, true)?;
        self.publish_render(out, |writer| {
            let observed_svg = frame
                .scene
                .write_svg_observed(&frame.bitmaps.images, options, Some(&self.title()), writer)
                .map_err(|error| self.render_io(error))?;
            let summary = self.register_frame_evidence(
                &observed.origins,
                &frame,
                &BackendInputs {
                    items: &observed_svg.item_inputs,
                    bitmaps: &observed_svg.bitmap_inputs,
                    masks: &observed_svg.mask_inputs,
                    title: true,
                    diagnostics_complete: observed_svg.svg.skipped.is_empty()
                        && frame.has_no_omissions(),
                },
                options,
            )?;
            Ok(Prepared {
                render: UiSvg {
                    width: observed_svg.svg.width,
                    height: observed_svg.svg.height,
                    skipped: observed_svg.svg.skipped,
                    bitmap_failures: frame.bitmaps.failures,
                    tag_failures: frame.tag_failures,
                    imports: frame.imports,
                },
                artifact: summary.artifact_origin,
                summary: summary.summary,
                inventory: summary.provenance,
            })
        })
    }

    fn publish_render<T>(
        &self,
        out: &Path,
        write: impl FnOnce(&mut dyn Write) -> io::Result<Prepared<T>>,
    ) -> Result<PublishedUiRender<T>, ConversionError> {
        let protection = self.write_protection()?;
        let receipt = FileReceipt::new(out, protection, protection)
            .map_err(|error| write_error(out, error))?;
        let prepared = receipt
            .publish(write, |writer, binding, result| {
                write_origin_companion(
                    writer,
                    binding,
                    &result.inventory,
                    [(&OriginOutputAddress::OpaquePayload, &result.artifact)],
                )
            })
            .map_err(|error| match error.downcast::<ConversionError>() {
                Ok(error) => error,
                Err(error) => write_error(out, error),
            })?;
        Ok(PublishedUiRender {
            render: prepared.render,
            origin_receipt: receipt.path().to_path_buf(),
            artifact_origin: prepared.artifact,
            summary_origins: prepared.summary,
            provenance: prepared.inventory,
        })
    }

    fn render_io(&self, error: GfxError) -> io::Error {
        match error {
            GfxError::Write(error) => error,
            error => io::Error::other(self.frame_error(error)),
        }
    }
}
