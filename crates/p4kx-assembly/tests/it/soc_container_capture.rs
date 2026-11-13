// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocCaptureError, SocChildState, SocContainerCapture, SocMemberSelection};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_model::{SocEntityFile, SocSourceValue};
use p4kx_testkit::{ArchiveBuilder, TestChunkFile, TestContainer, TestEntry};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROOT: &str = "Data/ObjectContainers/root.socpak";
const CHILD: &str = "Data/ObjectContainers/shared.socpak";
const ROOM: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";

fn archive_fixture() -> ArchiveBuilder {
    let root_descriptor = br#"<ObjectContainer><ChildObjectContainers>
        <Child name="shared.socpak" pos="1,0,0"/>
        <Child name="shared.socpak" pos="2,0,0"/>
        <Child/><Child name="absent.socpak"/><Child name="duplicate.socpak"/>
    </ChildObjectContainers></ObjectContainer>"#;
    let child_descriptor = br#"<ObjectContainer><ChildObjectContainers>
        <Child name="root.socpak"/>
    </ChildObjectContainers><StreamingOCs>
        <OC name="placed.soc"/><OC name="absent.soc"/><OC name="duplicate.soc"/><OC/>
    </StreamingOCs></ObjectContainer>"#;
    let root = TestContainer::new().entry(TestEntry::stored("root.xml", root_descriptor));
    let entity = format!(r#"<Entities><Entity EntityCryGUID="{ROOM}"/></Entities>"#);
    let stream = TestChunkFile::crch()
        .chunk(0x0004, 3, entity.into_bytes())
        .bytes();
    let registry =
        format!(r#"<EntityComponentRoom><Entity CryGUID="{ROOM}"/></EntityComponentRoom>"#);
    let shared = TestContainer::new()
        .entry(TestEntry::stored("shared.xml", child_descriptor))
        .entry(TestEntry::stored(
            "shared/metadata/entitycomponentroom.xml",
            registry,
        ))
        .entry(TestEntry::stored("placed.soc", stream))
        .entry(TestEntry::stored("duplicate.soc", b"one"))
        .entry(TestEntry::stored("DUPLICATE.soc", b"two"));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, root.build()))
        .entry(TestEntry::stored(CHILD, shared.build()))
        .entry(TestEntry::stored(
            "Data/ObjectContainers/duplicate.socpak",
            b"one",
        ))
        .entry(TestEntry::stored(
            "Data/ObjectContainers/DUPLICATE.socpak",
            b"two",
        ))
}

#[test]
fn authored_soc_capture_separates_logical_occurrences_from_shared_entry_identity() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("authored-soc.p4k");
    archive_fixture().write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("root fixture Entry")?;
    let container = archive.open_container(entry)?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    let [root, first, second] = capture.occurrences() else {
        return Err("expected root and two distinct child occurrences".into());
    };
    assert_eq!(
        [root.placement, first.placement, second.placement],
        [None, Some((0, 0)), Some((0, 1))],
        "placements remain distinct"
    );
    assert!(
        first
            .origin
            .binding()
            .is_same_entry(&second.origin.binding()),
        "both occurrences own the same physical source"
    );
    assert!(
        matches!(root.registry, SocMemberSelection::Missing),
        "no room registry in this owner"
    );
    verify_children(root)?;
    for occurrence in [first, second] {
        assert!(
            matches!(occurrence.children.as_slice(), [child] if matches!(child.state, SocChildState::Cycle(_))),
            "cycles stop at the active physical root"
        );
        verify_streams(occurrence)?;
    }
    Ok(())
}

fn verify_children(root: &p4kx_assembly::SocContainerOccurrence) -> TestResult {
    let [first, second, absent, missing, duplicate] = root.children.as_slice() else {
        return Err("expected all five original child declarations".into());
    };
    assert!(
        matches!(first.state, SocChildState::Opened { occurrence: 1 }),
        "first physical declaration"
    );
    assert!(
        matches!(second.state, SocChildState::Opened { occurrence: 2 }),
        "second physical declaration"
    );
    assert!(
        matches!(absent.state, SocChildState::InvalidSelector),
        "omitted selector retained"
    );
    assert_eq!(
        absent.selector.name,
        SocSourceValue::Absent,
        "missing name value"
    );
    assert!(
        matches!(missing.state, SocChildState::Missing),
        "unmatched selector"
    );
    assert!(
        matches!(&duplicate.state, SocChildState::Ambiguous(matches) if matches.len() == 2),
        "every duplicate physical member"
    );
    Ok(())
}

fn verify_streams(occurrence: &p4kx_assembly::SocContainerOccurrence) -> TestResult {
    assert!(
        occurrence.registry.selected().is_some(),
        "nested metadata member"
    );
    let [stream, missing, duplicate, omitted] = occurrence.streams.as_slice() else {
        return Err("expected all four original stream declarations".into());
    };
    assert!(
        matches!(missing.member, Some(SocMemberSelection::Missing)),
        "unmatched stream selector"
    );
    assert!(
        matches!(&duplicate.member, Some(SocMemberSelection::Ambiguous(matches)) if matches.len() == 2),
        "duplicate stream matches"
    );
    assert!(
        omitted.member.is_none(),
        "omitted stream selector remains distinct"
    );
    let member = stream
        .member
        .as_ref()
        .and_then(SocMemberSelection::selected)
        .ok_or("selected physical stream")?;
    let source = SocEntityFile::parse(member.payload().map_err(|_| "verified stream bytes")?)?;
    let chunk = source
        .entity_chunks()
        .next()
        .ok_or("entity source chunk")??;
    assert!(
        chunk
            .entities()
            .next()
            .is_some_and(|entity| entity.guid_attributes().map(|attr| attr.value()).eq([ROOM])),
        "same local identity remains in each occurrence"
    );
    Ok(())
}

#[test]
fn authored_soc_capture_refuses_foreign_archive_context_and_observes_cancellation() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("authored-soc.p4k");
    archive_fixture().write_to(&path)?;
    let archive = Archive::open(&path)?;
    let foreign = Archive::open(path)?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("root fixture Entry")?;
    let container = archive.open_container(entry)?;
    let cancel = CancelToken::new();
    assert!(
        matches!(
            SocContainerCapture::read(&[&foreign, &container], &cancel),
            Err(SocCaptureError::Root)
        ),
        "same bytes cannot supply another archive's origin"
    );
    cancel.cancel();
    assert!(
        matches!(
            SocContainerCapture::read(&[&archive, &container], &cancel),
            Err(SocCaptureError::Cancelled)
        ),
        "cancellation precedes traversal"
    );
    Ok(())
}
