// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_testkit::{ArchiveBuilder, SplitDds, TestTextureFormat};
use serde_json::Value;

use super::TestResult;

#[test]
fn converted_array_images_bind_their_own_colour_and_alpha_bytes() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let out = dir.path().join("out");
    let format = TestTextureFormat::Dx10Pixel {
        dxgi_format: 28,
        pixel_bytes: 4,
    };
    let dds = SplitDds::new(format, 8, 8)
        .array(2)
        .split(2)
        .alpha(TestTextureFormat::R8);
    dds.entries("Data/glass.dds")
        .into_iter()
        .map(|entry| match entry.path() {
            "Data/glass.dds.1" | "Data/glass.dds.2a" => entry.encrypted(),
            _ => entry,
        })
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&archive)?;
    let output = p4kx_testkit::p4kx()?
        .args(["--json", "extract"])
        .arg(&archive)
        .args(["--filter", "Data/", "--convert", "--out"])
        .arg(&out)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report: Value = serde_json::from_slice(&output.stdout)?;
    let textures = report["texture_artifacts"]
        .as_array()
        .ok_or("a receipt for every converted texture")?;
    let [texture] = textures.as_slice() else {
        return Err("one converted texture".into());
    };
    check_texture_fields(texture);
    let images = texture["images"].as_array().ok_or("converted images")?;
    assert_eq!(images.len(), 2, "a receipt for each array layer");
    let first = check_image_receipt(&out, texture, &images[0], "Data/glass_layer0.png")?;
    let second = check_image_receipt(&out, texture, &images[1], "Data/glass_layer1.png")?;
    assert_eq!(first, [0, 0], "first layer offsets in color and alpha");
    assert_eq!(second, [256, 64], "second layer offsets in color and alpha");
    Ok(())
}

fn check_texture_fields(texture: &Value) {
    assert_eq!(
        texture["source_transfer"], "linear",
        "explicit DXGI transfer"
    );
    assert_eq!(
        texture["output_transfer"], "linear",
        "unchanged colour encoding"
    );
    assert_eq!(
        texture["png_transform"], "preserve_colour_transfer",
        "the alpha merge leaves RGB transfer unchanged"
    );
    assert_eq!(
        texture["alpha_association"], "unknown",
        "the original colour header makes no association claim"
    );
    for field in [
        "/source_transfer",
        "/output_transfer",
        "/png_transform",
        "/alpha_association",
    ] {
        assert_eq!(
            texture["field_origins"][field]["containsEncryptedSource"], false,
            "the plain colour header owns {field} without the private alpha tail"
        );
    }
}

fn check_image_receipt(
    out: &Path,
    texture: &Value,
    image: &Value,
    path: &str,
) -> Result<Vec<u64>, Box<dyn Error>> {
    let bytes = fs::read(out.join(path))?;
    let png = png::Decoder::new(std::io::Cursor::new(&bytes)).read_info()?;
    assert_eq!(
        png.info().gama_chunk.map(png::ScaledFloat::into_value),
        Some(1.0),
        "the written array layer carries linear PNG metadata"
    );
    let mut digest = p4kx_extraction::OriginDigestWriter::new(std::io::sink());
    std::io::Write::write_all(&mut digest, &bytes)?;
    let digest = digest.finish()?;
    assert_eq!(image["path"], path, "target-relative receipt path");
    assert_eq!(
        image["digest"],
        serde_json::to_value(digest)?,
        "digest binds the written bytes"
    );
    assert_eq!(image["sourceSetComplete"], true, "contributors are known");
    assert_eq!(
        image["containsEncryptedSource"], true,
        "encrypted selected alpha contributes"
    );

    let provenance = &texture["provenance"];
    let refs = image["sourceUsesRefs"].as_array().ok_or("source uses")?;
    let uses = provenance["uses"].as_array().ok_or("uses inventory")?;
    let sources = provenance["sources"]
        .as_array()
        .ok_or("sources inventory")?;
    let mut offsets = Vec::new();
    for reference in refs {
        let usage = uses
            .iter()
            .find(|entry| &entry["id"] == reference)
            .ok_or("use")?;
        if usage.pointer("/location/namespace").and_then(Value::as_str) != Some("ddsMipPayload") {
            continue;
        }
        let source = sources
            .iter()
            .find(|entry| entry["id"] == usage["source"])
            .ok_or("source")?;
        let source_path = source
            .pointer("/path/value")
            .and_then(Value::as_str)
            .ok_or("source path")?;
        assert!(
            matches!(source_path, "Data/glass.dds.2" | "Data/glass.dds.2a"),
            "{source_path}"
        );
        offsets.push(
            usage
                .pointer("/location/indices/1")
                .and_then(Value::as_str)
                .ok_or("byte offset")?
                .parse::<u64>()?,
        );
    }
    assert_eq!(offsets.len(), 2, "one color and one alpha byte span");
    offsets.sort_unstable_by(|left, right| right.cmp(left));
    Ok(offsets)
}
