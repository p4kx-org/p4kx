// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_extraction::{ConversionError, CubemapLayout, DdsSourceInfo, Texture, TextureError};
use serde::Serialize;

use crate::cli::{TextureAction, TextureArgs, TextureSource};
use crate::json;
use crate::texture_image::{ImageOptions, export_image};

const UNKNOWN: &str = "unknown";
const NONE: &str = "none";

#[derive(Serialize)]
struct InfoJson<'a> {
    texture: &'a str,
    format: String,
    width: u32,
    height: u32,
    depth: u32,
    mip_count: u32,
    array_size: u32,
    faces: u32,
    cubemap: bool,
    parts: Option<u32>,
    alpha_tail: Option<AlphaTailJson>,
    source_metadata: DdsSourceInfo,
}

#[derive(Serialize)]
struct AlphaTailJson {
    format: Option<String>,
    parts: Option<u32>,
    error: Option<String>,
}

impl AlphaTailJson {
    fn of(tail: Result<Texture<'_>, ConversionError>) -> Self {
        match tail {
            Ok(tail) => Self {
                format: Some(tail.header().format().to_string()),
                parts: tail.part_count().ok(),
                error: None,
            },
            Err(error) => Self {
                format: None,
                parts: None,
                error: Some(error_chain(&error)),
            },
        }
    }

    fn describe(&self) -> String {
        match (&self.format, &self.error) {
            (_, Some(error)) => format!("broken: {error}"),
            (Some(format), None) => format!("{format}, {} parts", count(self.parts)),
            (None, None) => UNKNOWN.to_owned(),
        }
    }
}

fn error_chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

#[derive(Serialize)]
struct MergeJson<'a> {
    texture: &'a str,
    output: String,
    parts: u32,
    size: u64,
}

/// Runs `p4kx texture info`, `merge` or `png`.
pub(crate) fn run(args: &TextureArgs, as_json: bool) -> Result<()> {
    match &args.action {
        TextureAction::Info(source) => with_texture(source, |texture| info(texture, as_json)),
        TextureAction::Merge { source, out } => {
            with_texture(source, |texture| merge(texture, out, as_json))
        }
        TextureAction::Png {
            source,
            out,
            mip,
            cross,
            tone_map,
        } => {
            let options = ImageOptions {
                mip: *mip,
                layout: if *cross {
                    CubemapLayout::Cross
                } else {
                    CubemapLayout::Faces
                },
                tone_map: *tone_map,
            };
            with_texture(source, |texture| {
                export_image(texture, out, options, as_json)
            })
        }
    }
}

fn with_texture(
    source: &TextureSource,
    run: impl FnOnce(&Texture<'_>) -> Result<()>,
) -> Result<()> {
    match &source.entry {
        Some(entry) => {
            let archive = crate::key::open(&source.path)?;
            run(&Texture::from_archive(&archive, entry)?)
        }
        None => run(&Texture::from_file(&source.path)?),
    }
}

fn info(texture: &Texture<'_>, as_json: bool) -> Result<()> {
    let header = texture.header();
    let info = InfoJson {
        texture: texture.name(),
        format: header.format().to_string(),
        width: header.width(),
        height: header.height(),
        depth: header.depth(),
        mip_count: header.mip_count(),
        array_size: header.array_size(),
        faces: header.faces(),
        cubemap: header.is_cubemap(),
        parts: texture.part_count().ok(),
        alpha_tail: texture.alpha_tail().map(AlphaTailJson::of),
        source_metadata: texture.source_metadata(),
    };
    if as_json {
        json::print(&info)?;
        return Ok(());
    }
    let lines = [
        ("Texture", info.texture.to_owned()),
        ("Format", info.format),
        ("Size", format!("{} x {}", info.width, info.height)),
        ("Depth", info.depth.to_string()),
        ("Mips", info.mip_count.to_string()),
        ("Array size", info.array_size.to_string()),
        ("Faces", info.faces.to_string()),
        ("Parts", count(info.parts)),
        (
            "Alpha tail",
            info.alpha_tail
                .as_ref()
                .map_or_else(|| NONE.to_owned(), AlphaTailJson::describe),
        ),
    ];
    let mut out = io::stdout().lock();
    for (label, value) in lines {
        writeln!(out, "{:<12} {value}", format!("{label}:"))?;
    }
    Ok(())
}

fn merge(texture: &Texture<'_>, out: &Path, as_json: bool) -> Result<()> {
    let report = texture.merge_to_file(out)?;
    if as_json {
        json::print(&MergeJson {
            texture: texture.name(),
            output: json::path(out),
            parts: report.parts,
            size: report.size,
        })?;
    } else {
        writeln!(
            io::stdout().lock(),
            "merged {} and {} parts into {}, {} bytes",
            texture.name(),
            report.parts,
            out.display(),
            report.size
        )?;
    }
    Ok(())
}

fn count(parts: Option<u32>) -> String {
    parts.map_or_else(|| UNKNOWN.to_owned(), |parts| parts.to_string())
}

/// Returns whether `error` is a texture, sound or chunk file failure caused by the command line: an
/// Entry or file that does not exist, a mip level the texture or a chunk the chunk file does not have,
/// or an output that is one of the inputs.
pub(crate) fn is_invalid_input(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ConversionError>(),
        Some(
            ConversionError::UnknownEntry(_)
                | ConversionError::NoSuchFile { .. }
                | ConversionError::OutputIsInput { .. }
                | ConversionError::NoSuchChunk { .. }
                | ConversionError::ChunkNotText { .. }
                | ConversionError::UiCanvas { .. }
                | ConversionError::Texture {
                    source: TextureError::NoSuchMip { .. },
                    ..
                }
        )
    )
}

/// Returns whether `error` is a texture, sound or chunk file failure caused by an unreadable,
/// corrupt or too large Archive, Entry or input file.
pub(crate) fn is_archive_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ConversionError>(),
        Some(
            ConversionError::Archive(_)
                | ConversionError::ReadEntry { .. }
                | ConversionError::Read { .. }
                | ConversionError::ReadPart { .. }
                | ConversionError::ChunkFile { .. }
                | ConversionError::ChunkFileTooLarge { .. }
                | ConversionError::ChunkText { .. }
                | ConversionError::SoundTooLarge { .. }
                | ConversionError::UiMovie { .. }
                | ConversionError::UiMovieTooLarge { .. }
                | ConversionError::UiTexts { .. }
                | ConversionError::Texture {
                    source: TextureError::ReadHeader(_),
                    ..
                }
        )
    )
}
