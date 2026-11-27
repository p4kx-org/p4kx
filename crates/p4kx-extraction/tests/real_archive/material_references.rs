// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opt-in material-reference canaries selected by source capability on both Archives.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::model::{LodSelection, MaterialReading, PositionBox};
use p4kx_extraction::{
    Archive, Entry, GlbOptions, MaterialMode, ModelSource, PathIndex, TextureVerdicts,
};
use p4kx_testkit::{check_glb, real_archive_key, real_archive_pair};

type TestResult = Result<(), Box<dyn Error>>;

const MAX_CANDIDATES: usize = 4096;
const MAX_CANDIDATE_BYTES: u64 = 64 << 20;

struct Discovery {
    selected: [String; 2],
    examined: usize,
    probe_failures: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn both_archives_resolve_selected_virtual_and_authoring_materials() -> TestResult {
    let Some(paths) = real_archive_pair()? else {
        super::report("material-reference coverage unavailable: second Archive is not configured");
        return Ok(());
    };
    let paths: [_; 2] = paths.into();
    for path in paths {
        check_archive(&path)?;
    }
    Ok(())
}

fn check_archive(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(real_archive_key()?))?;
    let index = PathIndex::new(&archive);
    let discovery = discover(&archive, &index, &TextureVerdicts::new())?;
    super::report(&format!(
        "material-reference discovery: {} candidates, {} unavailable probes",
        discovery.examined, discovery.probe_failures,
    ));
    let output = tempfile::tempdir()?;
    for (position, selected) in discovery.selected.iter().enumerate() {
        check_selected(
            &index,
            selected,
            &output.path().join(format!("{position}.glb")),
        )?;
    }
    Ok(())
}

fn candidates(archive: &Archive) -> impl Iterator<Item = &Entry> {
    let mut candidates = BTreeMap::new();
    for (position, entry) in archive.entries().iter().enumerate() {
        let extension = entry.path().rsplit('.').next().unwrap_or_default();
        if entry.uncompressed_size() > MAX_CANDIDATE_BYTES
            || !["cgf", "cga", "skin"]
                .iter()
                .any(|suffix| extension.eq_ignore_ascii_case(suffix))
        {
            continue;
        }
        let key = (entry.path(), position);
        if candidates.len() == MAX_CANDIDATES
            && candidates
                .last_key_value()
                .is_some_and(|(largest, _)| key >= *largest)
        {
            continue;
        }
        candidates.insert(key, entry);
        if candidates.len() > MAX_CANDIDATES {
            let _largest = candidates.pop_last();
        }
    }
    candidates.into_values()
}

fn discover(
    archive: &Archive,
    index: &PathIndex<'_>,
    verdicts: &TextureVerdicts,
) -> Result<Discovery, Box<dyn Error>> {
    let mut selected = [None, None];
    let mut probe_failures = 0;
    let mut examined = 0;
    for entry in candidates(archive) {
        examined += 1;
        let Ok(source) = ModelSource::from_index_reading(
            index,
            entry.path(),
            LodSelection::Only(0),
            PositionBox::default(),
            MaterialReading::Skipped,
            verdicts,
        ) else {
            probe_failures += 1;
            continue;
        };
        consider(&source, entry.path(), &mut selected);
        if selected.iter().all(Option::is_some) {
            break;
        }
    }
    let [Some(virtual_root), Some(authoring)] = selected else {
        return Err(format!(
            "both material-reference shapes required within {MAX_CANDIDATES} candidates; capability unavailable"
        ).into());
    };
    Ok(Discovery {
        selected: [virtual_root, authoring],
        examined,
        probe_failures,
    })
}

fn consider(source: &ModelSource, path: &str, selected: &mut [Option<String>; 2]) {
    let Some(reference) = source.model().material_library.as_deref() else {
        return;
    };
    let shape = if reference.starts_with("//") {
        Some(1)
    } else if reference.starts_with('/') {
        Some(0)
    } else {
        None
    };
    if let Some(slot) = shape.and_then(|shape| selected.get_mut(shape))
        && slot.is_none()
        && source
            .model()
            .lods
            .iter()
            .any(|lod| lod.triangle_count() > 0)
    {
        *slot = Some(path.to_owned());
    }
}

fn check_selected(index: &PathIndex<'_>, selected: &str, target: &Path) -> TestResult {
    let source = ModelSource::from_index(index, selected)?;
    let library = source.model().materials.library().ok_or_else(|| {
        source
            .model()
            .materials
            .problem()
            .unwrap_or_else(|| "no library".to_owned())
    })?;
    if library.target.is_none() {
        return Err("selected Archive library has no mapped target".into());
    }
    source.write_glb(
        &GlbOptions::default().with_materials(MaterialMode::Colours),
        target,
    )?;
    let glb = check_glb(&fs::read(target)?)?;
    let assigned = glb
        .document
        .meshes()
        .flat_map(|mesh| mesh.primitives())
        .any(|primitive| {
            primitive.material().name().is_some_and(|name| {
                name != "default"
                    && library
                        .materials
                        .iter()
                        .any(|material| material.name == name)
            })
        });
    if !assigned {
        return Err("selected model has no binding to its loaded material library".into());
    }
    Ok(())
}

#[test]
fn bounded_candidates_keep_the_same_physical_order_as_a_full_sorted_inventory() -> TestResult {
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    let mut builder = ArchiveBuilder::new().entry(TestEntry::stored("Data/ignore.txt", b"x"));
    for position in (0..MAX_CANDIDATES + 2).rev() {
        builder = builder.entry(TestEntry::stored(format!("Data/{position}.cgf"), b"x"));
    }
    builder
        .entry(TestEntry::stored("Data/0.cgf", b"duplicate"))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let selected: Vec<_> = candidates(&archive).collect();
    let mut expected: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| {
            entry
                .path()
                .rsplit('.')
                .next()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("cgf"))
        })
        .collect();
    expected.sort_by(|left, right| left.path().cmp(right.path()));
    expected.truncate(MAX_CANDIDATES);
    assert_eq!(
        selected.len(),
        MAX_CANDIDATES,
        "candidate storage obeys the real scan cap"
    );
    assert!(
        selected
            .iter()
            .zip(&expected)
            .all(|(actual, expected)| std::ptr::eq(*actual, *expected)),
        "ordering and physical duplicate identities remain unchanged"
    );
    Ok(())
}
