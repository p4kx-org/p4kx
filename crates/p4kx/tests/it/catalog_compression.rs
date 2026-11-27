// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Read;
use std::path::Path;

use super::{TestResult, catalog, p4kx, write_archive};

#[test]
fn catalog_to_a_zst_name_writes_a_zstd_frame_with_the_bytes_of_the_plain_catalog() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let plain = dir.path().join("plain.json");

    catalog(&archive, &plain, &[])?;
    let plain = fs::read(plain)?;
    let default = read_catalog_frame(&archive, &dir.path().join("default.json.zst"), &[], &plain)?;
    let fast = read_catalog_frame(
        &archive,
        &dir.path().join("fast.json.ZST"),
        &["--zstd-level", "1"],
        &plain,
    )?;
    assert!(
        default != fast,
        "level 1 changes the encoding of the fixture"
    );
    Ok(())
}

fn read_catalog_frame(
    archive: &Path,
    packed: &Path,
    args: &[&str],
    plain: &[u8],
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut flags = vec!["--json"];
    flags.extend_from_slice(args);
    let summary = catalog(archive, packed, &flags)?;
    let frame = fs::read(packed)?;
    assert_eq!(frame.get(..4), Some(&[0x28, 0xb5, 0x2f, 0xfd][..]), "magic");
    assert!(
        frame.get(4).is_some_and(|flags| flags & 0b100 != 0),
        "content checksum flag"
    );
    let content_size =
        zstd::zstd_safe::get_frame_content_size(&frame).map_err(|_| "invalid zstd frame header")?;
    assert_eq!(
        content_size,
        Some(u64::try_from(plain.len())?),
        "frame content size"
    );

    let mut decoded = Vec::new();
    zstd::Decoder::new(frame.as_slice())?.read_to_end(&mut decoded)?;
    assert!(decoded == plain, "byte-identical JSON");
    let json: serde_json::Value = serde_json::from_str(&summary)?;
    assert_eq!(
        json.get("size").and_then(serde_json::Value::as_u64),
        Some(u64::try_from(decoded.len())?),
        "uncompressed size"
    );
    assert_eq!(
        json.get("compressed_size")
            .and_then(serde_json::Value::as_u64),
        Some(u64::try_from(frame.len())?),
        "compressed size"
    );
    assert_eq!(
        json.get("decompressed_limit")
            .and_then(serde_json::Value::as_u64),
        Some(1_000_000_000),
        "transport limit"
    );
    Ok(frame)
}

#[test]
fn catalog_rejects_invalid_zstd_levels_before_opening_the_archive() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("missing.p4k");
    for (name, level, diagnostic) in [
        ("catalog.json.zst", "0", "0 is not in 1..=22"),
        ("catalog.json.zst", "23", "23 is not in 1..=22"),
        (
            "catalog.json",
            "1",
            "requires an --out filename ending in .zst",
        ),
    ] {
        let out = dir.path().join(name);
        fs::write(&out, b"existing catalog")?;

        let output = p4kx(&["catalog", "--provenance", "full"])?
            .arg(&archive)
            .arg("--out")
            .arg(&out)
            .args(["--zstd-level", level])
            .output()?;

        assert_eq!(output.status.code(), Some(2), "invalid input: {output:?}");
        let stderr = String::from_utf8(output.stderr)?;
        assert!(stderr.contains(diagnostic), "compression error: {stderr}");
        assert_eq!(
            fs::read(out)?,
            b"existing catalog",
            "the previous output is preserved"
        );
    }
    Ok(())
}

#[test]
fn catalog_to_a_json_name_stays_plain_json_without_a_compressed_size() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("plain.json");

    let summary = catalog(&archive, &out, &["--json"])?;

    assert_eq!(fs::read(&out)?.first(), Some(&b'{'), "plain JSON");
    let json: serde_json::Value = serde_json::from_str(&summary)?;
    assert_eq!(
        json.get("compressed_size"),
        Some(&serde_json::Value::Null),
        "plain output records no compressed size"
    );
    Ok(())
}

#[test]
fn catalog_to_a_gz_name_is_refused_instead_of_writing_plain_json() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = write_archive(dir.path())?;
    let out = dir.path().join("p4kx-catalog.json.gz");

    let output = p4kx(&["catalog", "--provenance", "full"])?
        .arg(&archive)
        .arg("--out")
        .arg(&out)
        .output()?;

    assert_eq!(output.status.code(), Some(2), "invalid input: {output:?}");
    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("gzip") && stderr.contains(".json.zst"),
        "names the cause and the alternative: {stderr}"
    );
    assert!(!out.exists(), "nothing is written");
    Ok(())
}
