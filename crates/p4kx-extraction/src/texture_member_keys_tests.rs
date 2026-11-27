// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::{TextureMemberKeys, TextureMemberRelation};
use crate::path_index;
use crate::{Archive, PathIndexBuildError, PathIndexError, TextureMember, TextureMemberRole};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn complete_families_keep_duplicate_headers_gaps_late_numbers_and_orphans() -> TestResult {
    with_entries(
        [
            "Data/a.dds",
            "data/A.DDS",
            "Data/a.dds.1",
            "Data/a.dds.3",
            "Data/a.dds.33",
            "Data/a.dds.a",
            "Data/a.dds.3a",
            "Data/A.DDS.A",
            "Data/orphan.dds.7",
            "Data/a.dds.4294967295",
            "Data/other.xml",
        ]
        .map(|path| TestEntry::stored(path, b"x")),
        |archive| {
            let keys = TextureMemberKeys::try_new_with(archive, |_| Ok::<_, io::Error>(()))?;
            let index = keys.index(archive);
            let rows: Vec<_> = index
                .family_candidates_with("DATA\\A.dds", |_| Ok::<_, io::Error>(()))?
                .collect::<Result<_, _>>()?;
            assert_eq!(
                rows.iter().map(|row| row.entry.index()).collect::<Vec<_>>(),
                [0, 1, 2, 3, 4, 5, 6, 7, 9]
            );
            assert!(matches!(
                rows[4].relation,
                TextureMemberRelation::Matched {
                    role: TextureMemberRole::Member(_, TextureMember::Part(33))
                }
            ));
            assert!(matches!(
                rows[8].relation,
                TextureMemberRelation::Matched {
                    role: TextureMemberRole::Member(_, TextureMember::Part(u32::MAX))
                }
            ));
            let orphan: Vec<_> = index
                .family_candidates_with("Data/orphan.dds", |_| Ok::<_, io::Error>(()))?
                .collect::<Result<_, _>>()?;
            assert_eq!(orphan.len(), 1);
            assert_eq!(orphan[0].entry.index(), 8);
            assert!(matches!(
                orphan[0].relation,
                TextureMemberRelation::Matched {
                    role: TextureMemberRole::Member(_, TextureMember::Part(7))
                }
            ));
            Ok(())
        },
    )
}

#[test]
fn hash_collisions_are_rejected_by_the_actual_owning_name() -> TestResult {
    with_entries(
        [
            TestEntry::stored("Data/unrelated.dds.1", b"x"),
            TestEntry::stored("Data/same.dds.1", b"one"),
            TestEntry::stored("data/SAME.DDS.1", b"two"),
        ],
        |archive| {
            let hash = path_index::path_key("Data/same.dds");
            let keys = TextureMemberKeys {
                keys: vec![(hash, 0), (hash, 1), (hash, 2)],
            };
            let index = keys.index(archive);
            let rows: Vec<_> = index
                .family_candidates_with("data/same.DDS", |_| Ok::<_, io::Error>(()))?
                .collect::<Result<_, _>>()?;
            assert_eq!(
                rows.iter().map(|row| row.entry.index()).collect::<Vec<_>>(),
                [1, 2]
            );
            assert_eq!(
                rows.iter()
                    .map(|row| row.entry.uncompressed_size())
                    .collect::<Vec<_>>(),
                [3, 3]
            );
            Ok(())
        },
    )
}

#[test]
fn family_names_use_ascii_case_only() -> TestResult {
    with_entries(
        [
            TestEntry::stored("Data/\u{00c4}.dds", b"x"),
            TestEntry::stored("Data/\u{00e4}.dds.1", b"x"),
        ],
        |archive| {
            let keys = TextureMemberKeys::try_new_with(archive, |_| Ok::<_, io::Error>(()))?;
            let index = keys.index(archive);
            let rows: Vec<_> = index
                .family_candidates_with("Data/\u{00c4}.dds", |_| Ok::<_, io::Error>(()))?
                .collect::<Result<_, _>>()?;
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].entry.index(), 0);
            Ok(())
        },
    )
}

#[test]
fn lossy_display_candidates_keep_physical_identity_without_adopted_roles() -> TestResult {
    with_entries(
        [
            TestEntry::stored("x", b"header").with_path_bytes(b"Data/bad\xff.dds"),
            TestEntry::stored("x", b"part").with_path_bytes(b"Data/bad\xfe.dds.3"),
            TestEntry::stored("Data/bad\u{fffd}.dds", b"lossless"),
        ],
        |archive| {
            let keys = TextureMemberKeys::try_new_with(archive, |_| Ok::<_, io::Error>(()))?;
            let index = keys.index(archive);
            let rows: Vec<_> = index
                .family_candidates_with("Data/bad\u{fffd}.dds", |_| Ok::<_, io::Error>(()))?
                .collect::<Result<_, _>>()?;
            assert_eq!(
                rows.iter().map(|row| row.entry.index()).collect::<Vec<_>>(),
                [0, 1, 2]
            );
            assert!(matches!(
                rows[0].relation,
                TextureMemberRelation::UnsupportedRawPath
            ));
            assert!(matches!(
                rows[1].relation,
                TextureMemberRelation::UnsupportedRawPath
            ));
            assert_eq!(
                archive.raw_path(rows[0].entry.index()),
                Some(b"Data/bad\xff.dds".as_slice())
            );
            assert_eq!(
                archive.raw_path(rows[1].entry.index()),
                Some(b"Data/bad\xfe.dds.3".as_slice())
            );
            assert!(matches!(
                rows[2].relation,
                TextureMemberRelation::Matched {
                    role: TextureMemberRole::Header
                }
            ));
            Ok(())
        },
    )
}

#[test]
fn an_interrupted_build_returns_the_original_failure_without_an_index() -> TestResult {
    use std::sync::atomic::Ordering;

    with_entries(
        [
            TestEntry::stored("Data/a.dds", b"x"),
            TestEntry::stored("Data/a.dds.1", b"x"),
        ],
        |archive| {
            let cancel = crate::CancelToken::new();
            let mut charged = 0;
            let result = TextureMemberKeys::try_new_with(archive, |count| {
                charged += count;
                if charged >= 3 {
                    cancel.cancel();
                }
                if cancel.flag().load(Ordering::Relaxed) {
                    Err(io::Error::from(io::ErrorKind::Interrupted))
                } else {
                    Ok(())
                }
            });
            assert!(
                matches!(result, Err(PathIndexBuildError::Interrupted(cause))
            if cause.kind() == io::ErrorKind::Interrupted)
            );
            assert!(cancel.flag().load(Ordering::Relaxed));
            Ok(())
        },
    )
}

#[test]
fn a_candidate_admission_error_ends_the_iterator_with_its_original_cause() -> TestResult {
    with_entries([TestEntry::stored("Data/a.dds", b"x")], |archive| {
        let keys = TextureMemberKeys::try_new_with(archive, |_| Ok::<_, io::Error>(()))?;
        let index = keys.index(archive);
        let mut charged = 0;
        let mut rows = index.family_candidates_with("Data/a.dds", |count| {
            charged += count;
            if charged > 2 {
                Err(io::Error::from(io::ErrorKind::PermissionDenied))
            } else {
                Ok(())
            }
        })?;
        assert!(
            matches!(rows.next(), Some(Err(PathIndexBuildError::Interrupted(cause)))
            if cause.kind() == io::ErrorKind::PermissionDenied)
        );
        assert!(rows.next().is_none());
        Ok(())
    })
}

#[test]
fn an_invalid_physical_position_is_a_typed_index_failure() -> TestResult {
    with_entries([TestEntry::stored("Data/a.dds", b"x")], |archive| {
        let keys = TextureMemberKeys {
            keys: vec![(path_index::path_key("Data/a.dds"), u32::MAX)],
        };
        let index = keys.index(archive);
        let mut rows = index.family_candidates_with("Data/a.dds", |_| Ok::<_, io::Error>(()))?;
        assert!(matches!(
            rows.next(),
            Some(Err(PathIndexBuildError::Index(
                PathIndexError::SourceMismatch
            )))
        ));
        Ok(())
    })
}

#[test]
fn no_recognised_members_require_no_key_vector_storage() -> TestResult {
    with_entries(
        [
            TestEntry::stored("Data/a.mtl", b"x"),
            TestEntry::stored("Data/a.dds.0", b"x"),
            TestEntry::stored("Data/a.dds.01", b"x"),
            TestEntry::stored("Data/a.dds.4294967296", b"x"),
        ],
        |archive| {
            let keys = TextureMemberKeys::try_new_with(archive, |_| Ok::<_, io::Error>(()))?;
            let no_keys: [(u64, u32); 0] = [];
            assert_eq!(keys.keys, no_keys);
            assert_eq!(keys.keys.capacity(), 0);
            assert_eq!(crate::texture_role("Data/a.dds.0"), None);
            assert_eq!(crate::texture_role("Data/a.dds.01"), None);
            assert_eq!(crate::texture_role("Data/a.dds.4294967296"), None);
            Ok(())
        },
    )
}

fn with_entries(
    entries: impl IntoIterator<Item = TestEntry>,
    run: impl FnOnce(&Archive) -> TestResult,
) -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("test.p4k");
    let mut builder = ArchiveBuilder::new();
    for entry in entries {
        builder = builder.entry(entry);
    }
    builder.write_to(&path)?;
    let archive = Archive::open(path)?;
    run(&archive)
}
