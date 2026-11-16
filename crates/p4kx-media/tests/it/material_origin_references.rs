// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_extraction::{ConflictPolicy, Filter};
use p4kx_media::ExportOutcome;
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
        MaterialSourceLimits::default(),
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
fn shader_links_use_the_original_submaterial_list_even_when_the_tree_is_partial() -> TestResult {
    let padding = "a".repeat(4096);
    let xml = format!(
        "<Material><SubMaterials Padding='{padding}'>\
         <Placeholder/><Material Shader='Actual' StringGenMask='%A'/>\
         </SubMaterials><SubMaterials>\
         <Placeholder/><Material Shader='Decoy' StringGenMask='%B'/>\
         </SubMaterials></Material>"
    );
    let material = || support::material(MTL, xml.as_bytes(), Storage::Plain);
    let complete = Fixture::new([material()])?;
    let (_, record) = complete.record(MTL)?;
    let links = record
        .pointer(LINKS)
        .and_then(Value::as_array)
        .ok_or("complete shader links")?;
    assert_eq!(
        links.len(),
        2,
        "the selected list contributes one shader and flag"
    );
    for (link, token) in links.iter().zip(["Actual", "%A"]) {
        assert_eq!(
            link.get("material").and_then(Value::as_u64),
            Some(1),
            "the unknown slot keeps its original position"
        );
        assert_eq!(
            link.pointer("/reference/original/value")
                .and_then(Value::as_str),
            Some(token),
            "shader links follow the selected original list"
        );
    }

    let mut limits = MaterialSourceLimits::default();
    limits.text_bytes = 2048;
    let partial = Fixture::with([material()], support::options(), limits)?;
    let (_, record) = partial.record(MTL)?;
    assert_partial_submaterials(record);
    Ok(())
}

fn assert_partial_submaterials(record: &Value) {
    assert_eq!(
        record
            .pointer("/data/payload/authored/losses/0/kind")
            .and_then(Value::as_str),
        Some("text-bytes"),
        "the selected list exceeds the capture budget"
    );
    assert_eq!(
        record
            .pointer("/data/payload/authored/root/children/0/syntax/ordinal")
            .and_then(Value::as_u64),
        Some(1),
        "the later list remains in the partial authored tree"
    );
    assert_eq!(
        record
            .pointer("/data/payload/materials/1/interpreted_surface/shader")
            .and_then(Value::as_str),
        Some("Actual"),
        "the typed parser still selects the original list"
    );
    assert!(
        record
            .pointer(LINKS)
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty),
        "the later list cannot supply shader links for the omitted original"
    );
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

#[test]
fn shader_links_share_the_exact_inventory_without_claiming_unread_payloads() -> TestResult {
    let unresolved = "Data/unresolved.mtl";
    let fixture = Fixture::new([
        support::material(MTL, SOURCE, Storage::Plain),
        support::material(
            unresolved,
            b"<Material Shader='shadow' StringGenMask='%A'/>",
            Storage::Plain,
        ),
        support::shader(Storage::Plain),
        shader_container("plain.socpak", Storage::Plain),
        shader_container("encrypted.socpak", Storage::Encrypted),
    ])?;
    let set_rows = support::rows(fixture.base.join("origin-use-sets.jsonl"))?;
    let [header, .., eof] = set_rows.as_slice() else {
        return Err("one shared shader directory inventory".into());
    };
    let id = header.get("id").ok_or("shared shader inventory ID")?;
    assert_eq!(header["members"], "3", "root and both inspected containers");
    assert_eq!(eof["sets"], "1", "all shader links use the same inventory");

    let property = format!("{LINKS}/1/reference/resolution/target/subresource/indices/0");
    let ambiguity = format!("{LINKS}/0/reference/resolution/status");
    for (source, pointer) in [(MTL, &property), (unresolved, &ambiguity)] {
        check_shared_shader_inventory(&fixture, source, pointer, id)?;
    }
    assert!(
        fixture.has_use(MTL, &property, SHADER, "payload", "conversion")?,
        "the selected Property uses the decoded definition"
    );
    check_ambiguous_shader(&fixture, unresolved, &ambiguity)?;

    let mut options = fixture.options.clone();
    options.conflict = ConflictPolicy::Skip;
    let registry = support::registry(MaterialSourceLimits::default())?;
    assert!(
        matches!(
            p4kx_media::export_with(&fixture.archive, &fixture.output, &options, &registry)?,
            ExportOutcome::Skipped { .. }
        ),
        "the generation reader validates pooled membership and immediate markers"
    );
    Ok(())
}

fn check_ambiguous_shader(fixture: &Fixture, source: &str, pointer: &str) -> TestResult {
    assert_eq!(
        fixture
            .record(source)?
            .1
            .pointer(pointer)
            .and_then(Value::as_str),
        Some("ambiguous"),
        "same-name nested entries are distinct candidates"
    );
    let shader = r"Data\shadow.ext";
    let candidates: BTreeSet<&str> = fixture
        .contributions(source, pointer)?
        .into_iter()
        .filter(|used| {
            fixture.input_path(used) == Some(shader)
                && used["scope"] == "archiveDirectoryMetadata"
                && used["role"] == "selector"
        })
        .map(|used| used["source"].as_str().ok_or("physical candidate source"))
        .collect::<Result<_, _>>()?;
    assert_eq!(candidates.len(), 2, "both physical selectors are retained");
    fixture.no_payload(source, pointer, shader)
}

fn shader_container(path: &str, storage: Storage) -> TestEntry {
    let bytes = ArchiveBuilder::new()
        .entry(support::material(
            "Data/shadow.ext",
            support::DEFINITION,
            Storage::Plain,
        ))
        .build();
    support::material(path, &bytes, storage)
}

fn check_shared_shader_inventory(
    fixture: &Fixture,
    source: &str,
    pointer: &str,
    expected_set: &Value,
) -> TestResult {
    let receipt = fixture.field(source, pointer)?;
    assert_eq!(
        receipt.get("sourceUseSetRef"),
        Some(expected_set),
        "the field shares the original inventory plus its own local uses"
    );
    fixture.marker(source, pointer, Some(true))?;
    assert_eq!(
        receipt["sourceSetComplete"], true,
        "the full proof is retained"
    );

    let mut directories = BTreeSet::new();
    for used in fixture.contributions(source, pointer)? {
        assert!(
            matches!(
                used["role"].as_str(),
                Some("payload" | "selector" | "conversion")
            ),
            "references and attempted reads cannot contribute"
        );
        if used.pointer("/location/field").and_then(Value::as_str) != Some("matching-inventory") {
            continue;
        }
        assert_eq!(used["role"], "selector", "the original directory use role");
        assert_eq!(used["scope"], "payload", "the enumerated directory is used");
        assert!(
            directories.insert((
                fixture.input_path(used),
                used["effectiveEncryption"].as_str()
            )),
            "each physical directory contributes exactly once"
        );
    }
    assert_eq!(
        directories,
        BTreeSet::from([
            (None, Some("unknown")),
            (Some("plain.socpak"), Some("unencrypted")),
            (Some("encrypted.socpak"), Some("encrypted")),
        ]),
        "expanded membership has the root and each nested directory with its origin"
    );
    Ok(())
}
