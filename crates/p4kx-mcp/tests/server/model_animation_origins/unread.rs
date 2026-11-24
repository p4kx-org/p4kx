// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestAnimation, TestChunkFile, TestEntry};
use serde_json::{Value, json};

use super::{ANIM_INFO, CAF_DATA, DBA, SKIN, assert_clip_span, install_with};
use crate::model_animation_fixture::{DatabaseFixture, dba_fixture};
use crate::model_item_origin_checks::{assert_all_addresses, field, marker, uses};
use crate::support::TestResult;

const ADDITIVE: &str = "Data/Animations/Objects/Probe/additive.caf";
const BROKEN: &str = "Data/Animations/Objects/Probe/broken.caf";
const SKIPPED: &str = "clip/unread.caf";

#[test]
fn unread_pages_bind_additive_flags_and_failed_dba_names_without_claiming_failed_payload()
-> TestResult {
    let DatabaseFixture {
        bytes,
        name: selected_at,
        ..
    } = dba_fixture()?;
    let skip_at = selected_at
        .checked_sub(SKIPPED.len() + 1)
        .ok_or("skipped name")?;
    let additive = TestAnimation::new("additive.caf", 30).additive();
    let caf = TestChunkFile::ivo()
        .chunk(ANIM_INFO, 0x901, additive.anim_info())
        .chunk(CAF_DATA, 0x900, additive.caf_data());
    let flag = *caf.offsets().first().ok_or("additive flag position")?;
    let install = install_with(
        vec![
            TestEntry::zstd(ADDITIVE, caf.bytes()).encrypted(),
            TestEntry::zstd(BROKEN, b"bad chunk file").encrypted(),
            TestEntry::zstd(DBA, bytes).encrypted(),
        ],
        false,
    )?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"path": SKIN, "section": "unread_animations", "limit": 1});
    for (source, name) in [
        (ADDITIVE, json!("additive")),
        (BROKEN, Value::Null),
        (DBA, json!("unread")),
    ] {
        let response = server.call("list_model_items", &args)?;
        let reply = assert_page_row(&response, source, &name)?;
        assert_reason(reply, source, flag)?;
        if source == DBA {
            assert_skipped_name(reply, skip_at as u64..selected_at as u64)?;
            assert!(
                reply.get("next_cursor").is_none(),
                "no duplicate unread row"
            );
        } else {
            args["cursor"] = reply.get("next_cursor").ok_or("unread cursor")?.clone();
        }
    }
    Ok(())
}

fn assert_page_row<'page>(
    response: &'page Value,
    source: &str,
    name: &Value,
) -> TestResult<&'page Value> {
    let reply = response.get("structuredContent").ok_or("unread page")?;
    assert_eq!(
        reply.get("total"),
        Some(&json!(3)),
        "three original unread rows"
    );
    assert_eq!(
        reply.pointer("/items/0/source"),
        Some(&json!(source)),
        "original source order"
    );
    assert_eq!(
        reply.pointer("/items/0/name"),
        Some(name),
        "existing name or null"
    );
    assert_all_addresses(response)?;
    marker(field(reply, "items/0/source")?, false);
    Ok(reply)
}

fn assert_reason(reply: &Value, source: &str, flag: u64) -> TestResult {
    let reason = field(reply, "items/0/reason")?;
    let expected_encryption = if source == ADDITIVE {
        assert_clip_span(reply, "reason", "additive", 0, 0, flag..flag + 4)?;
        json!(true)
    } else {
        assert!(
            uses(reply, "items/0/reason")?.is_empty(),
            "no retained bytes of a failed reason"
        );
        Value::Null
    };
    assert_eq!(
        reason.get("containsEncryptedSource"),
        Some(&expected_encryption),
        "only an actual additive flag gives positive reason evidence"
    );
    assert_eq!(
        reason.get("sourceSetComplete"),
        Some(&json!(false)),
        "failure admission is incomplete"
    );
    Ok(())
}

fn assert_skipped_name(reply: &Value, span: std::ops::Range<u64>) -> TestResult {
    assert_clip_span(reply, "name", "databasePath", 0, 1, span)?;
    assert_eq!(
        field(reply, "items/0/name")?.get("containsEncryptedSource"),
        Some(&json!(true)),
        "the name uses the successfully decoded metadata of the failed clip"
    );
    Ok(())
}
