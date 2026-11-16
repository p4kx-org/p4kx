// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Compression, write_catalog_from};
use p4kx_testkit::{
    DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

#[test]
fn original_reload_options_join_only_through_their_physical_scopes() -> TestResult {
    let fixture = Fixture::open(&database(), Input::Encrypted, documents())?;
    let (built, catalog) = fixture.diagnostic()?;
    check_compressed_replay(&fixture, built.json())?;
    let schema = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    schema
        .validate(&catalog)
        .map_err(|error| error.to_string())?;
    let candidates = origin::rows(
        &catalog,
        "/projections/weaponPlayerAnimationCandidates/rows",
    )?;
    let [first, _] = candidates else {
        return Err("two distinct options for the same physical weapon and scoped fragment".into());
    };
    assert_eq!(
        first
            .pointer("/weaponRecordIndex/value")
            .and_then(Value::as_u64),
        Some(1),
        "the join identifies the original personal weapon Record",
    );
    assert_eq!(
        first
            .pointer("/attachmentComponentOrdinal/value")
            .and_then(Value::as_u64),
        Some(0),
        "the join identifies the physical AttachDef component",
    );
    check_candidate_order(candidates)?;
    for row in candidates {
        let tag_evidence = origin::cell(row, "tagEvidence", "present")?;
        assert_eq!(
            tag_evidence["value"], "sharedDeclaration",
            "only a scoped tag is established"
        );
        let playback = origin::cell(row, "playback", "present")?;
        assert_eq!(
            playback["value"], "unverified",
            "the source does not select a runtime animation"
        );
        assert_eq!(
            playback["containsEncryptedSource"], true,
            "the derived row includes the encrypted weapon tag source"
        );
    }
    check_sources(&catalog)?;
    origin::closure(&catalog)
}

fn check_candidate_order(candidates: &[Value]) -> TestResult {
    let positions = candidates
        .iter()
        .map(|candidate| {
            origin::at(candidate, "/optionPosition/value/indices/0")?
                .as_str()
                .ok_or("the original XML byte position")?
                .parse::<usize>()
                .map_err(Into::into)
        })
        .collect::<TestResult<Vec<_>>>()?;
    let authored = CANDIDATE
        .match_indices("<Fragment Tags=")
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(
        positions, authored,
        "the candidates retain the two distinct authored positions in source order",
    );
    Ok(())
}

fn check_compressed_replay(fixture: &Fixture, expected: &[u8]) -> TestResult {
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..crate::health_origin_fixture::options()
    };
    let (_, packed) = write_catalog_from(
        &fixture.archive,
        &fixture.entry,
        &options,
        Compression::Zstd { level: 1 },
        || Ok(Vec::new()),
    )?;
    let decoded = zstd::stream::decode_all(packed.as_slice())?;
    assert_eq!(
        decoded, expected,
        "weapon candidates and their origins survive both compressed writer passes in source order",
    );
    Ok(())
}

fn check_sources(catalog: &Value) -> TestResult {
    let sources = origin::at(catalog, "/definitions/weaponAnimationSources")?;
    check_shared_tag_selection(catalog, sources)?;
    let statuses: Vec<_> = origin::rows(sources, "/fragmentSelectors")?
        .iter()
        .filter_map(|row| row.pointer("/status/value").and_then(Value::as_str))
        .collect();
    assert!(
        statuses.contains(&"matched"),
        "the identical physical FragDef admits the alternatives"
    );
    assert!(
        statuses.contains(&"differentScope"),
        "an equal fragment spelling cannot cross another FragDef"
    );
    assert!(
        statuses.contains(&"noMatchingGroup"),
        "an explicit unmatched FragmentID is retained"
    );
    assert!(
        statuses.contains(&"unresolvedTarget"),
        "a missing SubADB preserves its source selector"
    );
    let branches = origin::rows(sources, "/branches")?;
    assert!(
        branches
            .iter()
            .any(|row| row.pointer("/traversal/value").and_then(Value::as_str) == Some("cycle")),
        "an actual cycle has an explicit source result"
    );
    let databases = origin::rows(sources, "/databases")?;
    let has_proc_layer = databases
        .iter()
        .flat_map(|row| row["options"].as_array().into_iter().flatten())
        .flat_map(|row| row["elements"].as_array().into_iter().flatten())
        .any(|row| row.pointer("/name/value").and_then(Value::as_str) == Some("ProcLayer"));
    assert!(
        has_proc_layer,
        "uninterpreted original procedural layers survive"
    );
    Ok(())
}

fn check_shared_tag_selection(catalog: &Value, sources: &Value) -> TestResult {
    let scopes = origin::rows(sources, "/scopes")?;
    let [first, rest @ ..] = scopes else {
        return Err("physical ADB tag scopes".into());
    };
    assert_eq!(rest.len(), 2, "three databases declare the shared tag root");
    let declarations = origin::rows(first, "/tags/declarations")?;
    assert_eq!(declarations.len(), 1, "the original imported tag");
    let imports = origin::rows(first, "/tags/imports")?;
    assert_eq!(imports.len(), 1, "the original shared import");
    let uses = origin::rows(catalog, "/provenance/uses")?;
    for scope in scopes {
        assert_eq!(
            origin::rows(scope, "/tags/declarations")?,
            declarations,
            "same physical definition retains the same declaration rows"
        );
        assert_eq!(
            origin::rows(scope, "/tags/imports")?,
            imports,
            "each scope preserves its original matching import"
        );
        let database = origin::at(scope, "/database/inputRef")?;
        let coverage = origin::rows(scope, "/tags/coverage/sourceUsesRefs")?;
        assert!(
            uses.iter().any(|row| {
                row.get("source") == Some(database)
                    && row.get("id").is_some_and(|id| coverage.contains(id))
            }),
            "the coverage includes the arriving database's own selector"
        );
    }
    Ok(())
}

const COMPONENT_BASE: &str = "DataForgeComponentParams";
const ENTITY: &str = "EntityClassDefinition";
const ATTACHMENT: &str = "SAttachableComponentParams";
const ITEM: &str = "SItemDefinition";
const MOTION: &str = "SAnimatedCharacterComponentParams";

fn database_schema() -> DataForgeBuilder {
    use TestType::{Class, String as StringType, StrongPointer};
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(COMPONENT_BASE))
        .structure(
            TestStruct::new(ENTITY).array("Components", StrongPointer(COMPONENT_BASE.to_owned())),
        )
        .structure(
            TestStruct::new(ITEM)
                .field("Type", StringType)
                .field("Tags", StringType),
        )
        .structure(
            TestStruct::new(ATTACHMENT)
                .parent(COMPONENT_BASE)
                .field("AttachDef", Class(ITEM.to_owned())),
        )
        .structure(
            TestStruct::new(MOTION)
                .parent(COMPONENT_BASE)
                .field("actionController", StringType)
                .field("animDatabase3P", StringType),
        )
}

fn database() -> Vec<u8> {
    database_schema()
        .record(
            TestRecord::new(
                0x1053,
                ENTITY,
                "EntityClassDefinition.Player",
                "synthetic/player.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(MOTION)
                        .value(
                            "actionController",
                            TestValue::text("Animations/TestController.xml"),
                        )
                        .value("animDatabase3P", TestValue::text("Animations/TestRoot.adb")),
                )]),
            ),
        )
        .record(
            TestRecord::new(
                0x1054,
                ENTITY,
                "EntityClassDefinition.SyntheticWeapon",
                "libs/foundry/records/entities/scitem/weapons/synthetic_reload.xml",
            )
            .value(
                "Components",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(ATTACHMENT).value(
                        "AttachDef",
                        TestValue::Struct(
                            TestInstance::new(ITEM)
                                .value("Type", TestValue::text("WeaponPersonal"))
                                .value("Tags", TestValue::text("matching_tag")),
                        ),
                    ),
                )]),
            ),
        )
        .build()
}

fn documents() -> Vec<TestEntry> {
    [
        ("TestController.xml", r#"<ControllerDef><Fragments filename="Animations/SharedFragments.xml"/></ControllerDef>"#),
        ("TestRoot.adb", ROOT),
        ("Candidate.adb", CANDIDATE),
        ("WrongScope.adb", WRONG_SCOPE),
        ("SharedFragments.xml", r#"<TagDefinition version="2"><Tags><Tag name="Reload"/></Tags></TagDefinition>"#),
        ("OtherFragments.xml", r#"<TagDefinition version="2"><Tags><Tag name="Reload"/></Tags></TagDefinition>"#),
        ("SharedTags.xml", r#"<TagDefinition version="2"><Imports><Import filename="Animations/ImportedTags.xml"/></Imports></TagDefinition>"#),
        ("ImportedTags.xml", r#"<TagDefinition version="2"><Tags><Group name="Synthetic"><Tag name="matching_tag"/></Group></Tags></TagDefinition>"#),
    ].into_iter().map(|(path, content)| TestEntry::stored(format!("Data/Animations/{path}"), content.as_bytes())).collect()
}

const ROOT: &str = r#"<AnimDB FragDef="Animations/SharedFragments.xml" TagDef="Animations/SharedTags.xml"><SubADBs>
    <SubADB File="Animations/Candidate.adb"><FragmentID Name="Reload"/><FragmentID Name="NotPresent"/></SubADB>
    <SubADB File="Animations/WrongScope.adb"><FragmentID Name="Reload"/></SubADB>
    <SubADB File="Animations/Missing.adb"><FragmentID Name="Reload"/></SubADB>
    </SubADBs></AnimDB>"#;
const CANDIDATE: &str = r#"<AnimDB FragDef="Animations/SharedFragments.xml" TagDef="Animations/SharedTags.xml">
    <SubADBs><SubADB File="Animations/TestRoot.adb"/></SubADBs>
    <FragmentList><Reload>
      <Fragment Tags="matching_tag" FragTags="" BlendOutDuration="0.2">
        <AnimLayer><Blend Duration="0.2" StartTime="0.3" ExitTime="0.4"/><Animation name="synthetic" speed="1.2" flags="4"/></AnimLayer>
        <ProcLayer><Procedural Procedure="synthetic"/></ProcLayer>
      </Fragment>
      <Fragment Tags="matching_tag" BlendOutDuration="0.5"><AnimLayer><Blend Duration="0.6"/><Animation name="second"/></AnimLayer></Fragment>
    </Reload></FragmentList></AnimDB>"#;
const WRONG_SCOPE: &str = r#"<AnimDB FragDef="Animations/OtherFragments.xml" TagDef="Animations/SharedTags.xml">
    <FragmentList><Reload><Fragment Tags="matching_tag" BlendOutDuration="4"/></Reload></FragmentList></AnimDB>"#;
