// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_media::{ExportOptions, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAnimation, TestDba, TestEntity, TestEntry, TestInstance,
    TestModel, TestStruct, TestType, TestValue, entity_schema, test_key,
};
use serde_json::json;

use super::{Origins, TestResult, entry_index, support, typed, verify_row};

const ENTITY: &str = "FixtureBankContributor";
const CHR: &str = "Data/Objects/BankActor.chr";
const PARAMS: &str = "Data/Objects/BankActor.chrparams";
const ADB: &str = "Data/Animations/ClipBank/fixture.adb";
const DBA: &str = "Data/Animations/ClipBank/fixture.dba";
const COMPONENT: &str = "SAnimatedCharacterComponentParams";
const FIELD: &str = "animDatabase3P";
const CHRPARAMS: &str = concat!(
    "<Params><AnimationList>",
    r#"<Animation Name="bankAlias" Path="Animations/ClipBank/bank_only.caf"/>"#,
    r#"<Animation Name="$TracksDatabase" Path="Animations/ClipBank/fixture.dba"/>"#,
    "</AnimationList></Params>",
);
const MANNEQUIN: &str = concat!(
    "<AnimDB><FragmentList><Idle><Fragment><AnimLayer>",
    r#"<Animation name="bankAlias"/>"#,
    "</AnimLayer></Fragment></Idle></FragmentList></AnimDB>",
);

#[test]
fn named_bank_contributor_separates_source_alias_directory_and_clip_ordinal_origins() -> TestResult
{
    let scratch = tempfile::tempdir()?;
    let archive = fixture(scratch.path())?;
    let output = scratch.path().join("out");
    let options = ExportOptions {
        kinds: vec![MediaKind::AnimationDefinition],
        animation_entity: Some(ENTITY.into()),
        ..ExportOptions::default()
    };
    let index = support::published(p4kx_media::export(&archive, &output, &options)?)?;
    let rows = support::records(&output, &index, "animation-definition.jsonl")?;
    let contributors = typed(&rows, "clip-contributor").collect::<Vec<_>>();
    let [(ordinal, row)] = contributors.as_slice() else {
        return Err("the dedicated bank-only alias has one observed contributor".into());
    };
    verify_selection(&archive, row)?;
    let origins = Origins::read(&output.join(index.artifact_base))?;
    verify_sources(&archive, &origins, *ordinal)
}

fn fixture(folder: &Path) -> Result<Archive, Box<dyn Error>> {
    let owner = TestEntity::new(1, ENTITY, "entities/bank-contributor.xml")
        .geometry("Objects/BankActor.chr", None)
        .component(TestValue::Struct(
            TestInstance::new(COMPONENT)
                .value(FIELD, TestValue::text("Animations/ClipBank/fixture.adb")),
        ));
    let data = entity_schema(DataForgeBuilder::new())
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field(FIELD, TestType::String),
        )
        .record(owner.record())
        .build();
    let bank = TestDba::new()
        .animation(TestAnimation::new("Animations/Decoy/bank_only.caf", 20))
        .animation(TestAnimation::new(
            "Data/Animations/ClipBank/bank_only.caf",
            30,
        ))
        .bytes();
    let path = folder.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", data))
        .entry(TestEntry::stored(CHR, TestModel::cube().cgf()))
        .entry(TestEntry::stored(PARAMS, CHRPARAMS.as_bytes()))
        .entry(TestEntry::stored(ADB, MANNEQUIN.as_bytes()))
        .entry(TestEntry::stored(DBA, bank).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

fn verify_selection(archive: &Archive, row: &MediaRecord) -> TestResult {
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    verify_row(row, &mut [0], &validator)?;
    assert_eq!(
        row.data.get("payload"),
        Some(&json!({
            "placement_ordinal": "0",
            "database_binding_ordinal": "0",
            "clip": {
                "entry_index": entry_index(archive, ADB)?.to_string(),
                "element_ordinal": "5",
                "declaration_ordinal": "0",
            },
            "alias": {
                "entry_index": entry_index(archive, PARAMS)?.to_string(),
                "element_ordinal": "2",
                "declaration_ordinal": "1",
                "kind": "named-dba",
                "candidate_ordinal": "0",
            },
            "target": {
                "kind": "dba",
                "entry_index": entry_index(archive, DBA)?.to_string(),
                "clip_ordinal": "1",
            },
            "match_basis": "exact",
        })),
        "the sole candidate is the original second physical bank clip, independently of the alias ordinal"
    );
    Ok(())
}

fn verify_sources(archive: &Archive, origins: &Origins, ordinal: usize) -> TestResult {
    let alias = entry_index(archive, PARAMS)?;
    let clip = entry_index(archive, ADB)?;
    let bank = entry_index(archive, DBA)?;
    for (field, encrypted, sources) in [
        (
            "/data/payload/alias/declaration_ordinal",
            false,
            BTreeSet::from([alias]),
        ),
        (
            "/data/payload/target/entry_index",
            false,
            BTreeSet::from([bank]),
        ),
        (
            "/data/payload/target/clip_ordinal",
            true,
            BTreeSet::from([bank]),
        ),
        (
            "/data/payload/match_basis",
            false,
            BTreeSet::from([alias, clip]),
        ),
    ] {
        assert_eq!(
            origins.entry_indices(ordinal, field, encrypted)?,
            sources,
            "the immediate origin of {field} retains exactly its actual contributing entries"
        );
    }
    Ok(())
}
