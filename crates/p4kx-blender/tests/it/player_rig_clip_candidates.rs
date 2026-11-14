// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_assembly::BodyVariant;
use p4kx_blender::{player_bundle_record_schema, write_player_source_bundle};
use p4kx_extraction::CancelToken;
use p4kx_model::CafImage;
use p4kx_testkit::{TestAnimation, TestChunkFile, TestDba, TestEntry};
use serde_json::{Value, json};

use super::{TestResult, fixture_with_entries};

const BANK: &str = "Data/Animations/Fixture/authorized.dba";
const OTHER_BANK: &str = "Data/Animations/Fixture/another-list.dba";
const LOOSEN: &str = "Data/Animations/Fixture/Loose.caf";
const SHARED: &str = "Animations/Fixture/sub/Shared.caf";
const PARAMS: &[u8] = br##"<Params>
<AnimationList>
  <Animation name="#filepath" path="Animations/Fixture"/>
  <Other/>
  <Animation name="*" path="*\*.CAF"/>
  <Animation name="$TracksDatabase" path="Animations/Fixture/authorized.dba"/>
  <Animation name="*" path="*/*.comb"/>
</AnimationList>
<Other/>
<AnimationList>
  <Animation name="*" path="*/*.caf"/>
  <Animation name="$TracksDatabase" path="Animations/Fixture/another-list.dba"/>
</AnimationList></Params>"##;

#[test]
fn rig_clip_candidates_use_original_folder_and_same_list_physical_bank_only() -> TestResult {
    let rows = capture(true)?;
    let clips = family(&rows, "rigClipCandidate");
    let [first, duplicate, other_scope, loose, unsupported_alias] = clips.as_slice() else {
        return Err("expected exactly five folder-bounded physical occurrences".into());
    };
    assert_eq!(
        first.pointer("/value/aliasKey/value"),
        Some(&json!("shared")),
        "ASCII alias joins the independently scoped declarations"
    );
    assert_eq!(
        first.pointer("/value/record"),
        Some(&json!(0)),
        "first image record keeps its ordinal"
    );
    assert_eq!(
        duplicate.pointer("/value/record"),
        Some(&json!(1)),
        "a duplicate image path retains its distinct ordinal"
    );
    assert_eq!(
        other_scope.pointer("/value/record"),
        Some(&json!(2)),
        "an out-of-scope bank retains the image observation"
    );
    assert_eq!(
        loose.pointer("/value/record"),
        Some(&json!(3)),
        "zero CRC retains the original loose occurrence"
    );
    assert_exact_metadata([first, duplicate]);
    assert_eq!(
        other_scope.pointer("/value/source/value/candidates"),
        Some(&json!([])),
        "another list bank cannot enter the result"
    );
    assert_eq!(
        loose.pointer("/value/source/value/type"),
        Some(&json!("loose")),
        "zero CRC is represented as a loose lookup"
    );
    assert_eq!(
        loose.pointer("/value/source/value/target/value/type"),
        Some(&json!("found")),
        "the loose archive Entry has an exact path match"
    );
    assert_eq!(
        unsupported_alias.pointer("/value/record"),
        Some(&json!(4)),
        "an unsupported comparison key keeps its physical image position"
    );
    assert_eq!(
        unsupported_alias.pointer("/value/aliasKey/state"),
        Some(&json!("unsupported")),
        "the raw archive path does not need a supported alias key"
    );
    assert_eq!(
        unsupported_alias.pointer("/value/path/value"),
        Some(&json!("Animations/Fixture/sub/🟣.caf")),
        "the raw path survives independently of its key"
    );
    assert_rules(&rows)
}

fn assert_exact_metadata(clips: [&Value; 2]) {
    let candidate = "/value/source/value/candidates";
    for result in clips {
        assert_eq!(
            result.pointer(&format!("{candidate}/0/bank")),
            Some(&json!(0)),
            "only the same authored list bank can contribute"
        );
        assert_eq!(
            result.pointer(&format!("{candidate}/0/records/0/ordinal")),
            Some(&json!(0)),
            "first exact DBA name retains its complete ordinal"
        );
        assert_eq!(
            result.pointer(&format!("{candidate}/0/records/1/ordinal")),
            Some(&json!(2)),
            "same basename in a different database folder is excluded"
        );
        assert_eq!(
            result.pointer("/value/path/containsEncryptedSource"),
            Some(&json!(false)),
            "the indexed path is independently clear"
        );
        assert_eq!(
            result.pointer("/value/source/containsEncryptedSource"),
            Some(&json!(true)),
            "bank metadata contributes to the candidate origin"
        );
        assert_eq!(
            result.pointer(&format!(
                "{candidate}/0/records/0/name/containsEncryptedSource"
            )),
            Some(&json!(true)),
            "the exact name is independently encrypted"
        );
    }
}

#[test]
fn rig_clip_candidates_keep_ambiguous_physical_bank_without_inspecting_either_entry() -> TestResult
{
    let rows = capture(false)?;
    let banks = family(&rows, "rigClipBank");
    let Some(bank) = banks.first() else {
        return Err("expected authored bank".into());
    };
    assert_eq!(
        bank.pointer("/value/target/value/type"),
        Some(&json!("ambiguous")),
        "physical duplicate path resolves as ambiguous"
    );
    let physical = bank
        .pointer("/value/target/value/candidates")
        .and_then(Value::as_array)
        .ok_or("expected physical bank positions")?;
    assert_eq!(physical.len(), 2, "both physical Entrys are retained");
    assert_ne!(
        physical.first(),
        physical.last(),
        "physical duplicates have distinct source identities"
    );
    assert_eq!(
        bank.pointer("/value/metadata"),
        Some(&Value::Null),
        "ambiguous bank payloads are unused"
    );
    let clips = family(&rows, "rigClipCandidate");
    let Some(first) = clips.first() else {
        return Err("expected physical clip".into());
    };
    assert_eq!(
        first.pointer("/value/source/value/candidates/0/records"),
        Some(&json!([])),
        "an ambiguous physical bank has no proven records"
    );
    assert_eq!(
        first.pointer("/value/source/containsEncryptedSource"),
        Some(&json!(false)),
        "unread attempted encrypted inputs do not contribute"
    );
    Ok(())
}

fn assert_rules(rows: &[Value]) -> TestResult {
    let rules = family(rows, "rigClipRule");
    let [folder, mask, _bank, unsupported, reset, _other_bank] = rules.as_slice() else {
        return Err("expected source order for every original rule".into());
    };
    assert_eq!(
        folder.pointer("/value/child"),
        Some(&json!(0)),
        "folder keeps its authored child ordinal"
    );
    assert_eq!(
        mask.pointer("/value/child"),
        Some(&json!(2)),
        "intervening XML child stays counted"
    );
    assert_eq!(
        mask.pointer("/value/kind/value/folderRule"),
        Some(&json!(0)),
        "the mask links to its active folder rule"
    );
    assert_eq!(
        unsupported.pointer("/value/kind/value/type"),
        Some(&json!("unsupported")),
        "a blendspace wildcard is retained in its own unsupported rule"
    );
    assert_eq!(
        reset.pointer("/value/list"),
        Some(&json!(2)),
        "original list coordinate includes intervening elements"
    );
    assert_eq!(
        reset.pointer("/value/kind/value/type"),
        Some(&json!("invalid")),
        "folder state does not cross an AnimationList boundary"
    );
    let rigs = family(rows, "rigClipRig");
    let [male, female] = rigs.as_slice() else {
        return Err("expected requested rigs".into());
    };
    let document = male
        .pointer("/value/animationList/value")
        .ok_or("missing XML document")?;
    assert!(
        family(rows, "document")
            .iter()
            .any(|row| row.get("index") == Some(document)),
        "the emitted document preserves exact CHRPARAMS original bytes"
    );
    assert_eq!(
        female.pointer("/value/skeleton/value/type"),
        Some(&json!("missing")),
        "missing second body rig remains explicit"
    );
    Ok(())
}

fn capture(unique: bool) -> TestResult<Vec<Value>> {
    let skeleton = format!("Data/{}", BodyVariant::Male.skeleton());
    let stem = skeleton.strip_suffix(".chr").ok_or("skeleton extension")?;
    let images = cif_image()?;
    let mut entries = vec![
        TestEntry::stored(&skeleton, b"rig"),
        TestEntry::stored(format!("{stem}.chrparams"), PARAMS),
        TestEntry::stored("Data/Animations/Animations.img", images),
        TestEntry::stored(LOOSEN, b"loose"),
        TestEntry::stored(BANK, dba()).encrypted(),
        TestEntry::stored(OTHER_BANK, dba()),
    ];
    if !unique {
        entries.push(TestEntry::stored(BANK.to_ascii_lowercase(), dba()));
    }
    let (_input, archive) = fixture_with_entries(entries)?;
    let output = tempfile::tempdir()?;
    let path = output.path().join("bundle");
    write_player_source_bundle(
        &archive,
        &[BodyVariant::Male, BodyVariant::Female],
        &path,
        &CancelToken::new(),
    )?;
    let rows = fs::read_to_string(path.join("records.jsonl"))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<Value>, _>>()?;
    let validator = jsonschema::validator_for(&player_bundle_record_schema())?;
    for row in &rows {
        validator.validate(row).map_err(|error| error.to_string())?;
    }
    Ok(rows)
}

fn cif_image() -> TestResult<Vec<u8>> {
    let bank = |path: &str| {
        p4kx_extraction::animation_bank_path_crc32(path).ok_or("invalid synthetic bank path")
    };
    let paths = [
        (SHARED, bank(BANK)?),
        ("data/animations/fixture/SUB/shared.caf", bank(BANK)?),
        (SHARED, bank(OTHER_BANK)?),
        ("Animations\\Fixture\\Loose.caf", 0),
        ("Animations/Fixture/sub/🟣.caf", 0),
        ("Animations/FixtureOutside/sub/shared.caf", bank(BANK)?),
    ];
    let mut records = (u32::try_from(paths.len())?).to_le_bytes().to_vec();
    records.extend(2_u32.to_le_bytes());
    for (path, crc) in paths {
        records.extend(path.as_bytes());
        records.resize(records.len() + 256 - path.len(), 0);
        records.extend(0_u32.to_le_bytes());
        records.extend(crc.to_le_bytes());
        records.resize(records.len() + 308 - 264, 0);
    }
    records.extend([0; 8]);
    Ok(TestChunkFile::ivo()
        .chunk(CafImage::CHUNK_TYPE, CafImage::CHUNK_VERSION, records)
        .bytes())
}

fn dba() -> Vec<u8> {
    TestDba::new()
        .animation(TestAnimation::new(SHARED, 30))
        .animation(TestAnimation::new("Animations/Other/sub/Shared.caf", 30))
        .animation(TestAnimation::new(
            "data\\animations\\fixture\\SUB\\shared.caf",
            30,
        ))
        .bytes()
}

fn family<'a>(rows: &'a [Value], kind: &str) -> Vec<&'a Value> {
    rows.iter()
        .filter(|row| row.get("type").and_then(Value::as_str) == Some(kind))
        .collect()
}
