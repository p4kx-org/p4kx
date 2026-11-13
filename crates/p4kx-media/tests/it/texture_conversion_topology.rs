// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{SplitDds, TestEntry, TestTextureFormat};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use crate::support;
use crate::support::audio_provider::TestResult;

const HDR: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 2,
    pixel_bytes: 16,
};
const INTEGER: TestTextureFormat = TestTextureFormat::Dx10Pixel {
    dxgi_format: 62,
    pixel_bytes: 1,
};

#[test]
fn texture_conversion_writes_each_cube_layer_and_volume_slice_with_exact_geometry() -> TestResult {
    let folder = tempfile::tempdir()?;
    let cube = SplitDds::new(TestTextureFormat::R8, 8, 8)
        .array(2)
        .cubemap();
    let volume = SplitDds::new(INTEGER, 8, 4).volume(4);
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("cube.dds", cube.header_bytes()),
            TestEntry::stored("volume.dds", volume.header_bytes()),
        ],
    )?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        texture_mip: Some(1),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    let base = output.join(&index.artifact_base);
    let images: Vec<_> = rows.iter().filter_map(image).collect();
    check_cube_volume(&base, &images)
}

fn check_cube_volume(base: &Path, images: &[&Value]) -> TestResult {
    assert_eq!(
        images.len(),
        14,
        "12 cube faces and two reduced-depth volume slices"
    );
    let faces = [
        "positive-x",
        "negative-x",
        "positive-y",
        "negative-y",
        "positive-z",
        "negative-z",
    ];
    for (index, image) in images.iter().take(12).enumerate() {
        assert_eq!(
            image.get("layer"),
            Some(&json!(index / faces.len())),
            "cube array layer"
        );
        assert_eq!(
            image.pointer("/face/type"),
            faces
                .get(index % faces.len())
                .map(|face| json!(face))
                .as_ref(),
            "cube orientation in storage order"
        );
        png(base, image, 4, 4)?;
    }
    for (slice, image) in images.iter().skip(12).enumerate() {
        assert_eq!(
            image.get("slice"),
            Some(&json!(slice)),
            "mip-local volume slice"
        );
        assert_eq!(
            image.get("source_depth"),
            Some(&json!(4)),
            "full volume depth"
        );
        assert_eq!(
            image.get("output_transfer"),
            Some(&json!("unknown")),
            "integer channels have no RGB transfer"
        );
        assert_eq!(
            image.get("png_transform"),
            Some(&json!("data_channels")),
            "integer channel transform is retained"
        );
        png(base, image, 4, 2)?;
    }
    Ok(())
}

#[test]
fn texture_conversion_keeps_native_float_exr_and_identical_artifact_occurrences() -> TestResult {
    let folder = tempfile::tempdir()?;
    let hdr = hdr_fixture()?;
    let archive = support::archive_with(
        folder.path(),
        [
            TestEntry::stored("first.dds", hdr.clone()),
            TestEntry::stored("second.dds", hdr),
        ],
    )?;
    let output = folder.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::Texture],
        texture_mip: Some(0),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "texture.jsonl")?;
    let images: Vec<_> = rows.iter().filter_map(image).collect();
    let [first, second] = images.as_slice() else {
        return Err("two physical conversion occurrences".into());
    };
    check_float_artifact(&output.join(&index.artifact_base), first, second)
}

fn hdr_fixture() -> TestResult<Vec<u8>> {
    let mut hdr = SplitDds::new(HDR, 2, 2).mips(1).header_bytes();
    let header_bytes = SplitDds::new(HDR, 2, 2).header_len();
    let data = hdr.get_mut(header_bytes..).ok_or("float payload")?;
    for (channel, bytes) in data.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let value = [2.5_f32, -0.5, 4.0, 0.25]
            .get(channel % 4)
            .ok_or("RGBA channel")?;
        bytes.copy_from_slice(&value.to_le_bytes());
    }
    Ok(hdr)
}

fn check_float_artifact(base: &Path, first: &Value, second: &Value) -> TestResult {
    assert_eq!(
        first.get("placement"),
        Some(&json!("written")),
        "first placement writes"
    );
    assert_eq!(
        second.get("placement"),
        Some(&json!("kept-identical")),
        "second retains identical bytes"
    );
    assert_eq!(
        first.get("artifact"),
        second.get("artifact"),
        "stable digest and file"
    );
    assert_eq!(
        first.get("format"),
        Some(&json!("open-exr")),
        "native float format"
    );
    assert_eq!(
        first.get("bit_depth"),
        Some(&json!(32)),
        "EXR float channel width"
    );
    assert_eq!(
        first.get("channel_transform"),
        Some(&json!("linear-float")),
        "no tone-map"
    );
    assert_eq!(
        first.get("alpha"),
        Some(&json!("decoded-channels")),
        "stored float alpha"
    );
    assert_eq!(
        first.get("output_transfer"),
        Some(&json!("linear")),
        "OpenEXR samples are linear"
    );
    assert_eq!(
        first.get("png_transform"),
        None,
        "OpenEXR omits PNG processing"
    );
    let bytes = artifact(base, first)?;
    assert_eq!(
        bytes.get(..4),
        Some([0x76, 0x2f, 0x31, 0x01].as_slice()),
        "OpenEXR magic"
    );
    Ok(())
}

fn image(row: &MediaRecord) -> Option<&Value> {
    (row.subresource.namespace == "texture.dds-image")
        .then(|| row.data.get("payload"))
        .flatten()
}

fn png(base: &Path, image: &Value, width: u32, height: u32) -> TestResult {
    let bytes = artifact(base, image)?;
    assert_eq!(
        bytes.get(16..20),
        Some(width.to_be_bytes().as_slice()),
        "PNG IHDR width"
    );
    assert_eq!(
        bytes.get(20..24),
        Some(height.to_be_bytes().as_slice()),
        "PNG IHDR height"
    );
    assert_eq!(bytes.get(24), Some(&8), "PNG IHDR bits per channel");
    Ok(())
}

fn artifact(base: &Path, image: &Value) -> TestResult<Vec<u8>> {
    let path = image
        .pointer("/artifact/path")
        .and_then(Value::as_str)
        .ok_or("artifact path")?;
    let bytes = fs::read(base.join(path))?;
    let mut digest = String::new();
    for byte in Sha256::digest(&bytes) {
        write!(digest, "{byte:02x}")?;
    }
    assert_eq!(
        image.pointer("/artifact/digest/hex"),
        Some(&json!(digest)),
        "actual encoded digest"
    );
    assert_eq!(
        image.pointer("/artifact/size"),
        Some(&json!(bytes.len())),
        "actual encoded length"
    );
    Ok(bytes)
}
