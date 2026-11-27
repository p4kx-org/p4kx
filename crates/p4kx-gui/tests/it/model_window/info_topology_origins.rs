// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use egui_kittest::kittest::Queryable;
use p4kx_extraction::{FieldOrigin, PhysicalSourceKind};
use p4kx_gui::{Language, ModelInfoProvenance, Unavailable, ViewerSupport};
use p4kx_testkit::{TestEntry, TestModel};

use super::info_chunk_origins::{assert_marker, reveal_info_row};
use super::info_mesh_origins::{provenance, synthetic_site};
use crate::viewing::{TestResult, model_preview, opened_at, select};

const PRIMARY: &str = "Data/Objects/fallback.cgf";
const BASE: &str = "Data/Objects/fallback.cgfm";
const INLINE: &str = "Data/Objects/inline.cgf";
const HELPERS: &str = "Data/Objects/helpers.cgf";

#[test]
fn topology_uses_only_the_successful_preview_load_and_preserves_unknown_geometry() -> TestResult {
    let cube = TestModel::cube();
    let entries = vec![
        TestEntry::zstd(PRIMARY, cube.cgf()),
        TestEntry::zstd(BASE, cube.cgfm()).encrypted(),
        TestEntry::zstd("Data/Objects/fallback_lod1.cgfm", b"corrupt chunk".to_vec()).encrypted(),
        TestEntry::zstd(INLINE, TestModel::cube().single_file().cgf()).encrypted(),
        TestEntry::zstd(HELPERS, TestModel::cube().helpers_only().cgf()).encrypted(),
    ];
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        synthetic_site(entries),
        |builder| builder,
    )?;

    select(&mut harness, "fallback.cgf")?;
    let preview = model_preview(harness.state()).ok_or("model preview")?;
    let info = preview.info().ok_or("model facts")?;
    assert_eq!(info.lods.len(), 1, "the retry admits only the base level");
    assert_eq!(info.meshes(), 1, "the first emitted row is still present");
    assert_eq!(
        info.geometry.as_deref(),
        Some("fallback.cgfm"),
        "the base companion"
    );
    assert_companion_topology(provenance(preview)?, PRIMARY, BASE, &[], Some(true))?;

    harness.get_by_label("Info").click();
    harness.run_steps(4);
    reveal_info_row(&mut harness, "fallback.cgfm")?;
    assert_marker(
        &mut harness,
        "fallback.cgfm",
        "Includes encrypted source data",
    )?;

    for (name, path, mesh_count) in [("inline.cgf", INLINE, 1), ("helpers.cgf", HELPERS, 0)] {
        select(&mut harness, name)?;
        let preview = model_preview(harness.state()).ok_or("model preview")?;
        let info = preview.info().ok_or("model facts")?;
        assert_eq!(info.geometry, None, "no companion for {path}");
        assert_eq!(info.meshes(), mesh_count, "the original display count");
        assert_in_file_topology(provenance(preview)?, path, mesh_count)?;
    }
    Ok(())
}

pub(super) fn assert_companion_topology(
    proof: &ModelInfoProvenance,
    primary: &str,
    base: &str,
    higher: &[&str],
    base_encrypted: Option<bool>,
) -> TestResult {
    let topology = &proof.mesh_summary().topology;
    assert_field(proof, &topology.geometry, [primary, base], base_encrypted)?;
    assert_field(
        proof,
        &topology.lod_count,
        std::iter::once(base).chain(higher.iter().copied()),
        if higher.is_empty() {
            base_encrypted
        } else {
            Some(true)
        },
    )?;
    let [first, rest @ ..] = topology.meshes_by_lod.as_slice() else {
        return Err("the first emitted mesh count".into());
    };
    assert_field(proof, first, [base], base_encrypted)?;
    assert_eq!(rest.len(), higher.len(), "one source row per emitted LOD");
    for (row, &path) in rest.iter().zip(higher) {
        assert_field(proof, row, [path], Some(true))?;
    }
    Ok(())
}

fn assert_in_file_topology(
    proof: &ModelInfoProvenance,
    path: &str,
    mesh_count: usize,
) -> TestResult {
    let topology = &proof.mesh_summary().topology;
    let [first] = topology.meshes_by_lod.as_slice() else {
        return Err("the single in-file LOD".into());
    };
    assert_field(proof, &topology.lod_count, [path], Some(true))?;
    for origin in [first, &topology.geometry] {
        if mesh_count == 0 {
            assert_field(proof, origin, [], None)?;
        } else {
            assert_field(proof, origin, [path], Some(true))?;
        }
    }
    Ok(())
}

fn assert_field<'a>(
    proof: &ModelInfoProvenance,
    origin: &FieldOrigin,
    expected: impl IntoIterator<Item = &'a str>,
    encrypted: Option<bool>,
) -> TestResult {
    let inventory = proof.inventory();
    let mut paths = BTreeSet::new();
    for reference in origin.source_uses_refs() {
        let used = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("topology source use")?;
        let source = inventory
            .iter_sources()
            .find(|source| source.id() == used.source())
            .ok_or("topology physical source")?;
        let PhysicalSourceKind::ArchiveEntry(entry) = source.kind() else {
            return Err("topology needs an exact archive Entry".into());
        };
        let path = entry.path().value().ok_or("topology physical path")?;
        paths.insert(path.as_str());
    }
    let expected: BTreeSet<_> = expected.into_iter().collect();
    assert_eq!(paths, expected, "the admitted Entry paths");
    assert!(
        !origin.source_set_complete(),
        "this field retains selected inputs"
    );
    assert_eq!(
        origin.contains_encrypted_source(),
        encrypted,
        "source encryption"
    );
    Ok(())
}
