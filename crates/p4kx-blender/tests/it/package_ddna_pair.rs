// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_assembly::{ArchiveModels, Assembly};
use p4kx_blender::{
    AlphaMeaning, DdnaSelection, DdsTextureAsset, Manifest, PackageError, PackageInfo,
    PackageTextureContainer, PackageWriteContext, Texture, TextureFormat, TextureRendition,
    manifest_schema, write_package_with_context,
};
use p4kx_extraction::{Archive, ArchiveWriteProtection, CancelToken, FieldPath};
use p4kx_extraction::{OriginArtifactDigest, OriginDigestWriter};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat, layered_entries, test_key};
use serde_json::Value;
use std::io::{self, Write as _};

use crate::support::{TestResult, assemble, fixture, glb_options};

const NORMAL: &str = r"Data\Objects\Test\layers\textures\base_ddna.dds";
const TEST_NORMAL_BLOCK: [u8; 16] = [255, 255, 0, 0, 0, 0, 0, 0, 30, 85, 30, 85, 0, 0, 0, 0];
const SMOOTHNESS_BLOCK: [u8; 8] = [64, 64, 0, 0, 0, 0, 0, 0];

#[test]
fn layered_package_publishes_and_authenticates_both_dds_assets() -> TestResult {
    let fixture = fixture(source_entries())?;
    let source = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let assembly = assemble(&source, "TEST_Layered")?;
    let folder = tempfile::tempdir()?;
    let preserved = std::env::var_os("P4KX_SYNTHETIC_DDNA_PACKAGE").map(PathBuf::from);
    let folder = preserved.as_deref().unwrap_or_else(|| folder.path());
    let dds = folder.join("dds");
    let png = folder.join("png");
    publish(
        &source,
        &assembly,
        &dds,
        PackageTextureContainer::BlenderDds,
        &CancelToken::new(),
    )?;
    publish(
        &source,
        &assembly,
        &png,
        PackageTextureContainer::Png,
        &CancelToken::new(),
    )?;
    let original = manifest_bytes(&dds)?;
    let json: Value = serde_json::from_slice(&original)?;
    jsonschema::validator_for(&manifest_schema())?
        .validate(&json)
        .map_err(|error| error.to_string())?;
    let manifest: Manifest = serde_json::from_value(json)?;
    verify(&dds, &manifest)?;
    verify_default_png(&png)?;
    verify_replacement(&source, &assembly, &dds, &original)
}

fn verify_default_png(png: &Path) -> TestResult {
    let png_scene: Manifest = serde_json::from_slice(&manifest_bytes(png)?)?;
    let normal = png_scene
        .textures
        .iter()
        .find(|row| row.source.ends_with("base_ddna.dds"))
        .ok_or("default normal")?;
    assert_eq!(
        normal.format,
        TextureFormat::Png,
        "native DDS remains opt-in"
    );
    assert_eq!(
        normal.alpha,
        AlphaMeaning::Smoothness,
        "default PNG composes smoothness in alpha"
    );
    assert!(
        normal.ddna_pair.is_none(),
        "no separate file in the PNG route"
    );
    Ok(())
}

fn verify_replacement(
    source: &Archive,
    assembly: &Assembly,
    dds: &Path,
    original: &[u8],
) -> TestResult {
    let receipt = fs::read(dds.join("receipt.json"))?;
    publish(
        source,
        assembly,
        dds,
        PackageTextureContainer::BlenderDds,
        &CancelToken::new(),
    )?;
    assert_eq!(
        manifest_bytes(dds)?,
        original,
        "replacement is deterministic"
    );
    assert_eq!(
        fs::read(dds.join("receipt.json"))?,
        receipt,
        "exact replacement receipt"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        matches!(
            publish(
                source,
                assembly,
                dds,
                PackageTextureContainer::BlenderDds,
                &cancel
            ),
            Err(PackageError::Cancelled)
        ),
        "cancelled replacement"
    );
    assert_eq!(
        manifest_bytes(dds)?,
        original,
        "cancel preserves earlier complete package"
    );
    Ok(())
}

fn publish(
    source: &Archive,
    assembly: &Assembly,
    out: &Path,
    container: PackageTextureContainer,
    cancel: &CancelToken,
) -> Result<(), PackageError> {
    let models = ArchiveModels::new(source);
    let loader = models.textures();
    let pending = CancelToken::new();
    let protection = ArchiveWriteProtection::new(source.path(), pending.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag())
        .with_archive_texture_source(&loader)
        .with_texture_container(container);
    write_package_with_context(
        assembly,
        &models,
        &loader,
        &glb_options(9),
        &PackageInfo::default(),
        out,
        &context,
    )?;
    Ok(())
}

fn manifest_bytes(package: &Path) -> io::Result<Vec<u8>> {
    fs::read(package.join("scene.json"))
}

fn verify(package: &Path, manifest: &Manifest) -> TestResult {
    let normal = manifest
        .textures
        .iter()
        .find(|row| row.source.ends_with("base_ddna.dds"))
        .ok_or("the normal DDS")?;
    let pair = normal.ddna_pair.as_deref().ok_or("DDNA relationship")?;
    let smoothness = manifest
        .textures
        .iter()
        .find(|row| row.id == pair.smoothness)
        .ok_or("the separate DDS texture")?;
    assert_eq!(pair.requested_mip, 9, "original requested mip");
    assert_eq!(pair.selected_mip, 1, "same common mip as composed PNG");
    assert_eq!(
        pair.selection,
        DdnaSelection::SmoothnessChain,
        "shorter smoothness chain"
    );
    assert_eq!(
        pair.source_origin.contains_encrypted_source(),
        Some(true),
        "joint pair link uses encrypted selected tail part"
    );
    assert_eq!(
        manifest
            .textures
            .iter()
            .filter(|texture| texture.id == pair.smoothness)
            .count(),
        1,
        "one reused companion row"
    );
    let receipt = package_receipt(package)?;
    let files = receipt.as_array().ok_or("file receipt rows")?;
    assert_eq!(
        [normal.source_rendition, smoothness.source_rendition],
        [
            Some(TextureRendition::NormalSmoothness),
            Some(TextureRendition::SeparateSmoothness)
        ],
        "records the exact DDNA member roles"
    );
    for (texture, block, encrypted) in [
        (normal, TEST_NORMAL_BLOCK.as_slice(), false),
        (smoothness, SMOOTHNESS_BLOCK.as_slice(), true),
    ] {
        verify_pair_member(package, files, texture, block, encrypted)?;
    }
    Ok(())
}

fn verify_pair_member(
    package: &Path,
    files: &[Value],
    texture: &Texture,
    block: &[u8],
    encrypted: bool,
) -> TestResult {
    assert_eq!(texture.format, TextureFormat::Dds, "each texture uses DDS");
    assert_eq!(
        (texture.width, texture.height),
        (3, 2),
        "matches the negotiated mip"
    );
    let file = texture.file.as_str();
    let asset = texture.dds_asset.as_deref().ok_or("per-file evidence")?;
    let hash = verify_dds(package, file, asset, block, encrypted)?;
    let listed = files
        .iter()
        .find(|row| row.get("path").and_then(Value::as_str) == Some(file))
        .ok_or("declared receipt member")?;
    assert_eq!(
        listed.get("sha256").and_then(Value::as_str),
        Some(hash.as_str()),
        "receipt authenticates the DDS file"
    );
    Ok(())
}

fn package_receipt(package: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let receipt: Value = serde_json::from_slice(&fs::read(package.join("receipt.json"))?)?;
    receipt
        .get("files")
        .cloned()
        .ok_or_else(|| "file receipt rows".into())
}

fn verify_dds(
    package: &Path,
    file: &str,
    asset: &DdsTextureAsset,
    payload: &[u8],
    encrypted: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let data = fs::read(package.join(file))?;
    let header_length = usize::try_from(
        asset
            .normalized_header_bytes
            .ok_or("standalone header size")?,
    )?;
    assert_eq!(
        data.get(header_length..),
        Some(payload),
        "the selected BC block is unchanged"
    );
    let digest = asset.copied_payload.as_ref().ok_or("payload digest")?;
    assert_eq!(digest, &sha256(payload)?, "selected block SHA-256 and size");
    assert_eq!(
        digest.byte_length(),
        payload.len() as u64,
        "selected block length"
    );
    verify_source(asset, payload.len(), encrypted)?;
    let hash = sha256(&data)?.sha256().to_owned();
    assert_eq!(asset.artifact.sha256(), hash, "complete DDS SHA-256");
    Ok(hash)
}

fn verify_source(asset: &DdsTextureAsset, payload_bytes: usize, encrypted: bool) -> TestResult {
    let origin = asset
        .field_origins
        .get(&FieldPath::parse("/copied_payload")?)
        .ok_or("per-file payload origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(encrypted),
        "sibling encryption does not taint unrelated payload"
    );
    let source_use = matching_use(asset, origin)?;
    let path = if encrypted {
        format!("{NORMAL}.1a").to_ascii_uppercase()
    } else {
        format!("{NORMAL}.1")
    };
    let source_id = source_use
        .get("source")
        .and_then(Value::as_str)
        .ok_or("selected source id")?;
    let sources = asset
        .provenance
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("source rows")?;
    let input = sources
        .iter()
        .find(|source| source.get("id").and_then(Value::as_str) == Some(source_id))
        .ok_or("payload input")?;
    assert_eq!(
        input.pointer("/path/value").and_then(Value::as_str),
        Some(path.as_str()),
        "exact mixed-case source member"
    );
    assert_eq!(
        source_use.pointer("/location/indices"),
        Some(&serde_json::json!([
            "1",
            "0",
            payload_bytes.to_string(),
            "0",
            "1"
        ])),
        "original mip, byte offset and length, surface and depth",
    );
    assert_eq!(
        asset
            .field_origins
            .get(&FieldPath::parse("/normalized_header_bytes")?)
            .ok_or("normalized header origin")?
            .contains_encrypted_source(),
        Some(false),
        "only the selected smoothness part is encrypted",
    );
    Ok(())
}

fn matching_use<'a>(
    asset: &'a DdsTextureAsset,
    origin: &p4kx_extraction::FieldOrigin,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let [reference] = origin.source_uses_refs() else {
        return Err("expected one physical payload use".into());
    };
    let uses = asset
        .provenance
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    uses.iter()
        .find(|source| source.get("id").and_then(Value::as_str) == Some(reference.as_str()))
        .ok_or_else(|| "selected use".into())
}

fn sha256(data: &[u8]) -> io::Result<OriginArtifactDigest> {
    let mut hash = OriginDigestWriter::new(io::sink());
    hash.write_all(data)?;
    hash.finish()
}

fn source_entries() -> Vec<TestEntry> {
    let normal = SplitDds::new(TestTextureFormat::LegacyDxt5, 7, 5)
        .mips(3)
        .split(2);
    let bc4 = TestTextureFormat::Dx10Block {
        dxgi_format: 80,
        block_bytes: 8,
    };
    let tail = SplitDds::new(bc4, 7, 5).mips(2).split(2);
    let mut entries: Vec<_> = layered_entries()
        .into_iter()
        .filter(|entry| {
            !entry
                .path()
                .to_ascii_lowercase()
                .starts_with(&NORMAL.to_ascii_lowercase())
        })
        .collect();
    let split = normal.entries(NORMAL);
    entries.extend(split.into_iter().map(|entry| {
        if entry.path().ends_with(".1") {
            TestEntry::stored(entry.path(), TEST_NORMAL_BLOCK)
        } else {
            entry
        }
    }));
    entries.push(TestEntry::stored(
        format!("{NORMAL}.a").to_ascii_uppercase(),
        tail.header_bytes(),
    ));
    entries.push(
        TestEntry::stored(
            format!("{NORMAL}.1a").to_ascii_uppercase(),
            SMOOTHNESS_BLOCK,
        )
        .encrypted(),
    );
    entries.push(TestEntry::stored(format!("{NORMAL}.2a"), tail.part(2)));
    entries
}
