// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::animation_headers::AnimationHeaderFault;
use p4kx_extraction::{Archive, DataForgeEntry, Entry, OpenOptions, SourceEncryption};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAnimation, TestDba, TestEntity, TestEntry, TestLoadout,
    TestLoadoutEntry, TestModel, TestSkeleton, TestTrack, entity_schema, test_key,
};

use super::{
    ClipArchiveSource, ClipDatabaseUnavailable, ClipSourceSelection, EntityClipCatalogue,
    discover_entity_clips,
};
use crate::{ArchiveModels, LoadoutResolver, RigFileRead};

#[path = "entity_clip_named_caf_tests.rs"]
mod named_caf;

type TestResult = Result<(), Box<dyn Error>>;

const ROOT_PARAMS: &str = "Data/Objects/Test/root.chrparams";
const CHILD_PARAMS: &str = "Data/Objects/Test/child.chrparams";
const DBA: &str = "Data/Animations/Db/packed.dba";
const CAF: &str = "Data/Animations/Loose/sub/repeated.caf";
const PROTECTED: &str = "Data/Animations/Loose/keyed.caf";

#[test]
fn original_headers_remain_distinct_and_a_missing_rig_does_not_hide_other_sources() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("animations.p4k");
    synthetic_archive().write_to(&path)?;
    let archive = Archive::open_with(&path, OpenOptions::new().key(Some(test_key())))?;
    let catalogue = inventory(&archive)?;
    verify_links(&catalogue)?;
    verify_headers(&catalogue)?;
    verify_block_faults(&catalogue)?;
    named_caf::verify_named_caf(&catalogue)?;

    let without_key = Archive::open(&path)?;
    let catalogue = inventory(&without_key)?;
    let protected = by_path(&catalogue, PROTECTED)?;
    assert_eq!(
        without_key
            .entries()
            .get(protected.archive_ordinal)
            .map(Entry::path),
        Some(PROTECTED),
        "an unread payload retains the original archive entry coordinate"
    );
    assert!(
        matches!(protected.file.read.as_ref(), RigFileRead::KeyRequired),
        "retain unread input row"
    );
    assert_eq!(
        protected.file.encryption(),
        SourceEncryption::Unknown,
        "unread payload contributes no origin"
    );
    assert!(
        by_path(&catalogue, CAF)?.file.value().is_some(),
        "plain CAF remains accessible"
    );
    assert!(
        by_path(&catalogue, DBA)?.file.value().is_some(),
        "plain DBA remains accessible"
    );
    Ok(())
}

fn inventory(archive: &Archive) -> Result<EntityClipCatalogue, Box<dyn Error>> {
    let forge_entry = DataForgeEntry::read(archive)?;
    let forge = forge_entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&forge_entry);
    let root = resolver.entities().find("RigRoot")?;
    let tree = resolver.resolve(&root)?;
    Ok(discover_entity_clips(&ArchiveModels::new(archive), &tree))
}

fn verify_links(catalogue: &EntityClipCatalogue) -> TestResult {
    let [root, child] = catalogue.rigs.as_slice() else {
        return Err("ordered root/child placements".into());
    };
    assert!(
        matches!(
            root.rig.skeleton.as_ref().map(|file| file.read.as_ref()),
            Some(RigFileRead::Missing)
        ),
        "root skeleton remains explicitly missing"
    );
    let [missing, folder] = root.links.as_slice() else {
        return Err("direct missing DBA and one folder".into());
    };
    assert!(
        matches!(
            &missing.selection,
            ClipSourceSelection::DatabaseUnavailable {
                reason: ClipDatabaseUnavailable::Missing
            }
        ),
        "a missing requested DBA is explicit"
    );
    assert_eq!(missing.parameter, 0, "original parameter occurrence");
    let ClipSourceSelection::Folder {
        sources: root_files,
    } = &folder.selection
    else {
        return Err("root folder candidates".into());
    };
    verify_child_links(catalogue, &child.links, root_files)
}

fn verify_child_links(
    catalogue: &EntityClipCatalogue,
    links: &[super::ClipSourceLink],
    root_files: &[usize],
) -> TestResult {
    let [child_folder, child_database, ..] = links else {
        return Err("the wildcard remains a declaration before the database link".into());
    };
    assert_eq!(
        child_database.parameter, 2,
        "intervening wildcard does not change authored position"
    );
    let ClipSourceSelection::Folder {
        sources: child_files,
    } = &child_folder.selection
    else {
        return Err("child folder candidates".into());
    };
    assert_eq!(
        child_files.as_ref(),
        root_files,
        "equivalent folder requests share physical candidate rows"
    );
    assert_eq!(
        child_files.len(),
        2,
        "includes recursive plain and protected loose CAFs"
    );
    let ClipSourceSelection::Database { source } = child_database.selection else {
        return Err("one actual physical database".into());
    };
    assert_eq!(
        catalogue
            .sources
            .get(source)
            .map(|source| source.file.path.as_str()),
        Some(DBA),
        "database linked directly"
    );
    assert_eq!(
        catalogue.sources.len(),
        3,
        "each physical Entry inventoried once"
    );
    Ok(())
}

fn verify_headers(catalogue: &EntityClipCatalogue) -> TestResult {
    let caf = by_path(catalogue, CAF)?.file.value().ok_or("loose CAF")?;
    let dba = by_path(catalogue, DBA)?
        .file
        .value()
        .ok_or("tracks database")?;
    let [caf] = caf.clips.as_slice() else {
        return Err("one CAF source clip".into());
    };
    let [dba_first, dba_second] = dba.clips.as_slice() else {
        return Err("both original DBA slots".into());
    };
    assert_eq!(
        dba_first
            .database_name
            .as_ref()
            .map(|name| name.bytes.as_slice()),
        Some(b"repeated".as_slice()),
        "original DBA name bytes"
    );
    assert_eq!(
        dba_second
            .database_name
            .as_ref()
            .map(|name| name.bytes.as_slice()),
        Some(b"repeated".as_slice()),
        "same display name remains a separate declaration"
    );
    assert_ne!(
        dba_first.metadata_range, dba_second.metadata_range,
        "physical DBA coordinates differ"
    );
    assert_eq!(dba_second.ordinal, 1, "original second slot");
    assert_eq!(
        caf.timing
            .as_ref()
            .map(|timing| timing.frames_per_second)
            .ok(),
        Some(24),
        "CAF clock"
    );
    assert_eq!(
        dba_first
            .timing
            .as_ref()
            .map(|timing| timing.frames_per_second)
            .ok(),
        Some(30),
        "first original DBA clock"
    );
    assert_eq!(
        dba_second
            .timing
            .as_ref()
            .map(|timing| timing.frames_per_second)
            .ok(),
        Some(60),
        "second original DBA clock"
    );
    Ok(())
}

fn verify_block_faults(catalogue: &EntityClipCatalogue) -> TestResult {
    let caf = by_path(catalogue, CAF)?.file.value().ok_or("loose CAF")?;
    let dba = by_path(catalogue, DBA)?
        .file
        .value()
        .ok_or("tracks database")?;
    let ([caf], [dba_first, dba_second]) = (caf.clips.as_slice(), dba.clips.as_slice()) else {
        return Err("original CAF and DBA declarations".into());
    };
    assert!(
        dba_first.controller_block.is_some()
            && dba_second.controller_block.is_none()
            && matches!(dba.partial, Some(AnimationHeaderFault::Structure(_))),
        "retain independently recoverable second metadata after controller failure"
    );
    assert!(
        matches!(caf.controller_block.as_ref().map(|block| block.controllers.as_slice()), Some([controller]) if controller.id == 41),
        "header captures the actual controller occurrence"
    );
    assert_eq!(
        by_path(catalogue, PROTECTED)?.file.encryption(),
        SourceEncryption::Encrypted,
        "keyed successful payload proof"
    );
    Ok(())
}

fn by_path<'a>(
    catalogue: &'a EntityClipCatalogue,
    path: &str,
) -> Result<&'a ClipArchiveSource, Box<dyn Error>> {
    catalogue
        .sources
        .iter()
        .find(|source| source.file.path == path)
        .ok_or_else(|| "physical file row missing".into())
}

fn synthetic_archive() -> ArchiveBuilder {
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .entry(TestEntry::stored(
            "Data/Objects/Test/child.chr",
            TestModel::cube()
                .with_skeleton(&TestSkeleton::two_bones())
                .cgf(),
        ))
        .entry(TestEntry::stored(
            ROOT_PARAMS,
            r##"<Params><AnimationList>
<Animation name="$TracksDatabase" path="Animations/Db/absent.dba"/>
<Animation name="#filepath" path="Animations/Loose"/>
</AnimationList></Params>"##,
        ))
        .entry(TestEntry::stored(
            CHILD_PARAMS,
            r##"<Params><AnimationList>
<Animation name="#filepath" path="Data/Animations/Loose/"/>
<Animation name="*" path="*/*.caf"/>
<Animation name="$TracksDatabase" path="Animations/Db/packed.dba"/>
<Animation name="relative" path="sub/repeated.caf"/>
<Animation name="rooted" path="Animations/Loose/sub/repeated.caf"/>
<Animation name="missing" path="absent.caf"/>
<Animation name="parentTraversal" path="../repeated.caf"/>
<Animation name="ambiguous" path="Data/Animations/Duplicate/test.caf"/>
</AnimationList><AnimationList>
<Animation name="beforeScope" path="sub/repeated.caf"/>
</AnimationList></Params>"##,
        ))
        .entry(TestEntry::stored(
            CAF,
            TestAnimation::new("repeated", 24).track(track(41)).caf(),
        ))
        .entry(TestEntry::stored(PROTECTED, TestAnimation::new("keyed", 50).caf()).encrypted())
        .entry(TestEntry::stored(DBA, broken_dba()))
        .entry(TestEntry::stored(
            "Data/Animations/Duplicate/test.caf",
            TestAnimation::new("duplicate", 25).caf(),
        ))
        .entry(TestEntry::stored(
            "Data/Animations/Duplicate/test.caf",
            TestAnimation::new("duplicate", 60).caf(),
        ))
}

fn database() -> Vec<u8> {
    let root = TestEntity::new(1, "RigRoot", "entities/rig_root.xml")
        .geometry("Objects/Test/root.chr", None)
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "port", "RigChild",
        )]));
    let child = TestEntity::new(2, "RigChild", "entities/rig_child.xml")
        .geometry("Objects/Test/child.chr", None);
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .build()
}

fn track(controller: u32) -> TestTrack {
    TestTrack::new(controller).positions(&[(0, [0.0, 0.0, 0.0]), (12, [1.0, 0.0, 0.0])])
}

fn broken_dba() -> Vec<u8> {
    let mut bytes = TestDba::new()
        .animation(TestAnimation::new("repeated", 30).track(track(10)))
        .animation(TestAnimation::new("repeated", 60).track(track(20)))
        .bytes();
    let second = bytes
        .windows(4)
        .enumerate()
        .filter(|(_, word)| *word == b"#dba")
        .nth(1)
        .map(|(at, _)| at)
        .expect("second synthetic block exists");
    *bytes
        .get_mut(second)
        .expect("second synthetic block starts in input") = b'!';
    bytes
}
