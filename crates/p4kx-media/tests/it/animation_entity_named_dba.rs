// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{Archive, Exclusion, OpenOptions};
use p4kx_media::{ExportOptions, ExportOutcome, MediaKind, MediaRecord};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestAnimation, TestChunkFile, TestDba, TestEntity, TestEntry,
    TestModel, TestSkeleton, entity_schema, test_key,
};
use serde_json::{Value, json};

use super::support::{TestResult, published, records};

const FIRST: &str = "Data/Animations/Banks/first.dba";
const FILTERED: &str = "Data/Animations/Banks/filtered.dba";
const OTHER_LIST: &str = "Data/Animations/Banks/other-list.dba";
const PARAMS: &str = r##"<Params><AnimationList>
    <Animation name="#filepath" path="Animations/Fixture"/>
    <Animation name="bankOnly" path="alias.caf"/>
    <Animation name="$TracksDatabase" path="Animations/Banks/first.dba"/>
    <Animation name="$TracksDatabase" path="Animations/Banks/filtered.dba"/>
    <Animation name="$TracksDatabase" path="Data\Animations\Banks\first.dba"/>
    </AnimationList><AnimationList>
    <Animation name="$TracksDatabase" path="Animations/Banks/other-list.dba"/>
    </AnimationList></Params>"##;
const BANK: &str = "animation-entity-named-dba-bank";
const MATCH: &str = "animation-entity-named-dba-match";

#[test]
fn bank_only_alias_matches_the_definition_provider_and_preserves_repeated_declarations()
-> TestResult {
    let temp = tempfile::tempdir()?;
    let archive = fixture(temp.path(), BankFixture::Intact)?;
    let output = temp.path().join("output");
    let index = published(p4kx_media::export(&archive, &output, &options()?)?)?;
    let rows = records(&output, &index, "animation.jsonl")?;
    let definitions = records(&output, &index, "animation-definition.jsonl")?;
    let canonical = definitions
        .iter()
        .find(|row| row.data.pointer("/record_type") == Some(&json!("named-dba-binding")))
        .ok_or("definition binding")?;
    let candidates = canonical
        .data
        .pointer("/payload/candidates")
        .and_then(Value::as_array)
        .ok_or("canonical candidates")?;
    assert_eq!(candidates.len(), 4);
    let banks = verify_banks(&rows, "enumerated")?;
    let matches: Vec<_> = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == MATCH)
        .collect();
    let validator = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    for row in banks
        .iter()
        .copied()
        .chain(matches.iter().map(|(_, row)| *row))
    {
        validator
            .validate(&serde_json::to_value(row)?)
            .map_err(|error| error.to_string())?;
    }
    assert_eq!(matches.len(), candidates.len());
    for ((_, row), canonical) in matches.iter().zip(candidates) {
        compare_candidate(row, canonical, &rows)?;
    }
    let loose = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-source-link"
                && row.subresource.indices == [0, 1, 0]
        })
        .ok_or("independent loose link")?;
    assert_eq!(
        loose.data.pointer("/payload/reason"),
        Some(&json!("missing"))
    );
    verify_origins(&output.join(index.artifact_base), &rows, &matches)
}

#[test]
fn incomplete_controllers_do_not_hide_names_and_a_truncated_name_table_stays_incomplete()
-> TestResult {
    let temp = tempfile::tempdir()?;
    for (subdir, fixture_kind, expected) in [
        (
            "controller-error",
            BankFixture::BadControllers,
            "enumerated",
        ),
        ("name-error", BankFixture::BadNames, "incomplete"),
    ] {
        let child = temp.path().join(subdir);
        fs::create_dir(&child)?;
        let archive = fixture(&child, fixture_kind)?;
        let output = child.join("output");
        let mut options = options()?;
        options.diagnostic = true;
        let ExportOutcome::Incomplete { diagnostic, .. } =
            p4kx_media::export(&archive, &output, &options)?
        else {
            return Err("damaged source remains diagnostically incomplete".into());
        };
        let rows = fs::read_to_string(
            diagnostic
                .parent()
                .ok_or("diagnostics root")?
                .join("animation.jsonl"),
        )?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<MediaRecord>, _>>()?;
        verify_banks(&rows, expected)?;
        assert_eq!(
            rows.iter()
                .filter(|row| row.subresource.namespace == MATCH)
                .count(),
            4
        );
    }
    Ok(())
}

fn options() -> Result<ExportOptions, Box<dyn Error>> {
    Ok(ExportOptions {
        kinds: vec![MediaKind::Animation, MediaKind::AnimationDefinition],
        animation_entity: Some("FixtureEntity".into()),
        exclude: Exclusion::parse([FILTERED])?,
        ..ExportOptions::default()
    })
}

fn verify_banks<'a>(
    rows: &'a [MediaRecord],
    first_state: &str,
) -> Result<[&'a MediaRecord; 3], Box<dyn Error>> {
    let banks: Vec<_> = rows
        .iter()
        .filter(|row| row.subresource.namespace == BANK)
        .collect();
    let [first, filtered, repeated] = banks.as_slice() else {
        return Err("three same-list bank declarations".into());
    };
    for (row, parameter, matches, state) in [
        (*first, 2, 2, first_state),
        (*filtered, 3, 0, "not-selected"),
        (*repeated, 4, 2, first_state),
    ] {
        assert_eq!(
            row.subresource.indices,
            [0, 1, parameter],
            "the bank belongs to the original rig and alias"
        );
        assert_eq!(
            row.data.pointer("/payload/database_parameter"),
            Some(&json!(parameter)),
            "the physical parameter position survives"
        );
        assert_eq!(
            row.data.pointer("/payload/observed_matches"),
            Some(&json!(matches)),
            "only exact observed matches are counted"
        );
        assert_eq!(
            row.data.pointer("/payload/state"),
            Some(&json!(state)),
            "the recorded bank discovery state"
        );
    }
    assert_eq!(
        first.data.pointer("/payload/source"),
        repeated.data.pointer("/payload/source"),
        "repeated individual declarations share one physical bank"
    );
    assert_ne!(
        first.data.pointer("/payload/source"),
        filtered.data.pointer("/payload/source"),
        "the filtered source has its own physical position"
    );
    Ok([first, filtered, repeated])
}

fn compare_candidate(row: &MediaRecord, canonical: &Value, rows: &[MediaRecord]) -> TestResult {
    let source = row
        .data
        .pointer("/payload/source")
        .and_then(Value::as_u64)
        .ok_or("source row")?;
    let clip = row
        .data
        .pointer("/payload/clip_ordinal")
        .and_then(Value::as_u64)
        .ok_or("physical ordinal")?;
    assert!(
        matches!(clip, 0 | 2),
        "the same basename on a different path must stay unmatched"
    );
    assert_eq!(
        canonical.get("clip_ordinal"),
        Some(&json!(clip.to_string())),
        "both projections keep the original DBA clip ordinal"
    );
    for member in ["authored_name", "name_range", "metadata_range"] {
        assert_eq!(
            row.data.get("payload").and_then(|p| p.get(member)),
            canonical.get(member),
            "the {member} matches the Definitions projection"
        );
    }
    let physical = rows
        .iter()
        .find(|row| {
            row.subresource.namespace == "animation-entity-clip"
                && row.subresource.indices == [source, clip]
        })
        .ok_or("entity physical clip")?;
    assert_eq!(
        physical.data.pointer("/payload/physical_record/source"),
        canonical.get("entry"),
        "the physical entity clip resolves the same bank as the definition"
    );
    Ok(())
}

fn verify_origins(
    base: &Path,
    rows: &[MediaRecord],
    matches: &[(usize, &MediaRecord)],
) -> TestResult {
    let text = fs::read_to_string(base.join("field-origins.jsonl"))?;
    let fields = text
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let filtered = rows
        .iter()
        .position(|row| row.subresource.namespace == BANK && row.subresource.indices == [0, 1, 3])
        .ok_or("filtered bank")?;
    let proof = |row: usize, address: &str, encrypted: bool| -> TestResult {
        let ordinal = row.to_string();
        let origin = fields
            .iter()
            .find(|field| {
                field.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("animation")
                    && field.pointer("/scope/ordinal").and_then(Value::as_str) == Some(&ordinal)
                    && field.get("field").and_then(Value::as_str) == Some(address)
            })
            .ok_or("per-field origin")?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "{address}"
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(true)),
            "{address}"
        );
        Ok(())
    };
    proof(filtered, "/data/payload/source", false)?;
    proof(filtered, "/data/payload/state", false)?;
    for &(ordinal, _) in matches {
        proof(ordinal, "/data/payload/authored_name", true)?;
        proof(ordinal, "/data/payload/clip_ordinal", true)?;
        proof(ordinal, "/data/payload/database_parameter", false)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum BankFixture {
    Intact,
    BadControllers,
    BadNames,
}

fn fixture(root: &Path, kind: BankFixture) -> Result<Archive, Box<dyn Error>> {
    let entity = TestEntity::new(1, "FixtureEntity", "entities/fixture.xml")
        .geometry("Objects/Fixture/actor.chr", None);
    let forge = entity_schema(DataForgeBuilder::new())
        .record(entity.record())
        .build();
    let path = root.join("Data.p4k");
    let first = TestDba::new()
        .animation(TestAnimation::new("Animations/Fixture/alias.caf", 30))
        .animation(TestAnimation::new("Animations/Other/alias.caf", 24))
        .animation(TestAnimation::new(
            "DATA\\ANIMATIONS\\FIXTURE\\ALIAS.CAF",
            60,
        ))
        .animation(TestAnimation::new("Animations/Fixture/end.caf", 24))
        .bytes();
    let other = TestDba::new()
        .animation(TestAnimation::new("Data/Animations/Fixture/alias.caf", 20))
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge))
        .entry(TestEntry::stored(
            "Data/Objects/Fixture/actor.chrparams",
            PARAMS,
        ))
        .entry(TestEntry::stored(
            "Data/Objects/Fixture/actor.chr",
            TestModel::cube()
                .with_skeleton(&TestSkeleton::two_bones())
                .cgf(),
        ))
        .entry(TestEntry::stored(FIRST, damage(first, kind)?).encrypted())
        .entry(TestEntry::stored(FILTERED, other.clone()).encrypted())
        .entry(TestEntry::stored(OTHER_LIST, other))
        .write_to(&path)?;
    Ok(Archive::open_with(
        path,
        OpenOptions::new().key(Some(test_key())),
    )?)
}

fn damage(first: Vec<u8>, kind: BankFixture) -> Result<Vec<u8>, Box<dyn Error>> {
    if matches!(kind, BankFixture::Intact) {
        return Ok(first);
    }
    let parsed = ChunkFile::parse(first.as_slice())?;
    let (index, metadata) = parsed
        .chunks_of(ivo::DBA_METADATA)
        .next()
        .ok_or("metadata chunk")?;
    let mut bytes = parsed.chunk_data(index).ok_or("metadata bytes")?.to_vec();
    if matches!(kind, BankFixture::BadNames) {
        *bytes.last_mut().ok_or("final terminator")? = b'x';
    }
    Ok(TestChunkFile::ivo()
        .chunk(ivo::DBA_DATA.id(), 0x900, b"\0\0\0\0bad block")
        .chunk(ivo::DBA_METADATA.id(), metadata.version(), bytes)
        .bytes())
}
