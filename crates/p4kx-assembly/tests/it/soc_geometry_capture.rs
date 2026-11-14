// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    SocContainerCapture, SocContainerOccurrence, SocGeometryCandidateKind, SocGeometryReference,
    SocGeometryTarget, SocGeometryTier,
};
use p4kx_extraction::{Archive, CancelToken, EntryOrigin, EntryOriginStep};
use p4kx_testkit::{
    ArchiveBuilder, SOC_IDENTITY, TestContainer, TestEntry, TestObjectContainer, TestSoc,
};

type Check = Result<(), Box<dyn std::error::Error>>;

const ROOT: &str = "Data/ObjectContainers/root.socpak";
const SHARED: &str = "Data/ObjectContainers/shared.socpak";
const OWNER_PATH: &str = "Objects/Test/owner.cga";
const LOCAL_PATH: &str = "Objects/Test/local.cga";
const ROOT_PATH: &str = "Objects/Test/outer.cga";
const DUPLICATE: &str = "Objects/Test/duplicate.cga";
const MISSING: &str = "Objects/Test/missing.cga";

#[test]
fn physical_model_lookup_uses_the_logical_parent_chain_and_keeps_same_tier_duplicates() -> Check {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("geometry.p4k");
    fixture().write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == ROOT)
        .ok_or("root Entry")?;
    let container = archive.open_container(entry)?;
    let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
    let [_, branch_a, shared_a, branch_b, shared_b] = capture.occurrences() else {
        return Err("root and two parent-child pairs".into());
    };
    assert!(
        shared_a
            .origin
            .binding()
            .is_same_entry(&shared_b.origin.binding()),
        "the globally stored child is physically shared"
    );
    for (branch, shared) in [(branch_a, shared_a), (branch_b, shared_b)] {
        verify_occurrence(branch, shared)?;
    }
    let left = only_owner_slot(shared_a)?;
    let right = only_owner_slot(shared_b)?;
    let (
        SocGeometryTarget::Matched { entry: left, .. },
        SocGeometryTarget::Matched { entry: right, .. },
    ) = (&left.target, &right.target)
    else {
        return Err("resolved model slots in the different logical branches".into());
    };
    assert!(
        !left.binding().is_same_entry(&right.binding()),
        "identical spelling selects physically distinct container Entries"
    );
    Ok(())
}

fn fixture() -> ArchiveBuilder {
    let root = TestContainer::new()
        .entry(descriptor("root", &["branch_a", "branch_b"]))
        .container("branch_a.socpak", &parent("branch_a"))
        .container("branch_b.socpak", &parent("branch_b"));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, root.build()))
        .entry(TestEntry::stored(SHARED, shared().build()))
        .entry(model("Data", OWNER_PATH))
        .entry(model("Data", LOCAL_PATH))
        .entry(model("Data", DUPLICATE))
        .entry(model("Data", ROOT_PATH))
}

fn parent(stem: &str) -> TestContainer {
    TestContainer::new()
        .entry(descriptor(stem, &["shared"]))
        .entry(model("Data", OWNER_PATH))
}

fn shared() -> TestContainer {
    let descriptor = TestObjectContainer::new("shared")
        .stream("scene.soc")
        .stream("scene.soc");
    let soc = TestSoc::new()
        .model(OWNER_PATH)
        .model(LOCAL_PATH)
        .model(ROOT_PATH)
        .model(DUPLICATE)
        .model(MISSING)
        .model("objects/../escape.cga")
        .object(0, SOC_IDENTITY);
    TestContainer::new()
        .entry(TestEntry::stored("shared.xml", descriptor.xml()))
        .entry(TestEntry::stored("scene.soc", soc.bytes()))
        .entry(model("", LOCAL_PATH))
        .entry(model("Data", DUPLICATE))
        .entry(model("Data", &DUPLICATE.to_ascii_uppercase()))
}

fn descriptor(stem: &str, children: &[&str]) -> TestEntry {
    let descriptor = children
        .iter()
        .fold(TestObjectContainer::new(stem), |descriptor, child| {
            descriptor.child(child, [0.0; 3], [1.0, 0.0, 0.0, 0.0])
        });
    TestEntry::stored(format!("{stem}.xml"), descriptor.xml())
}

fn model(prefix: &str, path: &str) -> TestEntry {
    let path = if prefix.is_empty() {
        path.to_owned()
    } else {
        format!("{prefix}/{path}")
    };
    TestEntry::deflated(path, b"unread synthetic target payload").with_payload([0; 8])
}

fn only_owner_slot(
    occurrence: &SocContainerOccurrence,
) -> Result<&SocGeometryReference, &'static str> {
    occurrence
        .streams
        .first()
        .and_then(|stream| stream.geometry.first())
        .ok_or("owner slot")
}

fn verify_occurrence(
    parent: &SocContainerOccurrence,
    occurrence: &SocContainerOccurrence,
) -> Check {
    let [stream, repeated] = occurrence.streams.as_slice() else {
        return Err("both logical stream declarations".into());
    };
    assert!(
        Arc::ptr_eq(&stream.geometry, &repeated.geometry),
        "repeat stream reads share one lookup within an occurrence"
    );
    let [owner, local, root, duplicate, missing, unsafe_path] = stream.geometry.as_ref() else {
        return Err("all original model-path slots".into());
    };
    assert_eq!(owner.chunk_ordinal, 1, "original raw chunk ordinal");
    assert_eq!(root.model_ordinal, 2, "original model slot order");
    assert_eq!(
        owner.source_range.end - owner.source_range.start,
        256,
        "physical path slot width"
    );
    verify_target(
        owner,
        1,
        SocGeometryCandidateKind::DataPrefixed,
        Some(parent),
    )?;
    verify_target(
        local,
        0,
        SocGeometryCandidateKind::Relative,
        Some(occurrence),
    )?;
    verify_target(root, 3, SocGeometryCandidateKind::DataPrefixed, None)?;
    verify_ambiguous(duplicate, occurrence)?;
    assert!(
        matches!(missing.target, SocGeometryTarget::Missing),
        "explicit missing state"
    );
    assert!(
        matches!(unsafe_path.target, SocGeometryTarget::InvalidSelector),
        "unsafe path stays visible"
    );
    Ok(())
}

fn verify_target(
    row: &SocGeometryReference,
    depth: usize,
    kind: SocGeometryCandidateKind,
    owner: Option<&SocContainerOccurrence>,
) -> Check {
    let SocGeometryTarget::Matched { tier, entry } = &row.target else {
        return Err("one directory match".into());
    };
    assert_eq!(
        (tier.depth, tier.kind),
        (depth, kind),
        "winning tier mirrors model provider order"
    );
    verify_owner(entry, owner);
    Ok(())
}

fn verify_owner(entry: &EntryOrigin, owner: Option<&SocContainerOccurrence>) {
    let nearest = entry
        .ancestors()
        .last()
        .flatten()
        .map(EntryOriginStep::ordinal);
    assert_eq!(
        nearest,
        owner.map(|owner| owner.origin.entry().ordinal()),
        "physical nearest directory ancestry"
    );
}

fn verify_ambiguous(row: &SocGeometryReference, owner: &SocContainerOccurrence) -> Check {
    let SocGeometryTarget::Ambiguous {
        tier: SocGeometryTier { depth, kind, .. },
        entries,
    } = &row.target
    else {
        return Err("both physical duplicates retained".into());
    };
    assert_eq!(
        (*depth, *kind),
        (0, SocGeometryCandidateKind::DataPrefixed),
        "parent fallback stays behind ambiguous local tier"
    );
    let [left, right] = entries.as_slice() else {
        return Err("exactly two local duplicates".into());
    };
    assert!(
        left.entry().ordinal() < right.entry().ordinal(),
        "same-tier candidates in physical order"
    );
    for entry in entries {
        verify_owner(entry, Some(owner));
    }
    Ok(())
}
