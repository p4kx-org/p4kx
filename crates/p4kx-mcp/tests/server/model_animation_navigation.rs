// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestAnimation, TestChunkFile, TestDba, TestEntry, TestModel, TestTrack};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

const MODEL: &str = "Data/Objects/Nav/cube.cgf";
const FOLDER: &str = "Data/Animations/Objects/Nav";
const ADMISSION: &str = "cryEngineAnimationAdmission";

#[test]
fn an_encrypted_clip_outside_the_page_contributes_to_complete_navigation() -> TestResult {
    let file = caf(false);
    let start = *file.offsets().first().ok_or("CAF metadata offset")?;
    let install = install_with(vec![
        TestEntry::stored(format!("{FOLDER}/first.caf"), file.bytes()),
        TestEntry::stored(format!("{FOLDER}/later.caf"), file.bytes()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let response = server.call("list_model_items", &query())?;
    let reply = response
        .get("structuredContent")
        .ok_or("animation response")?;
    assert_eq!(reply.get("total"), Some(&json!(2)), "both clips count");
    assert_eq!(
        reply.pointer("/items/0/name"),
        Some(&json!("first")),
        "only the first clip is on this page"
    );
    for output in ["total", "next_cursor"] {
        marker(field(reply, output)?, true);
        assert_eq!(
            admissions(reply, output)?.len(),
            2,
            "both physical sources are represented"
        );
        assert_site(
            reply,
            output,
            ADMISSION,
            "clipFlags",
            &[
                "0",
                "0",
                &start.to_string(),
                &(start + 4).to_string(),
                "4",
                "1",
            ],
        )?;
        assert!(
            admissions(reply, output)?
                .iter()
                .any(|site| site.get("effectiveEncryption") == Some(&json!("encrypted"))),
            "navigation retains the proved off-page input"
        );
    }
    marker(field(reply, "items")?, false);
    marker(field(reply, "items/0/frames_per_second")?, false);
    assert_all_addresses(&response)
}

#[test]
fn plaintext_navigation_is_complete_and_discards_failed_encrypted_inputs() -> TestResult {
    let install = install_with(vec![
        TestEntry::stored(format!("{FOLDER}/first.caf"), caf(false).bytes()),
        TestEntry::stored(format!("{FOLDER}/next.caf"), caf(false).bytes()),
        TestEntry::stored(format!("{FOLDER}/empty.dba"), TestDba::new().bytes()),
        TestEntry::stored(format!("{FOLDER}/broken.caf"), "invalid chunk file").encrypted(),
        TestEntry::stored(format!("{FOLDER}/rejected.dba"), rejected_database()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok("list_model_items", &query())?;
    assert_eq!(
        reply.get("total"),
        Some(&json!(2)),
        "the empty database and failed clip add no output rows"
    );
    for output in ["total", "next_cursor"] {
        marker(field(&reply, output)?, false);
        assert_eq!(
            admissions(&reply, output)?.len(),
            3,
            "only completed admission inputs contribute"
        );
        assert!(
            admissions(&reply, output)?.iter().any(|site| {
                site.pointer("/location/field") == Some(&json!("declaredClipCount"))
            }),
            "the empty database's authored count was used"
        );
    }
    marker(field(&reply, "items")?, false);
    Ok(())
}

#[test]
fn a_readable_additive_flag_contributes_to_navigation_when_its_row_is_excluded() -> TestResult {
    let install = install_with(vec![
        TestEntry::stored(format!("{FOLDER}/first.caf"), caf(false).bytes()),
        TestEntry::stored(format!("{FOLDER}/additive.caf"), caf(true).bytes()).encrypted(),
    ])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let reply = server.ok("list_model_items", &query())?;
    assert_eq!(
        reply.get("total"),
        Some(&json!(1)),
        "additive clip is still excluded"
    );
    assert!(
        reply.get("next_cursor").is_none(),
        "there is no second output row"
    );
    marker(field(&reply, "total")?, true);
    assert_eq!(
        admissions(&reply, "total")?.len(),
        2,
        "both accepted and rejected flags were evaluated"
    );
    marker(field(&reply, "items")?, false);
    Ok(())
}

#[test]
fn oversized_navigation_keeps_a_proved_encrypted_witness_without_losing_rows() -> TestResult {
    let count = 480;
    let bytes = caf(false).bytes();
    let mut entries: Vec<_> = (0..count)
        .map(|index| {
            TestEntry::stored(
                format!("{FOLDER}/{index:04}-{}.caf", "x".repeat(460)),
                bytes.clone(),
            )
        })
        .collect();
    entries.push(TestEntry::stored(format!("{FOLDER}/z-later.caf"), bytes).encrypted());
    let install = install_with(entries)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut query = query();
    let first = server.ok("list_model_items", &query)?;
    assert_eq!(
        first.get("total"),
        Some(&json!(count + 1)),
        "the count includes every admitted clip"
    );
    assert!(
        serde_json::to_vec(&first)?.len() <= 256 * 1024,
        "the complete response fits the existing transport budget"
    );
    assert_encrypted_witness(&first)?;
    marker(field(&first, "items")?, false);
    assert_eq!(
        first.pointer("/items/0/name"),
        Some(&json!(format!("0000-{}", "x".repeat(460)))),
        "the first value stays whole"
    );
    query["cursor"] = first.get("next_cursor").ok_or("next page")?.clone();
    let second = server.ok("list_model_items", &query)?;
    assert_eq!(
        second.pointer("/items/0/name"),
        Some(&json!(format!("0001-{}", "x".repeat(460)))),
        "the existing cursor advances once"
    );
    Ok(())
}

fn assert_encrypted_witness(reply: &Value) -> TestResult {
    for output in ["total", "next_cursor"] {
        let evidence = field(reply, output)?;
        assert_eq!(
            evidence.get("sourceSetComplete"),
            Some(&json!(false)),
            "witness is explicitly partial"
        );
        assert_eq!(
            evidence.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the known positive survives an incomplete inventory"
        );
        let sites = admissions(reply, output)?;
        assert_eq!(
            sites.len(),
            1,
            "the existential witness is sufficient for the positive marker"
        );
        assert!(
            sites
                .iter()
                .all(|site| site.get("effectiveEncryption") == Some(&json!("encrypted"))),
            "the retained witness is the actual encrypted input"
        );
    }
    Ok(())
}

fn admissions<'a>(reply: &'a Value, output: &str) -> TestResult<Vec<&'a Value>> {
    Ok(uses(reply, output)?
        .into_iter()
        .filter(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some(ADMISSION)
        })
        .collect())
}

fn query() -> Value {
    json!({"path": MODEL, "section": "animations", "limit": 1})
}

fn install_with(mut entries: Vec<TestEntry>) -> TestResult<Install> {
    let cube = TestModel::cube();
    entries.push(TestEntry::stored(MODEL, cube.cgf()));
    entries.push(TestEntry::stored("Data/Objects/Nav/cube.cgfm", cube.cgfm()));
    Install::new(entries)
}

fn rejected_database() -> Vec<u8> {
    TestDba::new()
        .animation(
            TestAnimation::new("unread.caf", 30).track(
                TestTrack::new(1)
                    .rotations(&[(0, [0.0, 0.0, 0.0, 1.0])])
                    .rotation_code(0xFF),
            ),
        )
        .bytes()
}

fn caf(additive: bool) -> TestChunkFile {
    let clip = TestAnimation::new("clip.caf", 30);
    let clip = if additive { clip.additive() } else { clip };
    TestChunkFile::ivo()
        .chunk(0x4733_C6ED, 0x901, clip.anim_info())
        .chunk(0xA949_6CB5, 0x900, clip.caf_data())
}
