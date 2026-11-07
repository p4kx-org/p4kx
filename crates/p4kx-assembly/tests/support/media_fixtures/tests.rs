// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities, EntityKind};
use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::media_fixture::SelectionError;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, TestPort,
    TestRecord, TestStruct, TestType, TestValue,
};

use super::observer::Fault;
use super::{Case, DockedVehicles, Family, Identity, Index, Requirements};

pub(super) const OPTIONAL: Requirements = Requirements {
    geometry_lod: None,
    item_size: None,
    requires_interior: false,
    requires_audio: false,
    docked: DockedVehicles::Expand,
};

pub(super) fn case(preferred: &'static [&'static str], requirements: Requirements) -> Case {
    Case {
        label: "synthetic source identity",
        preferred,
        families: &["Family"],
        kind: EntityKind::Other,
        requirements,
    }
}

fn archive() -> super::Result<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new().write_to(&path)?;
    Ok((directory, Archive::open(path)?))
}

#[test]
fn chosen_guid_alias_uses_its_exact_physical_record_and_geometry() -> super::Result<()> {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Family_First", "synthetic/first.xml")
                .geometry("First.cgf", None)
                .record(),
        )
        .record(
            TestEntity::new(1, "Family_Second", "synthetic/second.xml")
                .geometry("Second.cgf", None)
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
    let choice = index
        .choose(&archive, &case(&["Family_Second"], OPTIONAL))?
        .ok_or("fixture choice absent")?;
    assert_eq!(
        choice.candidate.identity.record_index, 1,
        "the preferred class retains its real physical source index"
    );
    assert_eq!(
        choice
            .capabilities
            .node()?
            .geometry()
            .ok_or("selected geometry absent")?
            .path(),
        "Second.cgf",
        "GUID aliases never substitute another physical root geometry"
    );
    assert_eq!(
        choice.candidate.identity.record_path, "synthetic/second",
        "the source path belongs to the same exact physical Record"
    );
    Ok(())
}

#[test]
fn duplicate_preferred_class_fails_before_any_canonical_projection() -> super::Result<()> {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(TestRecord::new(
            1,
            "EntityClassDefinition",
            "EntityClassDefinition.Family_Same",
            "synthetic/one.xml",
        ))
        .record(TestRecord::new(
            2,
            "EntityClassDefinition",
            "EntityClassDefinition.Family_Same",
            "synthetic/two.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
    let error = index
        .choose(&archive, &case(&["Family_Same"], OPTIONAL))
        .err()
        .ok_or("ambiguous preferred class was accepted")?;
    assert!(
        matches!(error.downcast_ref::<SelectionError<Identity>>(),
        Some(SelectionError::AmbiguousPreferred { identities, .. }) if identities.len() == 2),
        "both real physical identities remain visible instead of first-wins lookup"
    );
    Ok(())
}

#[test]
fn a_present_malformed_preference_is_a_failure_with_no_fallback() -> super::Result<()> {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition").field("bad", TestType::Bool))
        .record(
            TestRecord::new(
                1,
                "EntityClassDefinition",
                "EntityClassDefinition.Family_Preferred",
                "synthetic/preferred.xml",
            )
            .value("bad", TestValue::Raw(vec![2])),
        )
        .record(TestRecord::new(
            2,
            "EntityClassDefinition",
            "EntityClassDefinition.Family_Healthy",
            "synthetic/healthy.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
    let error = index
        .choose(&archive, &case(&["Family_Preferred"], OPTIONAL))
        .err()
        .ok_or("malformed preferred source was replaced")?;
    assert!(
        matches!(error.downcast_ref::<SelectionError<Identity>>(),
        Some(SelectionError::Probe { identity, .. }) if identity.record_index == 0),
        "an actual canonical source fault remains attached to the selected physical Record"
    );
    Ok(())
}

#[test]
fn a_declared_but_unreadable_model_never_becomes_unavailable_coverage() -> super::Result<()> {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Family_Preferred", "synthetic/preferred.xml")
                .geometry("MissingDeclaredGeometry.cgf", None)
                .record(),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
    let required = Requirements {
        geometry_lod: Some(0),
        ..OPTIONAL
    };
    assert!(
        index
            .choose(&archive, &case(&["Family_Preferred"], required))
            .is_err(),
        "the actual declared model read error fails instead of triggering replacement or absence"
    );
    Ok(())
}

pub(super) fn fault<'e>(error: &'e (dyn std::error::Error + 'static)) -> super::Result<&'e Fault> {
    let selection = error
        .downcast_ref::<SelectionError<Identity>>()
        .ok_or("typed selection error absent")?;
    let SelectionError::Probe { source, .. } = selection else {
        return Err("actual probe error absent".into());
    };
    source
        .downcast_ref::<Fault>()
        .ok_or_else(|| "original observed source failure absent".into())
}

#[test]
fn public_family_preferences_remain_inside_their_documented_scope() {
    for family in Family::ALL {
        let case = Case::from_family("synthetic_family_scope", family, OPTIONAL);
        for preferred in case.preferred {
            assert!(
                case.accepts_family(preferred),
                "a documented preferred fixture belongs to its own family"
            );
        }
    }
}

#[test]
fn canonical_and_field_faults_keep_the_exact_original_physical_owner() -> super::Result<()> {
    for layout_failure in [true, false] {
        let (_directory, archive) = archive()?;
        let files = ArchiveFiles::new(&archive);
        let structure = if layout_failure {
            TestStruct::new("EntityClassDefinition")
                .field("bad", TestType::Int32)
                .declared_size(6)
        } else {
            TestStruct::new("EntityClassDefinition").field("bad", TestType::Bool)
        };
        let record = TestRecord::new(
            1,
            "EntityClassDefinition",
            "EntityClassDefinition.Family_Preferred",
            "synthetic/preferred.xml",
        )
        .value(
            "bad",
            TestValue::Raw(if layout_failure {
                vec![2, 0, 0, 0]
            } else {
                vec![2]
            }),
        );
        let bytes = DataForgeBuilder::new()
            .structure(structure)
            .record(record)
            .build();
        let forge = DataForge::open(&bytes)?;
        let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
        let error = index
            .choose(&archive, &case(&["Family_Preferred"], OPTIONAL))
            .err()
            .ok_or("invalid source was accepted")?;
        let physical = match fault(error.as_ref())? {
            Fault::Canonical { record_index, .. } if layout_failure => *record_index,
            Fault::Field { record_index, .. } if !layout_failure => *record_index,
            _ => return Err("the actual read and field failure stages were conflated".into()),
        };
        assert_eq!(
            physical, 0,
            "the original physical source remains attached to its typed fault"
        );
    }
    Ok(())
}

#[test]
fn rejected_dependencies_retain_their_actual_target_record_index() -> super::Result<()> {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .structure(TestStruct::new("NotAnEntity"))
        .record(
            TestEntity::new(1, "Family_Preferred", "synthetic/root.xml")
                .port(TestPort::new("mount"))
                .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::reference(
                    "mount", 2,
                )]))
                .record(),
        )
        .record(TestRecord::new(
            2,
            "NotAnEntity",
            "NotAnEntity.Target",
            "synthetic/target.xml",
        ))
        .build();
    let forge = DataForge::open(&bytes)?;
    let index = Index::new(Arc::new(Entities::new(&forge)), &files)?;
    let error = index
        .choose(&archive, &case(&["Family_Preferred"], OPTIONAL))
        .err()
        .ok_or("rejected dependency was hidden by optional projection helpers")?;
    assert!(
        matches!(
            fault(error.as_ref())?,
            Fault::Rejected {
                record_index: 1,
                ..
            }
        ),
        "dependency provenance refers to the actual target rather than the selected root"
    );
    Ok(())
}
