// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::animation_headers::AnimationSourceHeaders;
use p4kx_extraction::{Archive, OpenOptions, PathKeys};
use p4kx_testkit::{ArchiveBuilder, TestAnimation, TestDba, TestEntry, test_key};

use super::{CAF, PROTECTED, TestResult, resolve_tree, synthetic_archive, track};
use crate::{ArchiveModels, ClipSourceSelection, discover_entity_clips_with};

#[derive(Debug, PartialEq, Eq)]
struct Interrupted;

#[test]
fn interruption_in_archive_stream_including_eof_is_propagated() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("stream.p4k");
    let payload = vec![47; 128 << 10];
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/test.caf", payload.clone()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive.entries().first().ok_or("stream entry")?;
    let models = ArchiveModels::new(&archive);
    let mut completed = 0;
    let bytes = models.read_file_entry_with(entry, u64::MAX, &mut || {
        completed += 1;
        Ok::<_, Interrupted>(())
    });
    assert!(matches!(bytes, Ok(Ok(bytes)) if bytes == payload));
    assert!(
        completed > 4,
        "larger-than-block payload has multiple checkpoints"
    );
    let mut count = 0;
    let abort = models.read_file_entry_with(entry, u64::MAX, &mut || {
        count += 1;
        if count == completed {
            Err(Interrupted)
        } else {
            Ok(())
        }
    });
    assert!(
        matches!(abort, Err(Interrupted)),
        "final EOF polls before success"
    );
    Ok(())
}

#[test]
fn cancellation_during_controller_headers_returns_no_partial_inventory() -> TestResult {
    let mut large = TestAnimation::new("Many controllers", 30);
    for controller in 0..128 {
        large = large.track(track(controller));
    }
    let bytes = TestDba::new().animation(large).bytes();
    let mut checkpoints = 0;
    let result = AnimationSourceHeaders::read_with("fixture.dba", &bytes, &mut || {
        checkpoints += 1;
        if checkpoints == 64 {
            Err(Interrupted)
        } else {
            Ok(())
        }
    });
    assert!(matches!(result, Err(Interrupted)));
    let complete = AnimationSourceHeaders::read("fixture.dba", &bytes)?;
    let clip = complete.clips.first().ok_or("DBA clip")?;
    let block = clip
        .controller_block
        .as_ref()
        .ok_or("original descriptors")?;
    assert_eq!(block.controllers.len(), 128);
    Ok(())
}

#[test]
fn checked_entity_discovery_retains_physical_folder_order() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("physical-order.p4k");
    synthetic_archive().write_to(&path)?;
    let archive = Archive::open_with(path, OpenOptions::new().key(Some(test_key())))?;
    let keys = PathKeys::try_new_with(&archive, |_| Ok::<_, std::io::Error>(()))?;
    let models = ArchiveModels::from_path_index(keys.index(&archive));
    let tree = resolve_tree(&archive)?;
    let checked = discover_entity_clips_with(&models, &tree, &mut || Ok::<_, std::io::Error>(()))?;
    let root = checked.rigs.first().ok_or("root")?;
    let folder = root.links.get(1).ok_or("authored folder")?;
    let ClipSourceSelection::Folder { sources } = &folder.selection else {
        return Err("resolved folder candidates".into());
    };
    let paths: Vec<_> = sources
        .iter()
        .filter_map(|&slot| checked.sources.get(slot))
        .map(|source| source.file.path.as_str())
        .collect();
    assert_eq!(paths, [CAF, PROTECTED], "use original physical order");
    let interrupted = discover_entity_clips_with(&models, &tree, &mut || Err(Interrupted));
    assert!(matches!(interrupted, Err(Interrupted)));
    Ok(())
}
