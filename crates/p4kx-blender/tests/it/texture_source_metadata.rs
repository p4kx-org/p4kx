// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_blender::{
    DdsTextureFallback, Manifest, PackageInfo, PackageTextureContainer, PackageWriteContext,
    TextureChannelTransform, TextureEncoding, TextureFormat, write_package_with_context,
};
use p4kx_extraction::texture_codec::{DdsPngTransform, DdsTransfer};
use p4kx_extraction::{
    Archive, ArchiveWriteProtection, CancelToken, FieldPath, OpenOptions, OriginDigestWriter,
};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat, layered_entries, test_key};

use crate::support::{TestResult, assemble, glb_options};

#[test]
fn package_texture_keeps_plain_dds_words_separate_from_used_encrypted_alpha() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_SYNTHETIC_TEXTURE_PACKAGE").map(PathBuf::from);
    let folder = retained.as_deref().unwrap_or_else(|| temporary.path());
    fs::create_dir_all(folder)?;
    let path = "Data\\Objects\\Test\\layers\\textures\\base_ddna.dds";
    let input = archive(folder, path)?;
    let manifest = write(&input, &folder.join("ship"), PackageTextureContainer::Png)?;
    verify_linear_colour(&manifest, folder)?;
    let receipt = manifest
        .textures
        .iter()
        .find(|texture| texture.source.ends_with("base_ddna.dds"))
        .and_then(|texture| texture.source_metadata.as_deref())
        .ok_or("same-read DDS metadata")?;
    assert_eq!(receipt.mip_level, 0, "actual requested mip");
    verify_normal_header(receipt)?;
    let image = receipt
        .field_origins
        .get(&FieldPath::parse("/image")?)
        .ok_or("image origin")?;
    assert_eq!(
        image.contains_encrypted_source(),
        Some(true),
        "actual split alpha supplies PNG pixels"
    );
    assert!(
        image.source_set_complete(),
        "complete color and alpha contributors"
    );
    Ok(())
}

fn verify_normal_header(receipt: &p4kx_blender::TextureSourceMetadata) -> TestResult {
    let width = receipt
        .field_origins
        .get(&FieldPath::parse("/dds/normalized/width")?)
        .ok_or("width origin")?;
    assert_eq!(
        width.contains_encrypted_source(),
        Some(false),
        "plain color header supplies width"
    );
    let transfer = receipt.dds.transfer.ok_or("transfer declarations")?;
    assert_eq!(
        transfer.effective,
        DdsTransfer::Unknown,
        "unmarked legacy source"
    );
    assert_eq!(
        receipt.output_transfer,
        Some(DdsTransfer::Unknown),
        "normal remains untagged"
    );
    assert_eq!(
        receipt.png_transform,
        Some(DdsPngTransform::DataChannels),
        "normal role keeps data channels"
    );
    assert_eq!(
        receipt.channel_transform,
        Some(TextureChannelTransform::FlipNormalGreen),
        "consumer Y transform remains independent from source transfer"
    );
    let origin = receipt
        .field_origins
        .get(&FieldPath::parse("/dds/transfer/effective")?)
        .ok_or("transfer origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "alpha does not affect colour transfer"
    );
    Ok(())
}

fn verify_linear_colour(manifest: &Manifest, root: &Path) -> TestResult {
    let colour = manifest
        .textures
        .iter()
        .find(|texture| texture.source.ends_with("base_diff.dds"))
        .ok_or("linear colour texture")?;
    assert_eq!(
        colour.encoding,
        Some(TextureEncoding::LinearColour),
        "linear colour is distinct from data"
    );
    let metadata = colour
        .source_metadata
        .as_deref()
        .ok_or("linear DDS metadata")?;
    assert_eq!(
        metadata.output_transfer,
        Some(DdsTransfer::Linear),
        "the package PNG is linear"
    );
    let origin = metadata
        .field_origins
        .get(&FieldPath::parse("/output_transfer")?)
        .ok_or("output transfer origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "the colour header is plain"
    );
    let image = fs::File::open(root.join("ship").join(&colour.file))?;
    let png = png::Decoder::new(std::io::BufReader::new(image)).read_info()?;
    assert_eq!(
        png.info().gama_chunk.map(png::ScaledFloat::into_value),
        Some(1.0),
        "real PNG transfer"
    );
    Ok(())
}

fn archive(folder: &Path, path: &str) -> Result<Archive, Box<dyn Error>> {
    let normal = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .alpha(TestTextureFormat::R8)
        .split(1);
    let alpha = normal.alpha_tail().ok_or("alpha fixture")?;
    let mut archive = p4kx_testkit::ArchiveBuilder::new();
    let part = format!("{path}.1");
    for entry in layered_entries() {
        if entry.path() != path && entry.path() != part && !entry.path().ends_with("base_diff.dds")
        {
            archive = archive.entry(entry);
        }
    }
    let colour = SplitDds::new(
        TestTextureFormat::Dx10Pixel {
            dxgi_format: 28,
            pixel_bytes: 4,
        },
        8,
        8,
    )
    .mips(1);
    let mut color_bytes = colour.header_bytes();
    let body = color_bytes
        .get_mut(colour.header_len()..)
        .ok_or("colour payload")?;
    for pixel in body.as_chunks_mut::<4>().0 {
        *pixel = [128, 64, 16, 81];
    }
    let output = folder.join("Data.p4k");
    archive
        .entry(TestEntry::stored(
            "Data\\Objects\\Test\\layers\\textures\\base_diff.dds",
            color_bytes,
        ))
        .entry(TestEntry::stored(path, normal.header_bytes()))
        .entry(TestEntry::stored(part, normal.part(1)))
        .entry(TestEntry::stored(format!("{path}.a"), alpha.header_bytes()))
        .entry(TestEntry::stored(format!("{path}.1a"), alpha.part(1)).encrypted())
        .write_to(&output)?;
    Ok(Archive::open_with(
        output,
        OpenOptions::new().key(Some(test_key())),
    )?)
}
fn write(
    archive: &Archive,
    package: &Path,
    container: PackageTextureContainer,
) -> Result<Manifest, Box<dyn Error>> {
    let assembly = assemble(archive, "TEST_Layered")?;
    let models = p4kx_assembly::ArchiveModels::new(archive);
    let loader = models.textures();
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_archive_texture_source(&loader)
        .with_texture_container(container);

    write_package_with_context(
        &assembly,
        &models,
        &loader,
        &glb_options(0),
        &PackageInfo::default(),
        package,
        &context,
    )?;
    let bytes = fs::read(package.join("scene.json"))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)?;
    let schema = p4kx_blender::manifest_schema();
    jsonschema::validator_for(&schema)?
        .validate(&value)
        .map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[test]
fn optional_dds_writes_exact_mip_and_digest_with_composed_png_fallback() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let retained = std::env::var_os("P4KX_SYNTHETIC_DDS_PACKAGE").map(PathBuf::from);
    let folder = retained.as_deref().unwrap_or_else(|| temporary.path());
    fs::create_dir_all(folder)?;
    let normal_path = r"Data\Objects\Test\layers\textures\base_ddna.dds";
    let input = archive(folder, normal_path)?;
    let package = folder.join("dds");
    let manifest = write(&input, &package, PackageTextureContainer::BlenderDds)?;
    verify_dds_asset(&manifest, &package)?;
    let normal = manifest
        .textures
        .iter()
        .find(|item| item.source.ends_with("base_ddna.dds"))
        .ok_or("PNG normal fallback")?;
    assert_eq!(
        normal.format,
        TextureFormat::Png,
        "composed texture stays PNG"
    );
    assert_eq!(
        normal.dds_fallback,
        Some(DdsTextureFallback::SplitAlpha),
        "split-alpha fallback is explicit"
    );
    let metadata = normal
        .source_metadata
        .as_deref()
        .ok_or("combined PNG receipt")?;
    let origin = metadata
        .field_origins
        .get(&FieldPath::parse("/image")?)
        .ok_or("combined image origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "fallback uses encrypted alpha"
    );
    let default_folder = folder.join("default");
    let default = write(&input, &default_folder, PackageTextureContainer::Png)?;
    let default_normal = default
        .textures
        .iter()
        .find(|item| item.id == normal.id)
        .ok_or("corresponding default normal")?;
    assert_eq!(
        default_normal.dds_fallback, None,
        "default PNG has no fallback"
    );
    assert_eq!(
        fs::read(package.join(&normal.file))?,
        fs::read(default_folder.join(&normal.file))?,
        "fallback and default use identical PNG bytes"
    );
    assert_eq!(
        manifest.textures.len(),
        default.textures.len(),
        "deduplication is independent of format"
    );
    Ok(())
}

fn verify_dds_asset(manifest: &Manifest, package: &Path) -> TestResult {
    use std::io::Write as _;

    let colour = manifest
        .textures
        .iter()
        .find(|item| item.source.ends_with("base_diff.dds"))
        .ok_or("DDS colour asset")?;
    assert_eq!(
        colour.format,
        TextureFormat::Dds,
        "compatible colour is stored in DDS"
    );
    assert_eq!(
        colour.encoding,
        Some(TextureEncoding::LinearColour),
        "source transfer drives importer sampling"
    );
    let asset = colour.dds_asset.as_deref().ok_or("same-read DDS receipt")?;
    assert_eq!(asset.mip_level, 0, "selected original mip");
    assert_eq!(
        asset.dds.transfer.ok_or("authored transfer")?.effective,
        DdsTransfer::Linear,
        "original transfer survives DDS normalization"
    );
    let artifact = fs::read(package.join(&colour.file))?;
    assert!(artifact.starts_with(b"DDS "), "standard magic");
    let body = artifact
        .get(asset.dds.raw.header_length..)
        .ok_or("DDS payload")?;
    assert_eq!(
        body,
        [128, 64, 16, 81].repeat(64),
        "unchanged authored samples"
    );
    let mut measured = OriginDigestWriter::new(std::io::sink());
    measured.write_all(&artifact)?;
    assert_eq!(
        asset.artifact,
        measured.finish()?,
        "written length and digest"
    );
    for field in ["/artifact", "/header_checksum"] {
        let origin = asset
            .field_origins
            .get(&FieldPath::parse(field)?)
            .ok_or(field)?;
        assert_eq!(
            origin.contains_encrypted_source(),
            Some(false),
            "{field} uses plain colour"
        );
        assert!(
            origin.source_set_complete(),
            "{field} binds complete source uses"
        );
    }
    Ok(())
}
