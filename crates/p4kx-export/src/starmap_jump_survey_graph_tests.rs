// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use std::time::UNIX_EPOCH;

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestContainer, TestElement, TestEntry, TestRecord,
    TestStruct, TestType, TestValue,
};

use super::{FieldSurvey, GraphSurvey, survey};
use crate::CatalogOptions;
use crate::build::original_input::test_support::{self, TestResult};
use crate::starmap_containers::jump_class::JumpEndpointRecords;

const ROOT: &str = "Data/ObjectContainers/PU/system/survey/surveysystem.socpak";
const BRANCH: &str = "Data/ObjectContainers/PU/system/survey/branch.socpak";
const LEAF: &str = "Data/ObjectContainers/PU/system/survey/leaf.socpak";
const MISSING: &str = "Data/ObjectContainers/PU/system/survey/missing.socpak";
const DUPLICATE: &str = "Data/ObjectContainers/PU/system/survey/duplicate.socpak";
const DETACHED: &str = "Data/ObjectContainers/PU/survey/detached.socpak";

#[test]
fn jump_survey_follows_nested_sources_and_retains_read_gaps() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("survey.p4k");
    let root = descriptor(TestContainer::new(), "surveysystem.xml", &[BRANCH, BRANCH]);
    let branch = descriptor(
        TestContainer::new(),
        "branch.xml",
        &[LEAF, DUPLICATE, MISSING],
    );
    let leaf = descriptor(TestContainer::new(), "leaf.xml", &[]);
    let duplicate = descriptor(
        TestContainer::new(),
        "duplicate.xml",
        &[DETACHED, LEAF, MISSING],
    );
    let duplicate_cycle = descriptor(
        TestContainer::new(),
        "duplicate.xml",
        &[DUPLICATE, DETACHED],
    );
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, root))
        .entry(TestEntry::stored(BRANCH, branch))
        .entry(TestEntry::stored(LEAF, leaf))
        .entry(TestEntry::stored(DUPLICATE, duplicate))
        .entry(TestEntry::stored(DUPLICATE, duplicate_cycle))
        .entry(TestEntry::stored(
            DETACHED,
            descriptor(TestContainer::new(), "detached.xml", &[]),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let result = survey(&archive, &JumpEndpointRecords::default())?;
    assert_eq!(result.roots, 1, "one admitted system root");
    assert_eq!(
        result.reachable.physical_containers, 3,
        "a reused container is read once and its grandchild is reached"
    );
    assert_eq!(
        result.reachable.readable_descriptors, 3,
        "every selected unique descriptor was read"
    );
    assert_eq!(
        result.reachable.external_targets, 5,
        "separate authored children remain separate evidence"
    );
    assert_eq!(
        result.reachable.ambiguous_external_targets, 1,
        "the duplicate is never elected"
    );
    assert_eq!(
        result.reachable.missing_external_targets, 1,
        "the missing source contributes an explicit gap"
    );
    check_supplemental_counts(&result);
    Ok(())
}

fn check_supplemental_counts(result: &GraphSurvey) {
    assert_eq!(
        result.unreached_system_container_candidates, 1,
        "one normalized path"
    );
    assert_eq!(
        result.supplemental_physical_seeds, 2,
        "both original candidates are retained"
    );
    assert_eq!(
        (
            result.reachable.missing_endpoint_external_targets,
            result.reachable.ambiguous_endpoint_external_targets,
        ),
        (0, 0),
        "ordinary unread and ambiguous links are not endpoint-specific gaps"
    );
    let detached = &result.supplemental;
    assert_eq!(
        detached.physical_containers, 3,
        "the detached leaf is read once without re-reading the reachable leaf"
    );
    assert_eq!(
        detached.readable_descriptors, 3,
        "both duplicates and the detached leaf are inspected"
    );
    assert_eq!(
        detached.ambiguous_external_targets, 1,
        "cycles back to duplicates remain ambiguous"
    );
    assert_eq!(
        detached.missing_external_targets, 1,
        "a detached reference can have a missing target"
    );
}

#[test]
fn endpoint_child_survey_finds_nested_attributes_and_text() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("endpoint-child.p4k");
    let container = TestContainer::new()
        .entry(TestEntry::stored(
            "surveysystem.xml",
            endpoint_document().to_xml(),
        ))
        .build();
    let duplicate = descriptor(TestContainer::new(), "duplicate.xml", &[]);
    ArchiveBuilder::new()
        .entry(TestEntry::stored(ROOT, container))
        .entry(TestEntry::stored(DUPLICATE, duplicate.clone()))
        .entry(TestEntry::stored(DUPLICATE, duplicate))
        .entry(TestEntry::stored(
            LEAF,
            descriptor(TestContainer::new(), "leaf.xml", &[]),
        ))
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            endpoint_records().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    test_support::with_archive_input(&archive, |forge, input| {
        let endpoints = JumpEndpointRecords::read(forge, input, &CatalogOptions::new(UNIX_EPOCH))?;
        let result = survey(&archive, &endpoints)?;
        assert_eq!(
            result.reachable.endpoint_declarations, 3,
            "three original endpoint Child occurrences"
        );
        assert_eq!(
            result.reachable.marked_gate_descriptors, 0,
            "no gate marker"
        );
        assert_eq!(
            result.reachable.missing_endpoint_external_targets, 1,
            "the absent reference is declared directly by an endpoint Child"
        );
        assert_eq!(
            result.unreached_system_container_candidates, 2,
            "the unplaced path and the ambiguous path remain distinct"
        );
        assert_eq!(
            result.reachable.ambiguous_endpoint_external_targets, 1,
            "ambiguous endpoint targets remain explicit"
        );
        assert_eq!(
            result.reachable.admitted_main_xml.endpoint_guid_texts, 2,
            "the unmarked main document includes both its endpoint and unrelated child text"
        );
        check_endpoint_fields(&result.reachable.endpoint_child_fields);
        Ok(())
    })
}

fn check_endpoint_fields(fields: &FieldSurvey) {
    assert_eq!(
        fields.candidate_shapes.get("Destination"),
        Some(&1),
        "a link-like element name is counted separately from its text"
    );
    assert_eq!(
        fields.endpoint_guid_values, 1,
        "nested attribute references an endpoint"
    );
    assert_eq!(
        fields.endpoint_guid_texts, 1,
        "nested text references an endpoint"
    );
    assert_eq!(
        fields.relation_named_texts, 1,
        "only its own non-placement descendants"
    );
    assert_eq!(
        fields.text_candidates_without_value_coordinates, 1,
        "decoded element text is structural evidence"
    );
}

fn endpoint_document() -> TestElement {
    const DESTINATION: &str = "20000000-0000-4000-8000-000000000002";
    let owner = TestElement::new("Child")
        .attribute("starMapRecord", "10000000-0000-4000-8000-000000000001")
        .child(
            TestElement::new("Route")
                .attribute("asset", DESTINATION)
                .child(TestElement::new("Destination").content(DESTINATION)),
        )
        .child(TestElement::new("ChildObjectContainers").child(
            TestElement::new("Child").child(TestElement::new("UnrelatedLink").content(DESTINATION)),
        ));

    TestElement::new("ObjectContainer").child(
        TestElement::new("ChildObjectContainers")
            .child(owner)
            .child(external_endpoint(MISSING))
            .child(external_endpoint(DUPLICATE)),
    )
}

fn external_endpoint(path: &str) -> TestElement {
    TestElement::new("Child")
        .attribute("starMapRecord", "10000000-0000-4000-8000-000000000001")
        .attribute("external", "1")
        .attribute("name", path)
}

fn endpoint_records() -> DataForgeBuilder {
    const TYPE: u128 = 3;
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObjectType").field("name", TestType::String))
        .structure(TestStruct::new("StarMapObject").field(
            "type",
            TestType::ReferenceTo("StarMapObjectType".to_owned()),
        ))
        .record(
            TestRecord::new(
                TYPE,
                "StarMapObjectType",
                "JumpType",
                "fixture/jump_type.xml",
            )
            .value("name", TestValue::text("JumpPoint")),
        )
        .record(endpoint(
            0x1000_0000_0000_4000_8000_0000_0000_0001,
            "First",
            TYPE,
        ))
        .record(endpoint(
            0x2000_0000_0000_4000_8000_0000_0000_0002,
            "Second",
            TYPE,
        ))
}

fn endpoint(guid: u128, name: &str, type_guid: u128) -> TestRecord {
    TestRecord::new(
        guid,
        "StarMapObject",
        name,
        format!("libs/foundry/records/starmap/pu/{name}.xml"),
    )
    .value("type", TestValue::Reference(type_guid))
}

fn descriptor(container: TestContainer, file: &str, targets: &[&str]) -> Vec<u8> {
    let list = targets
        .iter()
        .fold(TestElement::new("ChildObjectContainers"), |list, path| {
            list.child(
                TestElement::new("Child")
                    .attribute("external", "1")
                    .attribute("name", *path),
            )
        });
    let xml = TestElement::new("ObjectContainer").child(list).to_xml();
    container.entry(TestEntry::stored(file, xml)).build()
}
