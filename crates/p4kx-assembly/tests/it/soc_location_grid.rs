// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    SocArchiveMember, SocContainerCapture, SocGeometryTarget, SocLocationGridError,
    SocLocationGridSelection, SocLocationGridSource, SocMemberSelection, SocPhysicsVoxelTarget,
};
use p4kx_extraction::{
    Archive, CancelToken, Entry, ObservedEntryBinding, PathIndex, PathKeys, SourceUses,
    SourceValueState,
};
use p4kx_model::{SocContainerDescriptor, SocPhysicsDeclaration};
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, TestObjectContainer, test_key};

type Check<T = ()> = Result<T, Box<dyn std::error::Error>>;

const DESCRIPTOR: &str = "root.xml";
const ROOT: &str = "Data/ObjectContainers/root.socpak";
const SHARED: &str = "Data/ObjectContainers/shared.socpak";
const TARGET: &str = "Data/Objects/Test/Shared.cgf";
const XML: &[u8] = br#"<ObjectContainer gridRadius="2.5">
<PhysicsGridPart shape="voxel">
    <PhysicsGridVoxel filename="Objects\Test\Shared.cgf"/>
    <PhysicsGridVoxel filename="Objects/Test/Duplicate.cgf"/>
    <PhysicsGridVoxel filename="Objects/Test/Missing.cgf"/>
    <PhysicsGridVoxel filename="Objects/../Escape.cgf"/>
    <PhysicsGridVoxel filename=""/>
    <PhysicsGridVoxel/>
</PhysicsGridPart></ObjectContainer>"#;

#[test]
fn voxel_lookup_uses_directory_evidence_and_keeps_source_states() -> Check {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("location-grid.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(DESCRIPTOR, XML).encrypted())
        .entry(unread_target(TARGET))
        .entry(unread_target("Data/Objects/Test/Duplicate.cgf"))
        .entry(unread_target("data/objects/test/DUPLICATE.cgf"))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let member = SocArchiveMember::read(&archive, entry_at(&archive, DESCRIPTOR)?, || Ok(()))?;
    let selection = SocMemberSelection::Selected(member);
    let SocLocationGridSelection::Read(source) = SocLocationGridSelection::read(&selection) else {
        return Err("verified descriptor".into());
    };
    let keys = PathKeys::new(&archive);
    let layers = [keys.index(&archive)];
    check_provenance(&source, &layers)?;
    let cancel = CancelToken::new();
    let part = source
        .physics()
        .declarations()
        .next()
        .ok_or("voxel-bearing part")?;
    check_target_states(
        part.children()
            .map(|node| source.resolve_voxel(node, &layers, &cancel)),
    )
}

fn check_provenance(source: &SocLocationGridSource<'_>, layers: &[PathIndex<'_>]) -> Check {
    let values = SourceUses::new();
    let radius = source.sourced(source.physics().grid_radius(), &values)?;
    assert_eq!(radius.value(), Some(&2.5), "typed radius");
    assert_eq!(
        radius.origin().contains_encrypted_source(),
        Some(true),
        "descriptor payload is encrypted"
    );
    assert_eq!(
        radius.origin().source_uses_refs().len(),
        2,
        "radius element and attribute"
    );
    let part = source.physics().declarations().next().ok_or("part")?;
    let first = part.children().next().ok_or("first voxel")?;
    let observed = source.sourced_voxel(first, layers, &CancelToken::new(), &values)?;
    assert_eq!(
        observed.origin().contains_encrypted_source(),
        Some(true),
        "lookup inherits selector provenance"
    );
    assert_eq!(
        observed.origin().source_uses_refs().len(),
        3,
        "locator element, attribute and target directory path"
    );

    let absent_field = part
        .children()
        .last()
        .and_then(SocPhysicsDeclaration::filename)
        .ok_or("absent field")?;
    let absent = source.sourced(absent_field, &values)?;
    assert_eq!(
        absent.state(),
        SourceValueState::Absent,
        "missing filename has its own state"
    );
    assert_eq!(
        absent.origin().source_uses_refs().len(),
        1,
        "missing filename has only element evidence"
    );
    let foreign = SocContainerDescriptor::parse(XML)?;
    assert!(
        matches!(
            source.sourced(foreign.location_physics().grid_radius(), &values),
            Err(SocLocationGridError::Owner)
        ),
        "identical XML from another document cannot bind to this Entry"
    );
    Ok(())
}

fn check_target_states(
    results: impl Iterator<Item = Result<SocPhysicsVoxelTarget, SocLocationGridError>>,
) -> Check {
    let [matched, ambiguous, missing, unsafe_path, empty, absent] = results
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "six physical voxel occurrences")?;
    let SocPhysicsVoxelTarget::Path(SocGeometryTarget::Matched { entry, .. }) = matched else {
        return Err("unique normalized target".into());
    };
    assert_eq!(
        entry.entry().index(),
        1,
        "exact directory target without body read"
    );
    let SocPhysicsVoxelTarget::Path(SocGeometryTarget::Ambiguous { entries, .. }) = ambiguous
    else {
        return Err("same-tier duplicate target".into());
    };
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry.entry().index())
            .collect::<Vec<_>>(),
        [2, 3],
        "duplicates in physical source order"
    );
    assert!(
        matches!(
            missing,
            SocPhysicsVoxelTarget::Path(SocGeometryTarget::Missing)
        ),
        "valid unmatched locator"
    );
    for target in [unsafe_path, empty] {
        assert!(
            matches!(
                target,
                SocPhysicsVoxelTarget::Path(SocGeometryTarget::InvalidSelector)
            ),
            "malformed locator is invalid"
        );
    }
    assert!(
        matches!(absent, SocPhysicsVoxelTarget::Absent),
        "undeclared filename is absent"
    );
    Ok(())
}

#[test]
fn repeated_placements_select_distinct_targets_for_one_physical_grid() -> Check {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("shared-grid.p4k");
    shared_fixture().write_to(&path)?;
    let archive = Archive::open(path)?;
    let root = archive.open_container(entry_at(&archive, ROOT)?)?;
    let shared = archive.open_container(entry_at(&archive, SHARED)?)?;
    let root_keys = PathKeys::new(&root);
    let shared_keys = PathKeys::new(&shared);
    let archive_keys = PathKeys::new(&archive);
    let capture = SocContainerCapture::read(&[&archive, &root], &CancelToken::new())?;
    let [_, _, left, _, right] = capture.occurrences() else {
        return Err("two logical parent placements with the same child descriptor".into());
    };
    assert_eq!(left.placement, Some((1, 0)), "left logical occurrence");
    assert_eq!(right.placement, Some((3, 0)), "right logical occurrence");
    let branches = [(left, "left.socpak"), (right, "right.socpak")];
    let mut origins = Vec::new();
    for (occurrence, branch_name) in branches {
        let branch = root.open_container(entry_at(&root, branch_name)?)?;
        let keys = PathKeys::new(&branch);
        let layers = [
            shared_keys.index(&shared),
            keys.index(&branch),
            root_keys.index(&root),
            archive_keys.index(&archive),
        ];
        origins.push(branch_origins(&occurrence.descriptor, &layers)?);
    }
    let [left, right] = origins.as_slice() else {
        return Err("both repeated placement routes".into());
    };
    assert!(
        left.descriptor.is_same_entry(&right.descriptor),
        "descriptor is physically shared"
    );
    assert!(
        !left.target.is_same_entry(&right.target),
        "targets belong to their own logical parent"
    );
    Ok(())
}

struct BranchOrigins {
    descriptor: ObservedEntryBinding,
    target: ObservedEntryBinding,
}

fn branch_origins(
    selection: &SocMemberSelection,
    layers: &[PathIndex<'_>],
) -> Check<BranchOrigins> {
    let SocLocationGridSelection::Read(source) = SocLocationGridSelection::read(selection) else {
        return Err("repeated physical grid descriptor".into());
    };
    let voxel = source
        .physics()
        .declarations()
        .next()
        .and_then(|part| part.children().next())
        .ok_or("first physical voxel")?;
    let SocPhysicsVoxelTarget::Path(SocGeometryTarget::Matched { entry, tier }) =
        source.resolve_voxel(voxel, layers, &CancelToken::new())?
    else {
        return Err("voxel reached through the current logical parent".into());
    };
    assert_eq!(tier.depth, 1, "container parent is the winning lookup tier");
    Ok(BranchOrigins {
        descriptor: source.origin().binding(),
        target: entry.binding(),
    })
}

fn shared_fixture() -> ArchiveBuilder {
    let branch = |name| {
        TestContainer::new()
            .entry(with_children(name, &["shared"]))
            .entry(unread_target(TARGET))
    };
    let root = TestContainer::new()
        .entry(with_children("root", &["left", "right"]))
        .container("left.socpak", &branch("left"))
        .container("right.socpak", &branch("right"));
    let shared = TestContainer::new().entry(TestEntry::stored("shared.xml", XML));
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, root.build()))
        .entry(TestEntry::stored(SHARED, shared.build()))
}

fn with_children(name: &str, children: &[&str]) -> TestEntry {
    let container = children
        .iter()
        .fold(TestObjectContainer::new(name), |root, child| {
            root.child(child, [0.0; 3], [1.0, 0.0, 0.0, 0.0])
        });
    TestEntry::stored(format!("{name}.xml"), container.xml())
}

fn unread_target(path: &str) -> TestEntry {
    TestEntry::deflated(path, b"synthetic geometry is not read").with_payload([0; 8])
}

fn entry_at<'archive>(archive: &'archive Archive, path: &str) -> Check<&'archive Entry> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or_else(|| "physical fixture entry".into())
}
