// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocCaptureError, SocChildState, SocContainerCapture, SocMemberSelection};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_model::{SocEntityFile, SocSourceValue};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestContainer, TestElement,
    TestEntry,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROOT: &str = "Data/ObjectContainers/root.socpak";
const CHILD: &str = "Data/ObjectContainers/shared.socpak";
const ROOM: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
const FIRST_OVERRIDE_PATH: &str = "root/metadata/1001/entitycomponententityreferenceoverride.xml";
const SECOND_OVERRIDE_PATH: &str =
    "root/deep/METADATA/1002/EntityComponentEntityReferenceOverride.XML";
const UNRELATED_OVERRIDE_PATH: &str =
    "root/metadata/1003/old_entitycomponententityreferenceoverride.xml";
const BROKEN_OVERRIDE_PATH: &str = "root/metadata/1004/entitycomponententityreferenceoverride.xml";
const SHARED_OVERRIDE_PATH: &str =
    "shared/metadata/1001/entitycomponententityreferenceoverride.xml";

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
        .entry(TestEntry::stored("DUPLICATE.soc", b"two"))
        .entry(TestEntry::stored(
            SHARED_OVERRIDE_PATH,
            override_xml("shared"),
        ));
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
    assert!(
        root.entity_reference_overrides.is_empty(),
        "another container's companion is outside the root occurrence"
    );
    let [first_member] = first.entity_reference_overrides.as_slice() else {
        return Err("expected one captured member in the first child".into());
    };
    let [second_member] = second.entity_reference_overrides.as_slice() else {
        return Err("expected one captured member in the second child".into());
    };
    assert!(
        first_member
            .origin()
            .binding()
            .is_same_entry(&second_member.origin().binding()),
        "logical occurrences retain the same physical member identity"
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

#[test]
fn authored_soc_capture_retains_override_members_in_physical_order() -> TestResult {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("overrides.p4k");
    let first = override_xml("first");
    let second = override_xml("second");
    let repeated = override_xml("repeated path");
    let container = TestContainer::new()
        .entry(TestEntry::stored("root.xml", b"<ObjectContainer/>"))
        .entry(TestEntry::stored(FIRST_OVERRIDE_PATH, first.as_slice()))
        .entry(TestEntry::stored(UNRELATED_OVERRIDE_PATH, b"unrelated"))
        .entry(TestEntry::stored(SECOND_OVERRIDE_PATH, second.as_slice()))
        .entry(TestEntry::stored(FIRST_OVERRIDE_PATH, repeated.as_slice()))
        .entry(TestEntry::stored(BROKEN_OVERRIDE_PATH, b"bad checksum").with_crc32(1))
        .entry(TestEntry::stored(
            "root/other/1005/entitycomponententityreferenceoverride.xml",
            b"other directory role",
        ))
        .entry(TestEntry::stored(
            "root/metadata/not-decimal/entitycomponententityreferenceoverride.xml",
            b"other slot grammar",
        ));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, container.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("root fixture Entry")?;
    let container = archive.open_container(entry)?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    let [occurrence] = capture.occurrences() else {
        return Err("one container occurrence".into());
    };
    verify_override_members(
        &container,
        &occurrence.entity_reference_overrides,
        [first.as_slice(), second.as_slice(), repeated.as_slice()],
    )
}

fn verify_override_members(
    container: &Archive,
    members: &[p4kx_assembly::SocArchiveMember],
    [first, second, repeated]: [&[u8]; 3],
) -> TestResult {
    let [first_member, second_member, repeated_member, broken] = members else {
        return Err("four physical override members".into());
    };
    for (member, index, expected) in [
        (first_member, 1, first),
        (second_member, 3, second),
        (repeated_member, 4, repeated),
    ] {
        assert_eq!(member.origin().entry().index(), index, "physical order");
        assert!(
            container.owns_origin(member.origin()),
            "container owns the physical source"
        );
        assert_eq!(
            member.payload().map_err(|_| "verified CryXmlB payload")?,
            expected,
            "full original CryXmlB tables are retained"
        );
    }
    assert_eq!(
        broken.origin().entry().index(),
        5,
        "failed read keeps its physical position"
    );
    assert!(
        broken.payload().is_err(),
        "CRC verification refuses the corrupted member"
    );
    assert_eq!(
        first_member.origin().entry().path(),
        repeated_member.origin().entry().path(),
        "fixture repeats the exact archive path"
    );
    assert!(
        !first_member
            .origin()
            .binding()
            .is_same_entry(&repeated_member.origin().binding()),
        "matching paths keep separate physical identities"
    );
    Ok(())
}

fn override_xml(target: &str) -> Vec<u8> {
    let row = TestElement::new("AutoSpawnOverrideElement")
        .attribute("AutoSpawnIdx", "0")
        .attribute("Target", target)
        .attribute("AA", ROOM);
    let group = TestElement::new("AutoSpawnOverrides").child(row);
    let entity = TestElement::new("Entity")
        .attribute("CryGUID", ROOM)
        .child(group);
    TestElement::new("EntityComponentEntityReferenceOverride")
        .child(entity)
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}
