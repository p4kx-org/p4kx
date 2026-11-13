// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Filter;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::material_origin_support::{self as support, Fixture, MTL, SHADER, Storage, TestResult};

const SOURCE: &[u8] = b"<Material Shader='demo' StringGenMask='%A'/>";
const LINKS: &str = "/data/payload/shader_references";

#[test]
fn material_tokens_and_used_encrypted_shader_properties_have_distinct_immediate_markers()
-> TestResult {
    let fixture = Fixture::new([
        support::material(MTL, SOURCE, Storage::Plain),
        support::shader(Storage::Encrypted),
    ])?;
    let token = format!("{LINKS}/1/reference/original/value");
    let property = format!("{LINKS}/1/reference/resolution/target/subresource/indices/0");
    fixture.marker(MTL, &token, Some(false))?;
    fixture.marker(MTL, &property, Some(true))?;
    let input = format!("{LINKS}/1/input");
    fixture.marker(MTL, &format!("{input}/digest/hex"), Some(true))?;
    for pointer in [
        "digest/algorithm",
        "digest/scope",
        "source/root/algorithm",
        "source/root/scope",
    ] {
        fixture.independent(MTL, &format!("{input}/{pointer}"))?;
    }
    assert!(
        fixture.has_use(
            MTL,
            &format!("{LINKS}/1/reference/resolution/target/source/entry/path"),
            SHADER,
            "archiveDirectoryMetadata",
            "selector"
        )?,
        "the positive target path retains original candidate metadata without assuming global inventory is plain"
    );
    assert!(
        fixture.has_use(MTL, &property, MTL, "payload", "selector")?,
        "the material flag is the actual current selector"
    );
    assert!(
        fixture.has_use(MTL, &property, SHADER, "payload", "conversion")?,
        "the Property index derives from actual shader input"
    );
    fixture.no_payload(MTL, &token, SHADER)
}

#[test]
fn an_unknown_shader_declaration_does_not_make_plain_material_tokens_unknown() -> TestResult {
    let fixture = Fixture::new([
        support::material(MTL, SOURCE, Storage::Plain),
        support::shader(Storage::Unknown),
    ])?;
    fixture.marker(
        MTL,
        &format!("{LINKS}/1/reference/original/value"),
        Some(false),
    )?;
    let property = format!("{LINKS}/1/reference/resolution/target/subresource/indices/0");
    fixture.marker(MTL, &property, None)?;
    assert!(
        fixture.has_use(MTL, &property, SHADER, "payload", "conversion")?,
        "unknown encryption is retained on the actually used physical shader"
    );
    Ok(())
}

#[test]
fn an_untouched_encrypted_texture_target_contributes_metadata_only() -> TestResult {
    let source =
        b"<Material><Textures><Texture Map='TexSlot1' File='image.dds'/></Textures></Material>";
    let fixture = Fixture::new([
        support::material(MTL, source, Storage::Plain),
        support::material("Data/image.dds", b"unused image bytes", Storage::Encrypted),
    ])?;
    let reference = "/data/payload/references/0/reference";
    fixture.marker(MTL, &format!("{reference}/original/value"), Some(false))?;
    fixture.no_payload(
        MTL,
        &format!("{reference}/resolution/target/source/entry/path"),
        "Data/image.dds",
    )?;
    assert!(
        fixture.has_use(
            MTL,
            &format!("{reference}/resolution/target/source/entry/path"),
            "Data/image.dds",
            "archiveDirectoryMetadata",
            "selector"
        )?,
        "the original physical candidate metadata is retained"
    );
    fixture.no_payload(MTL, reference, "Data/image.dds")
}

#[test]
fn duplicate_shader_candidates_are_not_payload_inputs() -> TestResult {
    let fixture = Fixture::new([
        support::material(MTL, SOURCE, Storage::Plain),
        support::shader(Storage::Encrypted),
        support::material(
            "Data/other/demo.ext",
            support::DEFINITION,
            Storage::Encrypted,
        ),
    ])?;
    let pointer = format!("{LINKS}/0/reference/resolution");
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer(&format!("{pointer}/status"))
            .and_then(Value::as_str),
        Some("ambiguous")
    );
    fixture.marker(MTL, &format!("{pointer}/status"), None)?;
    assert!(
        fixture.has_root_inventory(MTL, &format!("{pointer}/status"))?,
        "complete ambiguity classification retains the original unknown root directory enumeration"
    );
    fixture.no_payload(MTL, &pointer, SHADER)?;
    fixture.no_payload(MTL, &pointer, "Data/other/demo.ext")?;
    assert!(
        fixture
            .record(MTL)?
            .1
            .pointer(&format!("{LINKS}/0/input"))
            .is_some_and(Value::is_null)
    );
    Ok(())
}

#[test]
fn filtered_shader_target_keeps_its_original_metadata_without_payload_taint() -> TestResult {
    let mut options = support::options();
    options.include = Some(Filter::parse("*.mtl")?);
    let fixture = Fixture::with(
        [
            support::material(MTL, SOURCE, Storage::Plain),
            support::shader(Storage::Encrypted),
        ],
        options,
        p4kx_extraction::model::MaterialSourceLimits::default(),
    )?;
    let pointer = format!("{LINKS}/0/reference/resolution/status");
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer(&pointer)
            .and_then(Value::as_str),
        Some("not-selected")
    );
    fixture.marker(MTL, &pointer, None)?;
    assert!(
        fixture.has_root_inventory(MTL, &pointer)?,
        "unique filtered classification retains the original inventory selector"
    );
    fixture.no_payload(MTL, &format!("{LINKS}/0/reference/resolution"), SHADER)
}

#[test]
fn cached_shader_binding_uses_each_materials_current_original_selector() -> TestResult {
    let first = "Data/first.mtl";
    let second = "Data/second.mtl";
    let fixture = Fixture::new([
        support::material(first, SOURCE, Storage::Plain),
        support::material(second, SOURCE, Storage::Encrypted),
        support::shader(Storage::Plain),
    ])?;
    let pointer = format!("{LINKS}/1/reference/resolution/target/subresource/indices/0");
    for (path, unused, marker) in [(first, second, None), (second, first, Some(true))] {
        fixture.marker(path, &pointer, marker)?;
        assert!(
            fixture.has_root_inventory(path, &pointer)?,
            "selecting the unique cached shader retains its original complete inventory"
        );
        fixture.marker(
            path,
            &format!("{LINKS}/1/reference/original/value"),
            Some(path == second),
        )?;
        assert!(
            fixture.has_use(path, &pointer, path, "payload", "selector")?,
            "every cache use has its current MTL selector"
        );
        assert!(
            !fixture
                .contributions(path, &pointer)?
                .iter()
                .any(|used| fixture.input_path(used) == Some(unused)),
            "a cache hit cannot inherit another material's selector"
        );
        assert!(
            fixture.has_use(path, &pointer, SHADER, "payload", "conversion")?,
            "the cached shader retains its original physical input"
        );
    }
    Ok(())
}

#[test]
fn encrypted_container_ancestry_survives_plain_material_and_shader_children() -> TestResult {
    let nested = ArchiveBuilder::new()
        .entry(support::material(MTL, SOURCE, Storage::Plain))
        .entry(support::shader(Storage::Plain))
        .build();
    let fixture = Fixture::new([TestEntry::zstd("outer.socpak", nested).encrypted()])?;
    let path = r"Data\source.mtl";
    assert_eq!(
        fixture
            .record(path)?
            .1
            .pointer("/source/identity/containers")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(1),
        "the actual nested material retains one original containing Entry"
    );
    for pointer in [
        "/data/payload/path",
        "/data/payload/materials/0/name",
        "/data/payload/shader_references/1/reference/original/value",
        "/data/payload/shader_references/1/input/digest/hex",
    ] {
        fixture.marker(path, pointer, Some(true))?;
    }
    fixture.independent(path, "/data/payload/materials/0/interpreted_surface/tinted")
}

#[test]
fn missing_shader_resolution_retains_the_original_unknown_root_inventory() -> TestResult {
    let fixture = Fixture::new([support::material(MTL, SOURCE, Storage::Plain)])?;
    let pointer = format!("{LINKS}/0/reference/resolution/status");
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer(&pointer)
            .and_then(Value::as_str),
        Some("missing")
    );
    fixture.marker(MTL, &pointer, None)?;
    assert!(
        fixture.has_root_inventory(MTL, &pointer)?,
        "absence is proved by the original root inventory, whose encryption is honestly unknown"
    );
    fixture.marker(
        MTL,
        &format!("{LINKS}/0/reference/original/value"),
        Some(false),
    )?;
    assert!(
        fixture.has_use(MTL, &pointer, MTL, "payload", "selector")?,
        "the actual missing shader name remains the authored selector"
    );
    Ok(())
}
