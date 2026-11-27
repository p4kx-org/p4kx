// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{Index, MediaRecord};
use p4kx_testkit::{ArchiveBuilder, SplitDds, TestEntry, TestTextureFormat};
use serde_json::{Value, json};

#[test]
fn media_cli_texture_mip_is_explicit_and_writes_the_selected_image()
-> Result<(), Box<dyn std::error::Error>> {
    let folder = tempfile::tempdir()?;
    let source = folder.path().join("Data.p4k");
    let dds = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 4);
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/synthetic.dds", dds.header_bytes()))
        .write_to(&source)?;
    for convert in [false, true] {
        let output = folder
            .path()
            .join(if convert { "converted" } else { "metadata" });
        let mut command = p4kx_testkit::p4kx()?;
        command
            .args(["media", "export"])
            .arg(&source)
            .arg("--out")
            .arg(&output)
            .args(["--kind", "texture"]);
        if convert {
            command.args(["--texture-mip", "1"]);
        }
        command.assert().code(0);
        let index: Index = serde_json::from_slice(&fs::read(output.join("media-index.json"))?)?;
        let base = output.join(&index.artifact_base);
        let rows = fs::read_to_string(base.join("texture.jsonl"))?;
        let records = rows
            .lines()
            .map(serde_json::from_str)
            .collect::<Result<Vec<MediaRecord>, _>>()?;
        let image = records
            .iter()
            .find(|row| row.subresource.namespace == "texture.dds-image");
        assert_eq!(
            image.is_some(),
            convert,
            "image requires an explicit requested mip"
        );
        if let Some(image) = image {
            assert_eq!(
                image.data.pointer("/payload/level"),
                Some(&json!(1)),
                "actual requested level"
            );
            let path = image
                .data
                .pointer("/payload/artifact/path")
                .and_then(Value::as_str)
                .ok_or("actual artifact path")?;
            let png = fs::read(base.join(path))?;
            assert_eq!(
                png.get(16..24),
                Some([0, 0, 0, 4, 0, 0, 0, 2].as_slice()),
                "PNG dimensions of mip 1"
            );
        }
    }
    Ok(())
}
