// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Finding prefabs and libraries: dotted names, GUIDs, library names and unreadable libraries.

use p4kx_assembly::{
    MAX_PREFAB_ISSUES, PrefabError, PrefabIssueKind, PrefabLookupRoute, PrefabSet,
};
use p4kx_testkit::{TestEntry, TestPrefabLibrary, TestPrefabObject, prefab_guid};

use crate::assembly_support::TestResult;
use crate::prefab::{IDENTITY, brush, kinds, nested, resolve, with_archive};

#[test]
fn a_prefab_name_may_contain_dots_and_the_guid_is_checked() -> TestResult {
    let library = TestPrefabLibrary::new("test/rooms")
        .prefab(
            "hall",
            vec![
                nested("good", "visual.flare", [0.0; 3], IDENTITY),
                nested("stale", "visual.flare", [0.0; 3], IDENTITY)
                    .attribute("PrefabGUID", "{00000000-0000-0000-0000-000000000001}"),
            ],
        )
        .prefab("visual.flare", vec![brush("glow", [0.0; 3])]);
    let resolved = resolve(&library, "HALL")?;
    assert_eq!(resolved.objects.len(), 2);
    assert_eq!(
        kinds(&resolved),
        [&PrefabIssueKind::GuidMismatch {
            prefab: "test/rooms.visual.flare".to_owned(),
            named: "{00000000-0000-0000-0000-000000000001}".to_owned(),
            found: prefab_guid("test/rooms", "visual.flare"),
        }]
    );
    Ok(())
}

#[test]
fn a_prefab_that_moved_is_found_by_its_guid() -> TestResult {
    let rooms = TestPrefabLibrary::new("test/rooms").prefab(
        "hall",
        vec![
            TestPrefabObject::nested("moved", "test/props", "part", [2.0, 0.0, 0.0], IDENTITY)
                .attribute("PrefabName", "gone/library.part"),
            TestPrefabObject::nested("wrong", "test/props", "part", [3.0, 0.0, 0.0], IDENTITY)
                .attribute("PrefabName", "test/rooms.hall"),
        ],
    );
    let props =
        TestPrefabLibrary::new("test/props").prefab("part", vec![brush("bolt", [1.0, 0.0, 0.0])]);
    let resolved = with_archive(vec![rooms.entry(), props.entry()], |archive| {
        Ok(PrefabSet::new(archive).resolve("test/rooms", "hall")?)
    })?;
    assert_eq!(resolved.issues, []);
    let bolts: Vec<[f32; 3]> = resolved
        .objects
        .iter()
        .map(|o| o.world().translation())
        .collect();
    assert_eq!(bolts, [[3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    Ok(())
}

#[test]
fn libraries_are_listed_and_found_by_name_or_path_ignoring_case() -> TestResult {
    let rooms = TestPrefabLibrary::new("test/Rooms").prefab("hall", vec![brush("wall", [0.0; 3])]);
    let props = TestPrefabLibrary::new("test/props").prefab("a", vec![]);
    with_archive(vec![props.entry(), rooms.entry()], |archive| {
        let mut set = PrefabSet::new(archive);
        assert_eq!(
            set.libraries().collect::<Vec<_>>(),
            ["test/props", "test/Rooms"]
        );
        for name in [
            "test/rooms",
            "TEST\\ROOMS",
            "Data\\Prefabs\\test\\rooms.xml",
        ] {
            let file = set.library(name)?;
            assert_eq!(file.library().prefabs.len(), 1);
        }
        assert!(matches!(
            set.library("test/none"),
            Err(PrefabError::NoSuchLibrary { .. })
        ));
        assert!(matches!(
            set.resolve("test/rooms", "none"),
            Err(PrefabError::NoSuchPrefab { .. })
        ));
        Ok(())
    })
}

#[test]
fn a_library_that_cannot_be_read_is_an_error_and_an_issue_for_its_users() -> TestResult {
    let rooms = TestPrefabLibrary::new("test/rooms").prefab(
        "hall",
        vec![TestPrefabObject::nested(
            "x",
            "test/broken",
            "p",
            [0.0; 3],
            IDENTITY,
        )],
    );
    let broken = TestEntry::stored(
        "Data\\Prefabs\\test\\broken.xml",
        b"<ObjectContainer/>".to_vec(),
    );
    with_archive(vec![rooms.entry(), broken], |archive| {
        let mut set = PrefabSet::new(archive);
        assert!(matches!(
            set.library("test/broken"),
            Err(PrefabError::Unreadable { .. })
        ));
        let resolved = set.resolve("test/rooms", "hall")?;
        assert_eq!(resolved.objects.len(), 1);
        assert!(matches!(
            kinds(&resolved)[..],
            [
                PrefabIssueKind::LibraryUnreadable { .. },
                PrefabIssueKind::Missing { .. }
            ]
        ));
        Ok(())
    })
}

#[test]
fn a_guid_mismatch_is_reported_once_however_often_the_prefab_is_expanded() -> TestResult {
    let stale = "{00000000-0000-0000-0000-00000000000B}";
    let library = TestPrefabLibrary::new("test/rooms")
        .prefab("target", vec![brush("bolt", [0.0; 3])])
        .prefab(
            "hall",
            (0..20)
                .map(|n| {
                    nested(&format!("n{n}"), "target", [0.0; 3], IDENTITY)
                        .attribute("PrefabGUID", stale)
                })
                .collect(),
        )
        .prefab(
            "top",
            (0..20)
                .map(|n| nested(&format!("h{n}"), "hall", [0.0; 3], IDENTITY))
                .collect(),
        );
    let resolved = resolve(&library, "top")?;
    assert_eq!(resolved.objects.len(), 400);
    assert_eq!(kinds(&resolved).len(), 1);
    Ok(())
}

#[test]
fn the_issue_list_is_bounded() -> TestResult {
    let library = TestPrefabLibrary::new("test/rooms")
        .prefab(
            "hall",
            (0..300)
                .map(|n| nested(&format!("g{n}"), "nothing", [0.0; 3], IDENTITY))
                .collect(),
        )
        .prefab(
            "top",
            (0..20)
                .map(|n| nested(&format!("h{n}"), "hall", [0.0; 3], IDENTITY))
                .collect(),
        );
    let resolved = resolve(&library, "top")?;
    assert_eq!(resolved.issues.len(), MAX_PREFAB_ISSUES + 1);
    assert_eq!(
        resolved.issues.last().map(|issue| &issue.kind),
        Some(&PrefabIssueKind::TooManyIssues {
            limit: MAX_PREFAB_ISSUES
        })
    );
    assert_eq!(resolved.file, "test/rooms");
    Ok(())
}

#[test]
fn provenance_retains_actual_guid_selection_instance_origins_and_empty_visits() -> TestResult {
    let rooms = TestPrefabLibrary::new("test/rooms").prefab(
        "hall",
        vec![
            TestPrefabObject::nested("moved", "test/props", "part", [2.0, 0.0, 0.0], IDENTITY)
                .attribute("PrefabName", "test/rooms.hall"),
            TestPrefabObject::nested("vacant", "test/props", "empty", [1.0, 0.0, 0.0], IDENTITY),
        ],
    );
    let props = TestPrefabLibrary::new("test/props")
        .prefab("part", vec![brush("bolt", [1.0, 0.0, 0.0])])
        .prefab("empty", Vec::new());
    with_archive(vec![rooms.entry(), props.entry()], |archive| {
        let mut set = PrefabSet::with_object_sources(archive);
        let resolved = set.resolve("test/rooms", "hall")?;
        assert_eq!(resolved.prefabs, 3);
        assert_eq!(resolved.objects.len(), 1);
        let bolt = resolved.objects.first().ok_or("expanded leaf")?;
        assert_eq!(bolt.world().translation(), [3.0, 0.0, 0.0]);
        assert_eq!(bolt.location().prefab.file.file_name(), "test/props");
        assert!(bolt.location().source().is_some());
        let route = bolt.instance_route().ok_or("nested instance evidence")?;
        assert!(route.parent().is_none());
        let selection = route.selection();
        assert_eq!(selection.object().prefab.file.file_name(), "test/rooms");
        assert_eq!(selection.object().object_index, 0);
        assert!(selection.object().source().is_some());
        let link = selection.lookup();
        assert_eq!(link.route(), PrefabLookupRoute::GuidIndex);
        assert_eq!(
            link.compared_name()
                .ok_or("compared name")?
                .file
                .file_name(),
            "test/rooms"
        );
        assert_eq!(
            link.target().ok_or("GUID target")?.file.file_name(),
            "test/props"
        );
        let source = resolved.source().ok_or("root resolution evidence")?;
        assert!(set.source_entry(source.root().file).is_some());
        assert_eq!(source.entered().len(), 2);
        assert_eq!(
            source
                .entered()
                .last()
                .ok_or("empty visit evidence")?
                .lookup()
                .route(),
            PrefabLookupRoute::NameAndGuid
        );
        Ok(())
    })
}

#[test]
fn an_unreadable_target_contributes_a_failed_attempt_without_a_target_payload() -> TestResult {
    let room = TestPrefabLibrary::new("test/room").prefab(
        "hall",
        vec![
            TestPrefabObject::nested("x", "test/broken", "p", [0.0; 3], IDENTITY)
                .attribute("PrefabGUID", ""),
        ],
    );
    let broken = TestEntry::stored(
        "Data\\Prefabs\\test\\broken.xml",
        b"<ObjectContainer/>".to_vec(),
    );
    with_archive(vec![room.entry(), broken], |archive| {
        let index = archive
            .entries()
            .iter()
            .find(|entry| entry.path().ends_with("broken.xml"))
            .ok_or("broken Entry")?
            .index();
        let resolved = PrefabSet::with_object_sources(archive).resolve("test/room", "hall")?;
        let kept = resolved.objects.first().ok_or("unexpanded object")?;
        let lookup = kept.unexpanded_source().ok_or("lookup attempt evidence")?;
        assert_eq!(lookup.route(), PrefabLookupRoute::UnmatchedName);
        assert!(lookup.target().is_none());
        assert_eq!(lookup.failed_entry_indices(), [index]);
        let source = resolved.source().ok_or("resolution evidence")?;
        assert_eq!(source.issues().len(), resolved.issues.len());
        assert_eq!(source.issues().len(), 2);
        assert!(source.issues().iter().all(|issue| {
            issue
                .object()
                .is_some_and(|object| object.object_index == 0)
        }));
        Ok(())
    })
}
