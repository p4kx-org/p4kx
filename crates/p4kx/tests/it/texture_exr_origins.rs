// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write as _};
use std::path::Path;

use p4kx_extraction::{CubemapLayout, OriginDigestWriter, Texture};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat, test_key};
use serde_json::Value;

use super::{TestResult, p4kx};

#[test]
fn exr_origins_distinguish_encrypted_mip_bytes_from_the_header() -> TestResult {
    const ENTRY: &str = "Data/sky.dds";

    let dir = tempfile::tempdir()?;
    let format = TestTextureFormat::Dx10Block {
        dxgi_format: 95,
        block_bytes: 16,
    };
    let dds = SplitDds::new(format, 8, 8)
        .mips(2)
        .split(2)
        .alpha(TestTextureFormat::R8);
    for encrypted_header in [false, true] {
        let archive = dir.path().join(format!("hdr-{encrypted_header}.p4k"));
        dds.entries(ENTRY)
            .into_iter()
            .map(|entry| {
                if entry.path() == ENTRY && !encrypted_header {
                    entry
                } else {
                    entry.encrypted()
                }
            })
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&archive)?;
        let out = dir.path().join(format!("hdr-{encrypted_header}.exr"));
        let output = p4kx()?
            .args(["--json", "--key", &test_key().to_hex(), "texture", "png"])
            .arg(&archive)
            .arg(ENTRY)
            .arg("--out")
            .arg(&out)
            .output()?;
        assert!(output.status.success(), "automatic EXR export: {output:?}");
        let report: Value = serde_json::from_slice(&output.stdout)?;
        check_exr_width_origin(&report, Some(encrypted_header))?;
        check_exr_receipt(&out, Some(true), Some("Data/sky.dds.2"))?;
    }
    Ok(())
}

pub(super) fn check_exr_width_origin(report: &Value, encrypted: Option<bool>) -> TestResult {
    let origin = report
        .pointer("/field_origins/~1width")
        .ok_or("EXR width origin")?;
    let expected = serde_json::to_value(encrypted)?;
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&expected),
        "width only uses original header dimensions"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "width source is complete"
    );
    let references = origin
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("width source uses")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("report uses")?;
    assert!(
        uses.iter().any(|usage| {
            usage.get("id").is_some_and(|id| references.contains(id))
                && usage.pointer("/location/namespace").and_then(Value::as_str) == Some("ddsHeader")
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("width")
        }),
        "the width resolves to the original DDS header word"
    );
    Ok(())
}

pub(super) fn check_exr_receipt(
    out: &Path,
    encrypted: Option<bool>,
    part: Option<&str>,
) -> TestResult {
    let companion = out.with_extension("exr.p4kx-origin.jsonl");
    let rows = fs::read_to_string(companion)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    check_exr_binding(out, &rows)?;
    let opaque = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .ok_or("opaque EXR image")?;
    let expected = serde_json::to_value(encrypted)?;
    assert_eq!(
        opaque.get("containsEncryptedSource"),
        Some(&expected),
        "EXR image encryption"
    );
    assert_eq!(
        opaque.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "all EXR inputs are observed"
    );
    let references = opaque
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("EXR image uses")?;
    let mut payloads = rows
        .iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|usage| {
            usage.pointer("/location/namespace").and_then(Value::as_str) == Some("ddsMipPayload")
        });
    let payload = payloads.next().ok_or("the original color payload")?;
    assert!(
        payloads.next().is_none(),
        "exactly one original color payload"
    );
    assert!(
        references.contains(payload.get("id").ok_or("the original payload id")?),
        "the selected original mip supplied the pixels"
    );
    if let Some(part) = part {
        let payload_source = payload
            .get("source")
            .ok_or("the original payload source id")?;
        let source = rows
            .iter()
            .filter_map(|row| row.get("source"))
            .find(|source| source.get("id") == Some(payload_source))
            .ok_or("original split DDS part")?;
        assert_eq!(
            source.pointer("/path/value").and_then(Value::as_str),
            Some(part),
            "only the exact selected part supplied pixels"
        );
    }
    Ok(())
}

fn check_exr_binding(out: &Path, rows: &[Value]) -> TestResult {
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&fs::read(out)?)?;
    let digest = serde_json::to_value(measured.finish()?)?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("EXR artifact binding")?;
    assert_eq!(
        binding.get("sha256"),
        Some(digest.get("sha256").ok_or("measured EXR checksum")?),
        "bind exact EXR bytes"
    );
    assert_eq!(
        binding.get("byteLength"),
        Some(digest.get("byteLength").ok_or("measured EXR length")?),
        "bind exact EXR length"
    );
    Ok(())
}

pub(super) fn check_file_format_origin(report: &Value, field: &str) -> TestResult {
    let references = report
        .get("field_origins")
        .and_then(|fields| fields.get(field))
        .and_then(|origin| origin.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or("EXR file source references")?;
    let uses = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("EXR source uses")?;
    assert!(
        uses.iter().any(|usage| {
            usage.get("id").is_some_and(|id| references.contains(id))
                && usage.pointer("/location/namespace").and_then(Value::as_str) == Some("ddsHeader")
                && usage.pointer("/location/field").and_then(Value::as_str)
                    == Some("dx10.dxgiFormat")
        }),
        "the DDS format supplies the OpenEXR extension of {field}"
    );
    Ok(())
}

#[test]
fn cubemap_array_receipts_follow_the_exact_face_groups() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = SplitDds::new(
        TestTextureFormat::Dx10Block {
            dxgi_format: 95,
            block_bytes: 16,
        },
        4,
        4,
    )
    .cubemap()
    .array(2)
    .mips(1)
    .write_files(dir.path(), "sky.dds")?;
    let texture = Texture::from_file(&header)?;
    let faces = verify_layout(&texture, dir.path(), CubemapLayout::Faces)?;
    let crosses = verify_layout(&texture, dir.path(), CubemapLayout::Cross)?;
    assert_eq!(faces.len(), 12, "two cubes with six faces each");
    assert_eq!(crosses.len(), 2, "one cross per cube");
    for (group, cross) in faces.as_chunks::<6>().0.iter().zip(crosses) {
        assert!(
            group.iter().all(|face| face.len() == 1),
            "one source span per cube face"
        );
        let group: BTreeSet<_> = group.iter().flatten().cloned().collect();
        assert_eq!(group.len(), 6, "six distinct source spans");
        assert_eq!(
            group, cross,
            "only the corresponding cube supplies each cross"
        );
    }
    Ok(())
}

fn verify_layout(
    texture: &Texture<'_>,
    directory: &Path,
    layout: CubemapLayout,
) -> Result<Vec<BTreeSet<String>>, Box<dyn std::error::Error>> {
    let plain = texture.exr_to_file_as(&directory.join("plain.exr"), 0, layout)?;
    let (with_receipts, origins) =
        texture.exr_to_file_with_origins(&directory.join("sourced.exr"), 0, layout)?;
    assert_eq!(
        plain.files.len(),
        with_receipts.files.len(),
        "the output layout is unchanged"
    );
    assert_eq!(
        with_receipts.files.len(),
        origins.receipts.len(),
        "one receipt per image"
    );
    plain
        .files
        .iter()
        .zip(&with_receipts.files)
        .zip(&origins.receipts)
        .map(|((plain, with_receipt), origin)| {
            assert_eq!(
                fs::read(plain)?,
                fs::read(with_receipt)?,
                "image encoding is unchanged"
            );
            mip_spans(origin)
        })
        .collect()
}

fn mip_spans(path: &Path) -> Result<BTreeSet<String>, Box<dyn std::error::Error>> {
    let rows = fs::read_to_string(path)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let references = rows
        .iter()
        .find(|row| row.pointer("/address/type").and_then(Value::as_str) == Some("opaquePayload"))
        .and_then(|row| row.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or("opaque EXR source references")?;
    rows.iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|usage| usage.get("id").is_some_and(|id| references.contains(id)))
        .filter(|usage| {
            usage.pointer("/location/namespace").and_then(Value::as_str) == Some("ddsMipPayload")
        })
        .map(|usage| {
            Ok(usage
                .pointer("/location/indices/1")
                .and_then(Value::as_str)
                .ok_or("EXR source mip offset")?
                .to_owned())
        })
        .collect()
}
