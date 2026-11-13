// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::Path;

use p4kx_extraction::{
    Archive, CubemapLayout, Entry, Exclusion, Filter, PathIndex, Texture, TextureImage,
    TextureImageFormat, TextureImagePlan,
};
use p4kx_media::{ExportOptions, ExportOutcome, Identity, MediaKind, MediaRecord};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::super::TestResult;

pub(super) fn one(
    archive: &Archive,
    original_paths: &PathIndex<'_>,
    copied: &Entry,
    original: &Entry,
    root: &Path,
) -> TestResult {
    let path = original.path();
    let output = root.join(original.index().to_string());
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        include: Some(Filter::parse(path)?),
        exclude: Exclusion::parse(["*.pak", "*.socpak"])?,
        texture_mip: Some(0),
        ..ExportOptions::default()
    };
    let ExportOutcome::Published { index, .. } = p4kx_media::export(archive, &output, &options)?
    else {
        return Err("selected texture generation was incomplete".into());
    };
    let base = output.join(&index.artifact_base);
    let texture = Texture::from_index(original_paths, path)?;
    let records = rows(&base, copied.index())?;
    let metadata = records
        .iter()
        .find(|row| row.subresource.namespace == "texture.dds-header")
        .and_then(|row| row.data.pointer("/payload/source_metadata"));
    if metadata != Some(&serde_json::to_value(texture.source_metadata())?) {
        return Err("Media source metadata differs from the original DDS read".into());
    }
    let images: Vec<_> = records
        .iter()
        .filter(|row| row.subresource.namespace == "texture.dds-image")
        .collect();
    compare_images(&texture, &images, &base, root, original.index())
}

fn rows(base: &Path, source_index: usize) -> TestResult<Vec<MediaRecord>> {
    let content = fs::read_to_string(base.join("texture.jsonl"))?;
    let mut rows = Vec::new();
    for line in content.lines() {
        let row: MediaRecord = serde_json::from_str(line)?;
        if matches!(&row.source.identity, Identity::Entry { containers, entry }
            if containers.is_empty() && entry.entry_index == source_index)
        {
            rows.push(row);
        }
    }
    Ok(rows)
}

fn compare_images(
    texture: &Texture<'_>,
    images: &[&MediaRecord],
    base: &Path,
    root: &Path,
    index: usize,
) -> TestResult {
    let header = texture.header();
    let format = TextureImageFormat::for_header(header);
    let plan = TextureImagePlan::new(header, 0, CubemapLayout::Faces, format)?;
    let direct = root.join(format!("direct-{index}"));
    fs::create_dir_all(&direct)?;
    let filenames = if format == TextureImageFormat::Exr {
        texture
            .exr_to_file_as(&direct.join("image.exr"), 0, CubemapLayout::Faces)?
            .files
    } else {
        texture
            .png_to_file_as(&direct.join("image.png"), 0, CubemapLayout::Faces)?
            .files
    };
    if filenames.len() != images.len() || plan.len() != images.len() {
        return Err("ordinary conversion and Media image set disagree".into());
    }
    for ((planned, image), direct) in plan.zip(images).zip(filenames) {
        let payload = image.data.get("payload").ok_or("image payload")?;
        geometry(&planned, payload)?;
        content(base, payload, &direct)?;
    }
    Ok(())
}

fn geometry(planned: &TextureImage, payload: &Value) -> TestResult {
    let format = if planned.format == TextureImageFormat::Exr {
        "open-exr"
    } else {
        "png"
    };
    for (field, expected) in [
        ("format", json!(format)),
        ("bit_depth", json!(planned.format.bit_depth())),
        ("level", json!(planned.level)),
        ("layer", json!(planned.layer)),
        ("face", serde_json::to_value(planned.face)?),
        ("slice", json!(planned.slice)),
        ("suffix", json!(planned.suffix)),
        ("width", json!(planned.width)),
        ("height", json!(planned.height)),
    ] {
        if payload.get(field) != Some(&expected) {
            return Err("original topology and Media image geometry disagree".into());
        }
    }
    Ok(())
}

fn content(base: &Path, payload: &Value, direct: &Path) -> TestResult {
    use std::fmt::Write as _;

    let path = payload
        .pointer("/artifact/path")
        .and_then(Value::as_str)
        .ok_or("artifact path")?;
    let media = fs::read(base.join(path))?;
    if media != fs::read(direct)? {
        return Err("ordinary conversion and Media image bytes disagree".into());
    }
    let mut digest = String::new();
    for byte in Sha256::digest(&media) {
        write!(digest, "{byte:02x}")?;
    }
    if payload.pointer("/artifact/digest/hex") != Some(&json!(digest)) {
        return Err("Media artifact digest differs from its bytes".into());
    }
    Ok(())
}
