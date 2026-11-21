// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{SplitDds, TestContainer, TestEntry, TestTextureFormat};
use serde_json::{Value, json};

use super::image_preview::preview;
use crate::support::{Install, TestResult};

const PNG: &str = "/content/1/data";
const ROOT: &str = "Data/probe.dds";

fn field<'a>(meta: &'a Value, name: &str) -> TestResult<&'a Value> {
    meta["fieldOrigins"]
        .get(format!("/structuredContent/{name}"))
        .ok_or_else(|| format!("missing field origin: {name}").into())
}

fn origin_uses<'a>(meta: &'a Value, origin: &Value) -> TestResult<Vec<&'a Value>> {
    let uses = meta
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?;
    origin["sourceUsesRefs"]
        .as_array()
        .ok_or("field source references")?
        .iter()
        .map(|reference| {
            uses.iter()
                .find(|used| &used["id"] == reference)
                .ok_or_else(|| "unresolved source use".into())
        })
        .collect()
}

fn mip_uses(meta: &Value) -> TestResult<Vec<(&str, &Value)>> {
    let sources = meta
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let png = meta
        .get("fieldOrigins")
        .and_then(|fields| fields.get(PNG))
        .ok_or("PNG origin")?;
    let mut mips = origin_uses(meta, png)?
        .into_iter()
        .filter(|used| {
            used.pointer("/location/namespace").and_then(Value::as_str) == Some("ddsMipPayload")
        })
        .map(|used| {
            assert_eq!(
                used.get("role").and_then(Value::as_str),
                Some("conversion"),
                "the encoded bytes supplied pixels"
            );
            let path = sources
                .iter()
                .find(|source| source.get("id") == used.get("source"))
                .and_then(|source| source.pointer("/path/value"))
                .and_then(Value::as_str)
                .ok_or("mip source path")?;
            Ok((
                path,
                used.pointer("/location/indices").ok_or("mip byte spans")?,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    mips.sort_unstable_by_key(|(path, _)| *path);
    Ok(mips)
}

fn marker(origin: &Value, expected: &Value) {
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(expected),
        "field encryption"
    );
    assert_eq!(
        origin.get("sourceSetComplete").and_then(Value::as_bool),
        Some(true),
        "all contributors are retained"
    );
}

#[test]
fn a_plain_header_and_an_encrypted_selected_alpha_part_have_different_field_origins() -> TestResult
{
    let entries = SplitDds::new(TestTextureFormat::LegacyBgra8, 16, 8)
        .split(2)
        .alpha(TestTextureFormat::R8)
        .entries(ROOT)
        .into_iter()
        .map(|entry| {
            if matches!(entry.path(), "Data/probe.dds.1" | "Data/probe.dds.2a") {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let install = Install::new(entries)?;
    let mut keyed = install.serve(&["--archive", "LIVE"])?;
    let (split, _) = preview(&mut keyed, &json!({"path": ROOT, "mip": 0}))?;
    marker(field(&split, "texture_width")?, &json!(false));
    marker(field(&split, "width")?, &json!(false));
    marker(field(&split, "alpha_tail")?, &json!(false));
    marker(field(&split, "png_bytes")?, &json!(true));
    assert_eq!(&split["fieldOrigins"][PNG], field(&split, "png_bytes")?);
    assert_eq!(split["alpha_tail"], true);
    let width = origin_uses(&split, field(&split, "texture_width")?)?;
    assert_eq!(width.len(), 1);
    assert_eq!(width[0]["role"], "payload");
    assert_eq!(width[0]["location"]["field"], "width");
    assert_eq!(width[0]["location"]["indices"], json!(["16", "4"]));
    assert_eq!(
        mip_uses(&split)?,
        [
            ("Data/probe.dds.2", &json!(["0", "0", "512", "0", "1"])),
            ("Data/probe.dds.2a", &json!(["0", "0", "128", "0", "1"])),
        ]
    );
    assert_eq!(
        split["provenance"]["sources"]
            .as_array()
            .ok_or("sources")?
            .len(),
        4,
        "only the headers and the selected part in each texture"
    );

    let mut no_key = install.serve_without_key(&["--archive", "LIVE"])?;
    let (tail, _) = preview(&mut no_key, &json!({"path": ROOT, "mip": 2}))?;
    marker(&tail["fieldOrigins"][PNG], &json!(false));
    assert_eq!(
        mip_uses(&tail)?,
        [
            ("Data/probe.dds", &json!(["2", "128", "32", "0", "1"])),
            ("Data/probe.dds.a", &json!(["2", "144", "8", "0", "1"])),
        ]
    );
    Ok(())
}

#[test]
fn a_selected_face_retains_one_span_while_a_cross_retains_the_strided_mip() -> TestResult {
    let entries = SplitDds::new(TestTextureFormat::LegacyBgra8, 8, 8)
        .cubemap()
        .split(1)
        .mips(3)
        .alpha(TestTextureFormat::R8)
        .entries(ROOT);
    let install = Install::new(entries)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;

    let (split, _) = preview(
        &mut server,
        &json!({"path": ROOT, "mip": 0, "face": "posy"}),
    )?;
    assert_eq!(
        mip_uses(&split)?,
        [
            ("Data/probe.dds.1", &json!(["0", "512", "256", "0", "1"])),
            ("Data/probe.dds.1a", &json!(["0", "128", "64", "0", "1"])),
        ]
    );
    let (face, _) = preview(
        &mut server,
        &json!({"path": ROOT, "mip": 1, "face": "negz"}),
    )?;
    assert_eq!(
        mip_uses(&face)?,
        [
            ("Data/probe.dds", &json!(["1", "528", "64", "0", "1"])),
            ("Data/probe.dds.a", &json!(["1", "244", "16", "0", "1"])),
        ]
    );
    assert_eq!(
        origin_uses(&face, field(&face, "face")?)?
            .iter()
            .map(|used| used["location"]["field"].as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        [Some("caps2"), Some("pixelFormat.fourCc")]
            .into_iter()
            .collect()
    );

    let (cross, _) = preview(&mut server, &json!({"path": ROOT, "mip": 1}))?;
    assert_eq!(
        mip_uses(&cross)?,
        [
            ("Data/probe.dds", &json!(["1", "128", "64", "80", "6"])),
            ("Data/probe.dds.a", &json!(["1", "144", "16", "20", "6"])),
        ]
    );
    assert!(
        cross["fieldOrigins"]
            .get("/structuredContent/face")
            .is_none()
    );
    Ok(())
}

#[test]
fn nested_payloads_inherit_container_encryption_and_use_original_stored_names() -> TestResult {
    let leaf = SplitDds::new(TestTextureFormat::R8, 4, 4).entries("icon.dds");
    let inner = leaf
        .into_iter()
        .fold(TestContainer::new(), TestContainer::entry)
        .build();
    let outer = TestContainer::new()
        .entry(TestEntry::stored("inner.socpak", inner))
        .build();
    let install = Install::new(vec![TestEntry::deflated("outer.socpak", outer).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let (meta, _) = preview(
        &mut server,
        &json!({"path": "OUTER.SOCPAK::INNER.SOCPAK::ICON.DDS"}),
    )?;
    assert_eq!(meta["path"], "outer.socpak::inner.socpak::icon.dds");
    marker(field(&meta, "texture_width")?, &json!(true));
    marker(field(&meta, "path")?, &json!(true));
    marker(&meta["fieldOrigins"][PNG], &json!(true));
    marker(field(&meta, "archive")?, &json!(false));

    let sources = meta["provenance"]["sources"]
        .as_array()
        .ok_or("physical sources")?;
    let mut binding = sources
        .iter()
        .map(|source| {
            (
                &source["path"]["value"],
                &source["storedEncryption"],
                source["ancestors"].as_array().map(Vec::len),
            )
        })
        .collect::<Vec<_>>();
    binding.sort_unstable_by_key(|(path, _, _)| path.as_str());
    assert_eq!(
        binding,
        [
            (&json!("icon.dds"), &json!("unknown"), Some(2)),
            (&json!("inner.socpak"), &json!("unknown"), Some(1)),
            (&json!("outer.socpak"), &json!("encrypted"), Some(0)),
        ]
    );
    let paths = origin_uses(&meta, field(&meta, "path")?)?;
    assert_eq!(paths.len(), 3);
    assert!(
        paths
            .iter()
            .all(|used| used["scope"] == "archiveDirectoryMetadata")
    );
    assert_eq!(
        paths
            .iter()
            .filter(|used| used["effectiveEncryption"] == "encrypted")
            .count(),
        2
    );
    Ok(())
}

#[test]
fn readable_unknown_declarations_remain_unknown_and_keep_png_bytes_identical() -> TestResult {
    let bytes = SplitDds::new(TestTextureFormat::R8, 4, 4).header_bytes();
    let plain = "Data/plain.dds";
    let install = Install::new(vec![
        TestEntry::stored(plain, &bytes[..]),
        TestEntry::stored(ROOT, bytes)
            .encrypted()
            .with_encryption_field(7),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let (_, plain_png) = preview(&mut server, &json!({"path": plain}))?;
    let (meta, unknown_png) = preview(&mut server, &json!({"path": ROOT}))?;
    assert_eq!(unknown_png, plain_png);
    marker(field(&meta, "format")?, &Value::Null);
    marker(field(&meta, "path")?, &json!(false));
    marker(&meta["fieldOrigins"][PNG], &Value::Null);
    assert_eq!(origin_uses(&meta, field(&meta, "format")?)?.len(), 2);
    Ok(())
}

#[test]
fn tool_output_schema_declares_the_exact_field_pointers_and_binary_origin() -> TestResult {
    let install = Install::new(SplitDds::new(TestTextureFormat::R8, 4, 4).entries(ROOT))?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let tools = server.tools()?;
    let schema = &tools
        .iter()
        .find(|tool| tool["name"] == "image_preview")
        .ok_or("image preview tool")?["outputSchema"];
    let fields = &schema["$defs"]["ImageFieldOrigins"];
    assert!(schema["properties"]["fieldOrigins"].is_object());
    assert!(schema["properties"]["provenance"].is_object());
    assert_eq!(fields["additionalProperties"], false);
    let mandatory = fields["required"]
        .as_array()
        .ok_or("required origin keys")?;
    assert!(mandatory.contains(&json!(PNG)));
    assert!(mandatory.contains(&json!("/structuredContent/width")));
    assert!(!mandatory.contains(&json!("/structuredContent/face")));

    let response = server.call("image_preview", &json!({"path": ROOT}))?;
    assert_eq!(response["content"][1]["type"], "image");
    let origins = response["structuredContent"]["fieldOrigins"]
        .as_object()
        .ok_or("field origins")?;
    assert_eq!(origins.len(), 16);
    for pointer in origins.keys() {
        assert!(response.pointer(pointer).is_some(), "{pointer}");
    }
    Ok(())
}
