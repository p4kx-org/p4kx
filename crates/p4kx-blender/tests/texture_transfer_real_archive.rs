// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded installed DDS witnesses for GLB and Blender package conversion.

use std::fs::{self, File};
use std::io::{self, BufReader, Cursor, Read as _, Write as _};
use std::path::Path;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, LoadoutResolver};
use p4kx_blender::{Manifest, PackageInfo, PackageWriteContext, TextureEncoding};
use p4kx_extraction::texture_codec::{DdsPngEncoding, DdsTransfer};
use p4kx_extraction::{
    Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken, DataForge, DdsHeader, DdsRawHeader,
    Entry, FieldPath, MAX_DDS_HEADER_LEN, Texture,
};
use p4kx_gltf::{GlbOptions, MaterialMode};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestModel, TestMtl, TestMtlMaterial,
    check_glb, entity_schema,
};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

const SAMPLES: usize = 512;
const MIP: u32 = 5;
const MAX_DIMENSION: u32 = 512;
const MODEL_PATH: &str = "Objects/Test/transfer.cgf";
const MTL_PATH: &str = "Objects/Test/transfer.mtl";
const ENTITY: &str = "TRANSFER_WITNESS";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_colour_reaches_glb_and_blender_package() -> Check {
    verify().map_err(|_| "installed DDS GLB and package verification failed".into())
}

fn verify() -> Check {
    let archive = Archive::open_with_key(
        p4kx_testkit::real_archive_path()?,
        p4kx_testkit::real_archive_key().ok(),
    )?;
    let watch = ArchiveWatch::new(&archive);
    watch.check()?;
    let chosen = select(&archive, &watch).map_err(|_| "installed source selection failed")?;
    let paths = p4kx_extraction::PathIndex::new(&archive);
    let loader = p4kx_extraction::ArchiveTextures::new(&paths);
    let scratch = tempfile::tempdir()?;
    let retained =
        std::env::var_os("P4KX_INSTALLED_TRANSFER_PACKAGE").map(std::path::PathBuf::from);
    let target = retained.as_deref().unwrap_or_else(|| scratch.path());
    fs::create_dir_all(target)?;
    let mut count = 0;
    for (entry, transfer) in chosen
        .into_iter()
        .zip([DdsTransfer::Linear, DdsTransfer::Srgb])
    {
        let leaf = if transfer == DdsTransfer::Linear {
            "linear"
        } else {
            "srgb"
        };
        let root = target.join(leaf);
        fs::create_dir_all(&root)?;
        let assembly = assembly(&root, entry.path()).map_err(|_| "synthetic assembly failed")?;
        let texture = Texture::from_entry(&archive, entry)?;
        let level = MIP.min(texture.header().mip_count().saturating_sub(1));
        let expected = texture.decode_mip(level)?;
        let options = GlbOptions::default()
            .with_materials(MaterialMode::Textures)
            .with_texture_mip(MIP);
        let mut glb = Vec::new();
        assembly.write_glb_with(&options, &loader, &mut glb)?;
        check_embedded(&glb, expected.pixels(), transfer).map_err(|_| "GLB inspection failed")?;
        write_package(&assembly, &archive, &loader, &root, &options)
            .map_err(|_| "package writer failed")?;
        check_package(&root, expected.pixels(), transfer, entry.is_encrypted())
            .map_err(|_| "package inspection failed")?;
        watch.check()?;
        count += 1;
    }
    writeln!(
        io::stdout().lock(),
        "Verified actual installed DDS through GLB and package: {count} transfer groups"
    )?;
    Ok(())
}

fn select<'a>(archive: &'a Archive, watch: &ArchiveWatch) -> Check<[&'a Entry; 2]> {
    let entries: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .as_bytes()
                .last_chunk::<9>()
                .is_some_and(|suffix| suffix.eq_ignore_ascii_case(b"_diff.dds"))
                && !entry.path().contains(['*', '?'])
        })
        .collect();
    let count = entries.len().min(SAMPLES);
    let mut chosen = [None; 2];
    for ordinal in 0..count {
        watch.poll()?;
        let entry = *entries
            .get(ordinal * entries.len() / count)
            .ok_or("sample position")?;
        let mut prefix = Vec::with_capacity(MAX_DDS_HEADER_LEN);
        archive
            .open_entry(entry)?
            .take(MAX_DDS_HEADER_LEN as u64)
            .read_to_end(&mut prefix)?;
        let raw = DdsRawHeader::parse(&prefix)?;
        let header = DdsHeader::from_raw(&raw)?;
        let level = MIP.min(header.mip_count().saturating_sub(1));
        if header.images(level) != 1
            || !header.format().is_decodable()
            || header.format().is_high_range()
            || header
                .width()
                .max(header.height())
                .checked_shr(level)
                .unwrap_or(0)
                > MAX_DIMENSION
            || Texture::from_entry(archive, entry)?.has_alpha_tail()
        {
            continue;
        }
        let transfer = DdsPngEncoding::of(header.format(), raw.transfer().effective).transfer;
        let index = match transfer {
            DdsTransfer::Linear => 0,
            DdsTransfer::Srgb => 1,
            _ => continue,
        };
        let slot = chosen.get_mut(index).ok_or("transfer stratum")?;
        if slot.is_none() {
            *slot = Some(entry);
        }
        if let [Some(linear), Some(srgb)] = chosen {
            return Ok([linear, srgb]);
        }
    }
    Err("two installed colour strata".into())
}

fn assembly(root: &Path, path: &str) -> Check<Assembly> {
    let model = TestModel::cube();
    let material = TestMtlMaterial::new("surface")
        .diffuse([1.0; 3])
        .opacity(0.5)
        .texture("TexSlot1", path);
    let material = TestMtl::multi(&[TestMtlMaterial::new("unused"), material.clone(), material]);
    let archive_path = root.join("mesh.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("Data/{MODEL_PATH}"), model.cgf()))
        .entry(TestEntry::stored(
            "Data/Objects/Test/transfer.cgfm",
            model.cgfm(),
        ))
        .entry(TestEntry::stored(
            format!("Data/{MTL_PATH}"),
            material.cryxml(),
        ))
        .write_to(&archive_path)?;
    let archive = Archive::open(archive_path)?;
    let models = ArchiveModels::new(&archive);
    let source = TestEntity::new(1, ENTITY, "qa/transfer.xml")
        .geometry(MODEL_PATH, Some("Objects/Test/transfer"));
    let bytes = entity_schema(DataForgeBuilder::new())
        .record(source.record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(ENTITY)?;
    let tree = resolver.resolve(&record)?;
    Ok(Assembly::build(
        &tree,
        &models,
        &AssemblyOptions::default(),
    )?)
}

fn write_package(
    assembly: &Assembly,
    source: &Archive,
    loader: &p4kx_extraction::ArchiveTextures<'_, '_>,
    root: &Path,
    options: &GlbOptions,
) -> Check {
    let archive = Archive::open(root.join("mesh.p4k"))?;
    let models = ArchiveModels::new(&archive);
    let cancel = CancelToken::new();
    let protection = ArchiveWriteProtection::new(source.path(), cancel.flag())?;
    let context =
        PackageWriteContext::new(&protection, cancel.flag()).with_archive_texture_source(loader);
    let summary = p4kx_blender::write_package_with_context(
        assembly,
        &models,
        loader,
        options,
        &PackageInfo::default(),
        &root.join("package"),
        &context,
    )?;
    if summary.textures != 1 || summary.texture_failures != 0 {
        return Err("source selected exact package texture".into());
    }
    Ok(())
}

fn check_embedded(bytes: &[u8], expected: &[u8], transfer: DdsTransfer) -> Check {
    let glb = check_glb(bytes)?;
    let image = glb.document.images().next().ok_or("embedded image")?;
    let metadata: serde_json::Value =
        serde_json::from_str(image.extras().as_ref().ok_or("extras")?.get())?;
    if metadata.pointer("/p4kx_transfer/output") != Some(&serde_json::json!("srgb"))
        || metadata.pointer("/p4kx_transfer/inputs/0/declared")
            != Some(&serde_json::json!(transfer))
    {
        return Err("GLB transfer metadata".into());
    }
    let actual = glb.decode(&image)?.rgba;
    if actual.len() != expected.len() {
        return Err("GLB image extent".into());
    }
    for (source, output) in expected
        .as_chunks::<4>()
        .0
        .iter()
        .zip(actual.as_chunks::<4>().0)
    {
        let [sr, sg, sb, sa] = *source;
        let [red, green, blue, alpha] = *output;
        if [red, green, blue]
            .into_iter()
            .zip([sr, sg, sb])
            .any(|(out, src)| {
                out.abs_diff(expected_srgb(src, transfer))
                    > u8::from(transfer == DdsTransfer::Linear)
            })
            || alpha != sa
        {
            return Err("GLB channel values".into());
        }
    }
    let reader = png::Decoder::new(Cursor::new(glb.png(&image)?)).read_info()?;
    if reader.info().srgb.is_none() {
        return Err("GLB PNG has no sRGB tag".into());
    }
    Ok(())
}

fn expected_srgb(sample: u8, transfer: DdsTransfer) -> u8 {
    if transfer == DdsTransfer::Srgb {
        return sample;
    }
    let linear = f64::from(sample) / 255.0;
    let srgb = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055_f64.mul_add(linear.powf(1.0 / 2.4), -0.055)
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "rounded byte range"
    )]
    let value = (srgb * 255.0).round() as u8;
    value
}

fn check_package(root: &Path, expected: &[u8], transfer: DdsTransfer, encrypted: bool) -> Check {
    let path = root.join("package");
    let manifest: Manifest = serde_json::from_slice(&fs::read(path.join("scene.json"))?)?;
    let row = manifest.textures.first().ok_or("package texture")?;
    let metadata = row.source_metadata.as_deref().ok_or("source metadata")?;
    let expected_encoding = if transfer == DdsTransfer::Linear {
        TextureEncoding::LinearColour
    } else {
        TextureEncoding::Srgb
    };
    let origin = metadata
        .field_origins
        .get(&FieldPath::parse("/output_transfer")?)
        .ok_or("output transfer origin")?;
    if metadata.output_transfer != Some(transfer)
        || row.encoding != Some(expected_encoding)
        || origin.contains_encrypted_source() != Some(encrypted)
    {
        return Err("package transfer or exact origin".into());
    }
    let mut image =
        png::Decoder::new(BufReader::new(File::open(path.join(&row.file))?)).read_info()?;
    if image.info().srgb.is_some() != (transfer == DdsTransfer::Srgb)
        || image.info().gama_chunk.map(png::ScaledFloat::into_value)
            != (transfer == DdsTransfer::Linear).then_some(1.0)
    {
        return Err("package PNG transfer tag".into());
    }
    let mut pixels = vec![0; image.output_buffer_size().ok_or("PNG image size")?];
    image.next_frame(&mut pixels)?;
    if pixels != expected {
        return Err("package colour and alpha bytes".into());
    }
    Ok(())
}
