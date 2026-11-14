// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt::Write as _;

use p4kx_extraction::{Archive, DataForgeEntry, OpenOptions, SourceEncryption};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry,
    TestModel, TestSkeleton, entity_schema, landing_system_record, test_key,
};

use super::{EntityRig, RigFile, RigFileRead, RigPlacement, discover_entity_rigs};
use crate::{ArchiveModels, LoadoutResolver};

type TestResult = Result<(), Box<dyn Error>>;

const SHARED: &str = r"Data\Objects\Test\shared.chr";
const PARAMETERS: &str = r"Data\Objects\Test\shared.chrparams";

#[test]
fn missing_root_leaves_encrypted_child_and_gear_rig_sources_visible() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("rigs.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .entry(
            TestEntry::stored(
                SHARED,
                TestModel::cube()
                    .with_skeleton(&TestSkeleton::two_bones())
                    .cgf(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(PARAMETERS, parameters()?))
        .write_to(&path)?;

    let archive = Archive::open_with(&path, OpenOptions::new().key(Some(test_key())))?;
    let forge_entry = DataForgeEntry::read(&archive)?;
    let forge = forge_entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&forge_entry);
    let root = resolver.entities().find("RigRoot")?;
    let tree = resolver.resolve(&root)?;
    let rows = discover_entity_rigs(&ArchiveModels::new(&archive), &tree);
    let [root, gear, child] = rows.as_slice() else {
        return Err("expected the root, gear and attached Entity in declaration order".into());
    };
    verify_placements(root, gear, child)?;
    verify_unavailable_key(&path, &tree)
}

fn verify_placements(root: &EntityRig, gear: &EntityRig, child: &EntityRig) -> TestResult {
    assert!(
        matches!(root.placement, RigPlacement::Root),
        "root retained"
    );
    assert!(
        matches!(
            root.skeleton.as_ref().map(|file| file.read.as_ref()),
            Some(RigFileRead::Missing)
        ),
        "missing rig remains explicit"
    );
    assert_eq!(
        root.skeleton.as_ref().map(RigFile::encryption),
        Some(SourceEncryption::Unknown),
        "unread payload has no encryption claim"
    );
    assert!(
        matches!(&gear.placement, RigPlacement::LandingGear { owner_path, ordinal: 0, bone: Some(bone) } if owner_path.is_empty() && bone == "strut"),
        "actual landing declaration retained"
    );
    assert!(
        matches!(&child.placement, RigPlacement::Attachment {port_path} if port_path == "port"),
        "actual attachment path retained"
    );
    assert_ne!(
        gear.record_index, child.record_index,
        "shared rig does not merge owner identity"
    );
    assert!(
        matches!(
            (&gear.skeleton, &child.skeleton),
            (Some(gear), Some(child)) if std::sync::Arc::ptr_eq(&gear.read, &child.read)
        ),
        "repeated placements share the same physical read"
    );
    verify_healthy(gear)?;
    verify_healthy(child)
}

fn verify_healthy(row: &EntityRig) -> TestResult {
    let source = row.skeleton.as_ref().ok_or("selected rig")?;
    let physical = source.source().ok_or("original rig Entry")?;
    assert_eq!(physical.entry().path(), SHARED, "same physical rig");
    assert_eq!(
        source.encryption(),
        SourceEncryption::Encrypted,
        "actual successful encrypted read"
    );
    assert_eq!(
        source
            .value()
            .and_then(Option::as_ref)
            .map(p4kx_model::SkeletonSource::bone_count),
        Some(2),
        "reads the actual original skeleton without geometry companions"
    );
    assert!(
        row.owner_source.is_some() && !row.geometry_selectors.is_empty(),
        "records the DataForge selection"
    );
    let params = row.parameters.as_ref().ok_or("parameter source")?;
    assert_eq!(
        params.source().map(|source| source.entry().path()),
        Some(PARAMETERS),
        "independent parameter source"
    );
    assert_eq!(
        params.encryption(),
        SourceEncryption::Unencrypted,
        "parameters and skeleton have independent ancestry"
    );
    let items = params.value().ok_or("exact parameter rows")?;
    assert_eq!(
        items.len(),
        71,
        "retains all directives beyond the preview quota"
    );
    assert_eq!(
        items.last().map(|item| (
            item.ordinal,
            item.name.as_str(),
            item.path.as_str(),
            item.selectors.len()
        )),
        Some((70, "*", "*/*.caf", 2)),
        "original ordinal, wildcard, and both positions remain available"
    );
    Ok(())
}

fn verify_unavailable_key(path: &std::path::Path, tree: &crate::EntityNode) -> TestResult {
    let without_key = Archive::open(path)?;
    let rows = discover_entity_rigs(&ArchiveModels::new(&without_key), tree);
    let [_, gear, _] = rows.as_slice() else {
        return Err("all placements remain accessible without the rig key".into());
    };
    let rig = gear.skeleton.as_ref().ok_or("requested rig")?;
    assert!(
        matches!(rig.read.as_ref(), RigFileRead::KeyRequired),
        "a missing key is its own outcome"
    );
    assert!(
        rig.source().is_none(),
        "no read physical contributor when key is required"
    );
    assert_eq!(
        rig.encryption(),
        SourceEncryption::Unknown,
        "unread selection has unknown payload provenance"
    );
    assert!(
        gear.parameters.as_ref().and_then(RigFile::source).is_some(),
        "plain parameters remain independently readable"
    );
    Ok(())
}

fn database() -> Vec<u8> {
    let root = TestEntity::new(1, "RigRoot", "entities/rig_root.xml")
        .geometry("Objects/Test/missing.chr", None)
        .vehicle("", "")
        .landing_system(3)
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "port", "RigChild",
        )]));
    let child = TestEntity::new(2, "RigChild", "entities/rig_child.xml")
        .geometry("Objects/Test/shared.chr", None);
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .record(landing_system_record(
            3,
            "RigLanding",
            &[("strut", "Objects/Test/shared.chr", 5.0)],
        ))
        .build()
}

fn parameters() -> Result<String, std::fmt::Error> {
    let mut out = "<Params><AnimationList>".to_owned();
    for index in 0..70 {
        write!(
            out,
            r##"<Animation name="#filepath" path="Animations/Test/{index}"/>"##
        )?;
    }
    out.push_str(r#"<Animation name="*" path="*/*.caf"/></AnimationList></Params>"#);
    Ok(out)
}

#[test]
fn cdf_selection_retains_its_origin_when_the_declared_skeleton_is_ambiguous() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("rigs.p4k");
    let entity = TestEntity::new(1, "RigRoot", "entities/rig_root.xml")
        .geometry("Objects/Test/setup.cdf", None);
    let database = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database))
        .entry(TestEntry::stored(
            "Data/Objects/Test/setup.cdf",
            r#"<CharacterDefinition><Model File="Objects/Test/shared.chr"/></CharacterDefinition>"#,
        ))
        .entry(TestEntry::stored(SHARED, b"first"))
        .entry(TestEntry::stored("data/objects/test/SHARED.chr", b"second"))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let tree = resolver.resolve(&resolver.entities().find("RigRoot")?)?;
    let rows = discover_entity_rigs(&ArchiveModels::new(&archive), &tree);
    let [root] = rows.as_slice() else {
        return Err("one resolved root position".into());
    };
    let definition = root.definition.as_ref().ok_or("original definition")?;
    let original = definition.value().ok_or("read exact CDF model selection")?;
    assert_eq!(
        original.skeleton.as_deref(),
        Some("Objects/Test/shared.chr"),
        "actual Model File"
    );
    assert!(
        original.selector.is_some(),
        "exact source attribute coordinate"
    );
    assert_eq!(
        definition.encryption(),
        SourceEncryption::Unencrypted,
        "CDF metadata was independently read"
    );
    let skeleton = root.skeleton.as_ref().ok_or("requested skeleton")?;
    assert!(
        matches!(skeleton.read.as_ref(), RigFileRead::Ambiguous),
        "no present duplicate is chosen arbitrarily"
    );
    assert_eq!(
        skeleton.encryption(),
        SourceEncryption::Unknown,
        "no payload was selected"
    );
    Ok(())
}
