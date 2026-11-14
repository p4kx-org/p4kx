// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::Result;
use p4kx_extraction::texture_codec::{
    DdsAlphaAssociation, DdsAlphaTransform, DdsPngTransform, DdsTransfer,
};
use p4kx_extraction::texture_preview::PngOrigins;
use p4kx_extraction::{CubemapLayout, FieldOrigins, SourceInventory, Texture};
use serde::Serialize;

use crate::json;

/// The file format `p4kx texture png` wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum ImageFormat {
    Png,
    Exr,
}

#[derive(Serialize)]
struct ImageJson<'a> {
    texture: &'a str,
    output: String,
    format: ImageFormat,
    files: Vec<String>,
    mip: u32,
    width: u32,
    height: u32,
    alpha_tail: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_transfer: Option<DdsTransfer>,
    png_transform: Option<DdsPngTransform>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpha_association: Option<DdsAlphaAssociation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    alpha_transform: Option<DdsAlphaTransform>,
    #[serde(flatten)]
    origins: Option<ReportOrigins>,
}

#[derive(Serialize)]
struct ReportOrigins {
    origin_receipts: Vec<String>,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

/// What `p4kx texture png` exports: the mip, the cubemap layout and whether a high-range
/// texture is tone-mapped to an 8-bit PNG.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageOptions {
    pub(crate) mip: u32,
    pub(crate) layout: CubemapLayout,
    pub(crate) tone_map: bool,
}

struct Written {
    format: ImageFormat,
    level: u32,
    width: u32,
    height: u32,
    files: Vec<PathBuf>,
    alpha_tail: bool,
    output_transfer: Option<DdsTransfer>,
    png_transform: Option<DdsPngTransform>,
    alpha_association: Option<DdsAlphaAssociation>,
    alpha_transform: Option<DdsAlphaTransform>,
    origins: Option<PngOrigins>,
}

/// Runs `p4kx texture png`: an 8-bit PNG, or OpenEXR for a high-range texture without
/// `--tone-map`, next to `out` with the extension `.exr`.
pub(crate) fn export_image(
    texture: &Texture<'_>,
    out: &Path,
    options: ImageOptions,
    as_json: bool,
) -> Result<()> {
    let format = texture.header().format();
    let high_range = format.is_high_range();
    if options.tone_map && !high_range {
        writeln!(
            io::stderr().lock(),
            "note: --tone-map has no effect: {format} is not a high-range format"
        )?;
    }
    let written = write_image(texture, out, options, high_range)?;
    if as_json {
        json::print(&ImageJson {
            texture: texture.name(),
            output: json::path(out),
            format: written.format,
            files: written.files.iter().map(|file| json::path(file)).collect(),
            mip: written.level,
            width: written.width,
            height: written.height,
            alpha_tail: written.alpha_tail,
            output_transfer: written.output_transfer,
            png_transform: written.png_transform,
            alpha_association: written.alpha_association,
            alpha_transform: written.alpha_transform,
            origins: written.origins.map(|origins| ReportOrigins {
                origin_receipts: origins
                    .receipts
                    .iter()
                    .map(|path| {
                        path.file_name()
                            .unwrap_or(path.as_os_str())
                            .to_string_lossy()
                            .into_owned()
                    })
                    .collect(),
                field_origins: origins.fields,
                provenance: origins.inventory,
            }),
        })?;
        return Ok(());
    }
    writeln!(io::stdout().lock(), "{}", describe(texture, out, &written))?;
    Ok(())
}

fn write_image(
    texture: &Texture<'_>,
    out: &Path,
    options: ImageOptions,
    high_range: bool,
) -> Result<Written> {
    if high_range && !options.tone_map {
        let exr = out.with_extension("exr");
        let report = texture.exr_to_file_as(&exr, options.mip, options.layout)?;
        Ok(Written {
            format: ImageFormat::Exr,
            level: report.level,
            width: report.width,
            height: report.height,
            files: report.files,
            alpha_tail: false,
            output_transfer: None,
            png_transform: None,
            alpha_association: None,
            alpha_transform: None,
            origins: None,
        })
    } else {
        let (report, origins) =
            texture.png_to_file_with_origins(out, options.mip, options.layout)?;
        Ok(Written {
            format: ImageFormat::Png,
            level: report.level,
            width: report.width,
            height: report.height,
            files: report.files,
            alpha_tail: report.alpha_tail,
            output_transfer: Some(report.encoding.transfer),
            png_transform: Some(report.encoding.transform),
            alpha_association: Some(report.alpha_association),
            alpha_transform: (report.alpha_transform != DdsAlphaTransform::Preserve)
                .then_some(report.alpha_transform),
            origins: Some(origins),
        })
    }
}

fn describe(texture: &Texture<'_>, out: &Path, written: &Written) -> String {
    let format = texture.header().format();
    let files: Vec<String> = written
        .files
        .iter()
        .map(|file| file.display().to_string())
        .collect();
    let tail = if written.alpha_tail {
        " with its alpha tail"
    } else {
        ""
    };
    let start = format!(
        "wrote mip {} of {}{tail}, {} x {}",
        written.level,
        texture.name(),
        written.width,
        written.height
    );
    let files = files.join(", ");
    match written.format {
        ImageFormat::Exr if written.files.iter().all(|file| file != out) => format!(
            "{start}, as OpenEXR to {files} instead of {}: {format} is a high-range format; \
             --tone-map writes an 8-bit PNG",
            out.display()
        ),
        ImageFormat::Exr => format!(
            "{start}, as OpenEXR to {files}: {format} is a high-range format; --tone-map writes \
             an 8-bit PNG"
        ),
        ImageFormat::Png if format.is_high_range() => {
            format!("{start}, tone-mapped to 8 bits, to {files}")
        }
        ImageFormat::Png => format!("{start}, to {files}"),
    }
}
