// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, InputScope, SourceInventory, SourceLocation};
use p4kx_gui::{Language, ModelPreview, Unavailable, ViewerSupport};
use p4kx_testkit::{TestEntry, TestModel};
use serde_json::Value;

use crate::viewing::{Site, TestResult, model_preview, opened_at, select};

const SKIN: &str = "Data/Objects/rigged.skin";
const CUBE: &str = "Data/Objects/cube.cgf";

#[test]
fn model_counts_bind_the_primary_entry_for_authored_and_generated_values() -> TestResult {
    let rigged = TestModel::two_bone_skin();
    let cube = TestModel::cube();
    let skin_bytes = rigged.cgf();
    let cube_bytes = cube.cgf();
    let site = Site {
        entries: vec![
            TestEntry::zstd(SKIN, skin_bytes.clone()),
            TestEntry::zstd("Data/Objects/rigged.skinm", rigged.cgfm()).encrypted(),
            TestEntry::zstd(CUBE, cube_bytes.clone()).encrypted(),
            TestEntry::zstd("Data/Objects/cube.cgfm", cube.cgfm()),
        ],
        installation: false,
        key: true,
    };
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        site,
        |builder| builder,
    )?;

    for (name, fixture) in [
        (
            "rigged.skin",
            Counts {
                path: SKIN,
                bytes: &skin_bytes,
                encrypted: false,
                nodes: 1,
                bones: 2,
                node_word: None,
                bone_word: Some(2),
            },
        ),
        (
            "cube.cgf",
            Counts {
                path: CUBE,
                bytes: &cube_bytes,
                encrypted: true,
                nodes: 2,
                bones: 0,
                node_word: Some(2),
                bone_word: None,
            },
        ),
    ] {
        select(&mut harness, name)?;
        let preview = model_preview(harness.state()).ok_or("model preview")?;
        assert_preview_counts(preview, &fixture)?;
    }
    Ok(())
}

fn assert_preview_counts(preview: &ModelPreview, fixture: &Counts<'_>) -> TestResult {
    let info = preview.info().ok_or("displayed model facts")?;
    assert_eq!(info.nodes, fixture.nodes, "displayed node count");
    assert_eq!(info.bones, fixture.bones, "displayed bone count");
    let provenance = preview
        .info_provenance()
        .ok_or("model provenance")?
        .map_err(|error| format!("model provenance: {error}"))?;
    for (origin, selector, word) in [
        (
            provenance.node_count(),
            "nodeMeshTypePrefix",
            fixture.node_word,
        ),
        (
            provenance.bone_count(),
            "skeletonTypePrefix",
            fixture.bone_word,
        ),
    ] {
        assert_count(origin, provenance.inventory(), fixture, selector, word)?;
    }
    Ok(())
}

struct Counts<'a> {
    path: &'a str,
    bytes: &'a [u8],
    encrypted: bool,
    nodes: usize,
    bones: usize,
    node_word: Option<u32>,
    bone_word: Option<u32>,
}

fn assert_count(
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    fixture: &Counts<'_>,
    selector: &str,
    word: Option<u32>,
) -> TestResult {
    assert!(
        origin.source_set_complete(),
        "complete selection for {selector}"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(fixture.encrypted),
        "only the primary Entry determines {selector}"
    );
    let mut selected = false;
    let mut count = None;
    for reference in origin.source_uses_refs() {
        let used = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("bound count use")?;
        let entry = inventory
            .iter_sources()
            .find(|source| source.id() == used.source())
            .ok_or("physical origin of the count")?;
        assert_eq!(used.scope(), InputScope::Payload, "original model bytes");
        assert_eq!(
            serde_json::to_value(entry)?
                .pointer("/path/value")
                .and_then(Value::as_str),
            Some(fixture.path),
            "the physical Entry of {selector}"
        );
        if let SourceLocation::Subresource { field, indices, .. } = used.location() {
            selected |= field == selector;
            if field == "nodeCount" || field == "boneCount" {
                let [_, start, end] = indices.as_slice() else {
                    return Err("expected original count byte range".into());
                };
                let bytes = fixture
                    .bytes
                    .get(usize::try_from(*start)?..usize::try_from(*end)?)
                    .ok_or("count bytes in fixture")?;
                let value = u32::from_le_bytes(bytes.try_into()?);
                assert!(count.replace(value).is_none(), "one authored count word");
            }
        }
    }
    assert!(selected, "original chunk type selection for {selector}");
    assert_eq!(count, word, "authored count word for {selector}");
    Ok(())
}
