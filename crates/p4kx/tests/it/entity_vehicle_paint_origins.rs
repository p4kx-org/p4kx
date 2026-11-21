// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::check_glb;
use serde_json::Value;

use super::{PaintFixture, TestResult, manifest};

const APPEARANCE: &str = "/asset/extras/p4kxPaintAppearance";

pub(super) fn check_export(fixture: &PaintFixture, out: &Path, package: &Path) -> TestResult {
    let bytes = fs::read(out)?;
    let appearance = glb_appearance(fixture, &bytes)?;
    let manifest = manifest(package)?;
    assert_eq!(
        text(&manifest, "version"),
        Some("1.22"),
        "the package version identifies the added paint provenance"
    );
    let proof = manifest
        .get("paint_provenance")
        .ok_or("package paint proof")?;
    assert_eq!(
        proof.get("appearance"),
        Some(&appearance),
        "both exports keep the same fitted paint values"
    );

    let companion = out.with_extension("glb.p4kx-origin.jsonl");
    let rows: Vec<Value> = fs::read_to_string(companion)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    check_binding(out, &bytes, &rows)?;
    check_fields(&appearance, proof, &rows)?;
    check_selection_sources(fixture, &rows)?;
    check_palette_sources(fixture, &rows)?;
    check_material_sources(fixture, &rows)
}

fn glb_appearance(fixture: &PaintFixture, bytes: &[u8]) -> TestResult<Value> {
    let glb = check_glb(bytes)?;
    let extras = glb
        .document
        .as_json()
        .asset
        .extras
        .as_deref()
        .ok_or("fitted paint extras")?;
    let mut extras: Value = serde_json::from_str(extras.get())?;
    let appearance = extras
        .as_object_mut()
        .and_then(|extras| extras.remove("p4kxPaintAppearance"))
        .ok_or("fitted paint values")?;
    assert_eq!(
        text(&appearance, "material").map(|path| path.replace('\\', "/")),
        Some(fixture.library.to_owned()),
        "the selected library is the physical archive path"
    );
    let kind = if fixture.selector.contains(".materialVariants[") {
        "materialVariant"
    } else {
        "subGeometry"
    };
    assert_eq!(
        text(&appearance, "variantKind"),
        Some(kind),
        "the root paint came from the fixture's original variant family"
    );
    Ok(appearance)
}

fn check_binding(out: &Path, bytes: &[u8], rows: &[Value]) -> TestResult {
    let header = rows.first().ok_or("binding row")?;
    assert_eq!(
        header
            .pointer("/binding/path/value")
            .and_then(Value::as_str),
        out.file_name().and_then(|name| name.to_str()),
        "the companion binds the written GLB filename"
    );
    let mut digest = OriginDigestWriter::new(io::sink());
    digest.write_all(bytes)?;
    let digest = serde_json::to_value(digest.finish()?)?;
    assert_eq!(
        header.pointer("/binding/sha256"),
        digest.get("sha256"),
        "the companion binds the exact committed bytes"
    );
    assert_eq!(
        rows.last().and_then(|row| text(row, "type")),
        Some("end"),
        "the companion has a terminal record"
    );
    Ok(())
}

fn check_fields(appearance: &Value, proof: &Value, rows: &[Value]) -> TestResult {
    let fields = appearance.as_object().ok_or("appearance identities")?;
    let package_fields = proof.get("fieldOrigins").ok_or("package source fields")?;
    for key in fields.keys() {
        let glb = companion_field(rows, key)?;
        let package = package_fields
            .get(format!("/{key}"))
            .ok_or("package source refs")?;
        for field in [
            "containsEncryptedSource",
            "sourceSetComplete",
            "sourceUsesRefs",
        ] {
            assert_eq!(
                glb.get(field),
                package.get(field),
                "same {key} observation in both outputs"
            );
        }
        uses_of(glb, rows)?;
    }
    Ok(())
}

fn check_selection_sources(fixture: &PaintFixture, rows: &[Value]) -> TestResult {
    let paint = companion_field(rows, "paintGuid")?;
    assert!(
        uses_of(paint, rows)?.iter().any(|usage| record_header(
            usage,
            fixture.paint_record,
            "guid"
        )),
        "the fitted paint identity comes from its actual Record header"
    );
    let variant = companion_field(rows, "variantKind")?;
    assert_eq!(
        variant.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "other variant decision operands remain unobserved"
    );
    let uses = uses_of(variant, rows)?;
    assert!(
        uses.iter().any(|usage| dataforge(
            usage,
            "selector",
            fixture.hull_record,
            fixture.variant_tags
        )),
        "the winning variant's actual original Tags field made the match"
    );
    assert!(
        uses.iter().any(|usage| dataforge(
            usage,
            "selector",
            fixture.paint_record,
            "Components[0].AttachDef.Tags"
        )),
        "the fitted item's authored Tags field supplied the appearance token"
    );
    Ok(())
}

fn check_palette_sources(fixture: &PaintFixture, rows: &[Value]) -> TestResult {
    let palette = companion_field(rows, "paletteGuid")?;
    assert_eq!(
        palette
            .get("containsEncryptedSource")
            .and_then(Value::as_bool),
        Some(true),
        "the palette identity uses an encrypted DataForge record"
    );
    assert_eq!(
        palette.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "the named palette retains the observed subset of the enclosing decision"
    );
    let uses = uses_of(palette, rows)?;
    assert!(
        uses.iter().any(|usage| dataforge(
            usage,
            "selector",
            fixture.hull_record,
            fixture.selector
        )),
        "the selected hull slot contributed its named palette reference"
    );
    assert!(
        uses.iter()
            .any(|usage| record_header(usage, fixture.palette_record, "guid")),
        "the palette identity comes from its actual Record header"
    );
    Ok(())
}

fn check_material_sources(fixture: &PaintFixture, rows: &[Value]) -> TestResult {
    let material = companion_field(rows, "material")?;
    assert_eq!(
        material.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "the exact library inputs do not close the entire paint selection"
    );
    let uses = uses_of(material, rows)?;
    let field = fixture
        .selector
        .replace(".Palette.RootRecord", ".Material.path");
    assert!(
        uses.iter()
            .any(|usage| dataforge(usage, "selector", fixture.hull_record, &field)),
        "the original hull field selected the loaded material"
    );
    let entry = rows
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|entry| {
            entry
                .pointer("/path/value")
                .and_then(Value::as_str)
                .is_some_and(|path| path.replace('\\', "/") == fixture.library)
        })
        .ok_or("the selected material archive entry")?;
    let id = entry.get("id").ok_or("the material archive entry ID")?;
    assert!(
        uses.iter().any(|usage| usage.get("source") == Some(id)
            && text(usage, "scope") == Some("archiveDirectoryMetadata")
            && text(usage, "role") == Some("payload")
            && usage.pointer("/location/type").and_then(Value::as_str) == Some("metadata")
            && usage.pointer("/location/field").and_then(Value::as_str) == Some("path")),
        "the selected library path comes from the archive directory field"
    );
    assert!(
        uses.iter().any(|usage| usage.get("source") == Some(id)
            && text(usage, "scope") == Some("payload")
            && text(usage, "role") == Some("selector")
            && usage.pointer("/location/type").and_then(Value::as_str) == Some("opaque")),
        "the parsed material entry separately gates the retained path"
    );
    Ok(())
}

fn companion_field<'a>(rows: &'a [Value], key: &str) -> TestResult<&'a Value> {
    let path = format!("{APPEARANCE}/{key}");
    rows.iter()
        .find(|row| {
            row.pointer("/address/type").and_then(Value::as_str) == Some("glbJsonField")
                && row.pointer("/address/path").and_then(Value::as_str) == Some(path.as_str())
        })
        .ok_or_else(|| format!("missing emitted field {path}").into())
}

fn uses_of<'a>(field: &Value, rows: &'a [Value]) -> TestResult<Vec<&'a Value>> {
    let refs = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source use references")?;
    refs.iter()
        .map(|id| {
            rows.iter()
                .filter_map(|row| row.get("sourceUse"))
                .find(|usage| usage.get("id") == Some(id))
                .ok_or_else(|| format!("missing source use {id}").into())
        })
        .collect()
}

fn record_header(usage: &Value, record: u64, field: &str) -> bool {
    let record = record.to_string();
    text(usage, "role") == Some("payload")
        && usage.pointer("/location/type").and_then(Value::as_str) == Some("subresource")
        && usage.pointer("/location/namespace").and_then(Value::as_str)
            == Some("dataforgeRecordHeader")
        && usage.pointer("/location/indices/0").and_then(Value::as_str) == Some(record.as_str())
        && usage.pointer("/location/indices/1").is_none()
        && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
}

fn dataforge(usage: &Value, role: &str, record: u64, field: &str) -> bool {
    text(usage, "role") == Some(role)
        && usage
            .pointer("/location/recordIndex")
            .and_then(Value::as_u64)
            == Some(record)
        && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
}

fn text<'a>(object: &'a Value, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}
