// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::ConflictPolicy;
use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_media::ExportOutcome;
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::{LINKS, SOURCE};
use crate::material_origin_support::{self as support, Fixture, MTL, SHADER, Storage, TestResult};

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
