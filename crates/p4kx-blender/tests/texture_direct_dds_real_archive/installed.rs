// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Read as _, Write as _};
use std::path::Path;

use p4kx_assembly::ArchiveModels;
use p4kx_blender::{
    Manifest, PackageInfo, PackageTextureContainer, PackageWriteContext, TextureFormat,
};
use p4kx_extraction::texture_codec::{DdsTransfer, DxgiFormat, TextureFormat as DdsFormat};
use p4kx_extraction::{
    Archive, ArchiveTextures, ArchiveWatch, ArchiveWriteProtection, CancelToken, DdsHeader,
    DdsRawHeader, Entry, FieldPath, MAX_DDS_HEADER_LEN, OriginDigestWriter, PathIndex, Texture,
};
use p4kx_gltf::GlbOptions;

use super::{Check, MAX_DIMENSION, MIP, SAMPLES, assembly};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_selected_dds_reaches_blender_package_without_changing_mip_bytes() -> Check {
    verify_direct_dds().map_err(|_| "installed verified DDS package check failed".into())
}

fn verify_direct_dds() -> Check {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let paths = PathIndex::new(&archive);
    let loader = ArchiveTextures::new(&paths);
    let scratch = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_INSTALLED_DDS_PACKAGE").map(std::path::PathBuf::from);
    let target = retained.as_deref().unwrap_or_else(|| scratch.path());
    let selected = select_direct_dds(&archive, &loader, &watch)?;
    for (index, entry) in selected.into_iter().enumerate() {
        let root = target.join(index.to_string());
        fs::create_dir_all(&root)?;
        verify_direct_package(&archive, &loader, entry, &root)?;
        watch.check()?;
    }
    writeln!(
        io::stdout().lock(),
        "Verified actual installed linear and sRGB DDS package mip bytes"
    )?;
    Ok(())
}

fn select_direct_dds<'a>(
    archive: &'a Archive,
    loader: &ArchiveTextures<'_, 'a>,
    watch: &ArchiveWatch,
) -> Check<[&'a Entry; 2]> {
    let candidates: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .as_bytes()
                .last_chunk::<9>()
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(b"_diff.dds"))
        })
        .collect();
    let count = candidates.len().min(SAMPLES);
    let mut chosen = [None; 2];
    for ordinal in 0..count {
        watch.poll()?;
        let entry = *candidates
            .get(ordinal * candidates.len() / count)
            .ok_or("sample position")?;
        let Ok(Some(transfer)) = direct_candidate(archive, entry) else {
            continue;
        };
        let Some(path) = entry.path().get(5..) else {
            continue;
        };
        if loader.load_dds(path, MIP).is_err() {
            continue;
        }
        let slot = match transfer {
            DdsTransfer::Linear => &mut chosen[0],
            DdsTransfer::Srgb => &mut chosen[1],
            _ => continue,
        };
        if slot.is_none() {
            *slot = Some(entry);
        }
        if let [Some(linear), Some(srgb)] = chosen {
            return Ok([linear, srgb]);
        }
    }
    Err("two direct DDS strata".into())
}

fn direct_candidate(archive: &Archive, entry: &Entry) -> Check<Option<DdsTransfer>> {
    let mut prefix = Vec::with_capacity(MAX_DDS_HEADER_LEN);
    archive
        .open_entry(entry)?
        .take(MAX_DDS_HEADER_LEN as u64)
        .read_to_end(&mut prefix)?;
    let raw = DdsRawHeader::parse(&prefix)?;
    let header = DdsHeader::from_raw(&raw)?;
    let level = MIP.min(header.mip_count().saturating_sub(1));
    let extent = header
        .width()
        .max(header.height())
        .checked_shr(level)
        .unwrap_or(0);
    let transfer = raw.transfer().effective;
    let supported = matches!(
        header.format(),
        DdsFormat::Dxgi(DxgiFormat(28 | 29 | 71 | 72 | 77 | 78))
    );
    Ok((supported
        && header.images(level) == 1
        && extent <= MAX_DIMENSION
        && matches!(transfer, DdsTransfer::Linear | DdsTransfer::Srgb))
    .then_some(transfer))
}

fn verify_direct_package(
    archive: &Archive,
    loader: &ArchiveTextures<'_, '_>,
    entry: &Entry,
    root: &Path,
) -> Check {
    let assembly = assembly(root, entry.path())?;
    let mesh_archive = Archive::open(root.join("mesh.p4k"))?;
    let models = ArchiveModels::new(&mesh_archive);
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_archive_texture_source(loader)
        .with_texture_container(PackageTextureContainer::BlenderDds);
    let options = GlbOptions::default().with_texture_mip(MIP);
    let output = root.join("package");
    let summary = p4kx_blender::write_package_with_context(
        &assembly,
        &models,
        loader,
        &options,
        &PackageInfo::default(),
        &output,
        &context,
    )?;
    if summary.textures != 1 || summary.texture_failures != 0 {
        return Err("one successful original DDS".into());
    }
    verify_direct_file(archive, entry, &output)
}

fn verify_direct_file(archive: &Archive, entry: &Entry, output: &Path) -> Check {
    let original = Texture::from_entry(archive, entry)?;
    let level = MIP.min(original.header().mip_count().saturating_sub(1));
    let raw_mip = original.read_mip(level)?;
    let part_count = original.part_count()?;
    let part = if level < part_count {
        let path = format!("{}.{}", entry.path(), part_count - level);
        Some(
            archive
                .entries()
                .iter()
                .find(|candidate| candidate.path().eq_ignore_ascii_case(&path))
                .ok_or("expected exact original part")?,
        )
    } else {
        None
    };
    let manifest: Manifest = serde_json::from_slice(&fs::read(output.join("scene.json"))?)?;
    let row = manifest.textures.first().ok_or("package texture")?;
    let receipt = row.dds_asset.as_deref().ok_or("package DDS receipt")?;
    let bytes = fs::read(output.join(&row.file))?;
    let raw = DdsRawHeader::parse(&bytes)?;
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(&bytes)?;
    let origin = receipt
        .field_origins
        .get(&FieldPath::parse("/artifact")?)
        .ok_or("artifact origin")?;
    let encrypted = entry.is_encrypted() || part.is_some_and(Entry::is_encrypted);
    if !origin.source_set_complete()
        || origin.contains_encrypted_source() != Some(encrypted)
        || receipt.part_checksum.is_some() != part.is_some()
    {
        return Err("selected original DDS provenance".into());
    }
    if row.format != TextureFormat::Dds
        || receipt.mip_level != level
        || bytes.get(raw.header_len()..) != Some(raw_mip.data())
        || receipt.artifact != digest.finish()?
        || receipt.dds.transfer.map(|transfer| transfer.effective) != Some(raw.transfer().effective)
    {
        return Err("original mip payload and written artifact binding".into());
    }
    Ok(())
}
