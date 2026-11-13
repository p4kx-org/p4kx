// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, BufReader, Cursor, Write as _};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{SplitDds, TestTextureFormat};
use serde_json::Value;

use super::{TestResult, p4kx, read_rgba};

const MARKER: usize = 124;
const FLAGS: usize = 36;
const PIXELS: [u8; 8] = [16, 128, 240, 61, 80, 40, 130, 203];

#[test]
fn source_transfer_tags_preserve_samples_and_decline_conflicts() -> TestResult {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("texture.png");
    for (format, cry, expected, gamma) in [
        (29, None, "srgb", None),
        (28, None, "linear", Some(1.0)),
        (28, Some(true), "unknown", None),
        (27, None, "unknown", None),
    ] {
        let header = fixture(dir.path(), format, cry)?;
        let report = export(&header, &out)?;
        assert_eq!(
            report.get("output_transfer").and_then(Value::as_str),
            Some(expected),
            "effective output transfer"
        );
        let reader = png::Decoder::new(BufReader::new(fs::File::open(&out)?)).read_info()?;
        assert_eq!(
            reader.info().srgb.is_some(),
            expected == "srgb",
            "sRGB chunk at {format}"
        );
        assert_eq!(
            reader.info().gama_chunk.map(png::ScaledFloat::into_value),
            gamma,
            "linear gamma chunk at {format}"
        );
        assert_eq!(
            read_rgba(&out)?.2,
            PIXELS,
            "all RGB and alpha bytes stay unchanged"
        );
    }
    Ok(())
}

#[test]
fn transfer_report_and_companion_use_exact_original_header_words() -> TestResult {
    let dir = tempfile::tempdir()?;
    let out = dir.path().join("texture.png");
    let header = fixture(dir.path(), 29, Some(true))?;
    let report = export(&header, &out)?;
    let receipt = dir.path().join("texture.png.p4kx-origin.jsonl");
    assert_eq!(
        report["origin_receipts"][0],
        "texture.png.p4kx-origin.jsonl"
    );
    let source_uses = report["provenance"]["uses"]
        .as_array()
        .ok_or("source uses")?;
    let transfer = &report["field_origins"]["/output_transfer"];
    assert_eq!(transfer["containsEncryptedSource"], Value::Null);
    let references = transfer["sourceUsesRefs"]
        .as_array()
        .ok_or("transfer references")?;
    for (name, offset) in [
        ("reserved1", "36"),
        ("reserved2", "124"),
        ("dx10.dxgiFormat", "128"),
    ] {
        assert!(
            source_uses.iter().any(|source| {
                source["location"]["field"] == name
                    && source["location"]["indices"][0] == offset
                    && references.contains(&source["id"])
            }),
            "original {name} at {offset}"
        );
    }

    let rows = fs::read_to_string(receipt)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&fs::read(out)?)?;
    let expected = serde_json::to_value(measured.finish()?)?;
    let binding = &rows.first().ok_or("binding")?["binding"];
    assert_eq!(binding["sha256"], expected["sha256"]);
    assert_eq!(binding["byteLength"], expected["byteLength"]);
    let opaque = rows
        .iter()
        .find(|row| row["type"] == "field" && row["address"]["type"] == "opaquePayload")
        .ok_or("opaque PNG field")?;
    assert_eq!(opaque["containsEncryptedSource"], Value::Null);
    assert_eq!(opaque["sourceSetComplete"], true);
    Ok(())
}

#[test]
fn preview_uses_the_same_transfer_policy_and_byte_samples() -> TestResult {
    let dir = tempfile::tempdir()?;
    let header = fixture(dir.path(), 28, Some(false))?;
    let texture = p4kx_extraction::Texture::from_file(&header)?;
    let preview = texture.preview_png(0, None)?;
    let mut reader = png::Decoder::new(Cursor::new(&preview.png)).read_info()?;
    assert_eq!(
        reader.info().gama_chunk.map(png::ScaledFloat::into_value),
        Some(1.0),
        "linear output metadata"
    );
    let mut decoded = [0; PIXELS.len()];
    reader.next_frame(&mut decoded)?;
    assert_eq!(decoded, PIXELS, "preview uses the same unchanged samples");
    Ok(())
}

fn fixture(
    path: &Path,
    format: u32,
    cry: Option<bool>,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let dds = SplitDds::new(
        TestTextureFormat::Dx10Pixel {
            dxgi_format: format,
            pixel_bytes: 4,
        },
        2,
        1,
    )
    .mips(1);
    let mut bytes = dds.header_bytes();
    bytes.truncate(dds.header_len());
    if let Some(srgb) = cry {
        bytes
            .get_mut(MARKER..MARKER + 4)
            .ok_or("marker word")?
            .copy_from_slice(b"FYRC");
        let flags = if srgb { 0x800_u32 } else { 0 };
        bytes
            .get_mut(FLAGS..FLAGS + 4)
            .ok_or("flag word")?
            .copy_from_slice(&flags.to_le_bytes());
    }
    bytes.extend_from_slice(&PIXELS);
    let header = path.join("texture.dds");
    fs::write(&header, bytes)?;
    Ok(header)
}

fn export(input: &Path, output: &Path) -> Result<Value, Box<dyn std::error::Error>> {
    let result = p4kx()?
        .args(["--json", "texture", "png"])
        .arg(input)
        .arg("--out")
        .arg(output)
        .output()?;
    assert!(result.status.success(), "fixture export: {result:?}");
    Ok(serde_json::from_slice(&result.stdout)?)
}
