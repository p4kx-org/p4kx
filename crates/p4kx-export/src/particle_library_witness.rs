// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::sink;

use p4kx_extraction::{Checksum, SourceValueState};
use p4kx_testkit::real_archive_pair;

use super::{TestResult, rain_slots};
use crate::build_derived::particle_library::{
    Archive, Observation, ParticleLibrary, in_scope, observe,
};
use crate::catalog_read::original::with_entry_capture;
use crate::entry_source::EntrySource;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_particle_libraries_keep_exact_occurrences() -> TestResult {
    let Some((live, ptu)) = real_archive_pair()? else {
        return Ok(());
    };
    let weather_path = std::env::var("P4KX_PARTICLE_WEATHER_ENTRY")?;
    let lightning_path = std::env::var("P4KX_PARTICLE_LIGHTNING_ENTRY")?;
    for (archive_path, expected_change, expected_rain, expected_lightning) in
        [(live, "13165946", 0, 30), (ptu, "13189690", 8, 31)]
    {
        let archive = Archive::open(archive_path)?;
        let build = archive.build().ok_or("readable installed Build manifest")?;
        assert_eq!(
            build.change_number(),
            expected_change,
            "original Build changelist"
        );
        let weather = projected_sample(&archive, &weather_path)?;
        let lightning = projected_sample(&archive, &lightning_path)?;
        let rain_count = rain_slots(&weather)
            .filter(|(_, rain)| rain.state() == SourceValueState::Present)
            .count();
        let lightning_count = lightning
            .nodes
            .iter()
            .filter(|node| node.name.value().is_some_and(|name| name == "Lightning"))
            .count();
        assert_eq!(weather.nodes.len(), 62);
        assert_eq!(rain_count, expected_rain);
        assert_eq!(lightning_count, expected_lightning);
        for key in ["SharedLight", "StartLight", "TargetLight"] {
            assert!(
                lightning
                    .nodes
                    .iter()
                    .flat_map(|node| &node.attributes)
                    .any(|attribute| { attribute.name.value().is_some_and(|name| name == key) }),
                "authored attribute key {key}"
            );
        }
    }
    Ok(())
}

fn projected_sample(archive: &Archive, path: &str) -> TestResult<ParticleLibrary> {
    let path = crate::starmap_index::key(path);
    assert!(in_scope(&path), "sample must use the ordinary source scope");
    let mut entries = archive
        .entries()
        .iter()
        .filter(|entry| crate::starmap_index::key(entry.path()) == path);
    let entry = entries
        .next()
        .ok_or("exact installed XML Entry is present")?;
    assert!(
        entries.next().is_none(),
        "sample has a unique physical owner"
    );
    assert!(
        matches!(entry.checksum(), Checksum::Crc32(_) | Checksum::Crc32c(_)),
        "sample declares an original CRC"
    );
    let size = std::io::copy(
        &mut archive.open_entry_verified(entry)?.reject_overlong(),
        &mut sink(),
    )?;
    assert_eq!(
        size,
        entry.uncompressed_size(),
        "independently verified whole Entry"
    );
    let outcome = with_entry_capture(archive, |capture| {
        let source = EntrySource::outer(capture, archive, entry)?;
        observe(archive, entry, &source, capture, None)
    })?;
    match outcome {
        Observation::Projected(library) => Ok(library),
        _ => Err("a CRC-verified sample did not project".into()),
    }
}
