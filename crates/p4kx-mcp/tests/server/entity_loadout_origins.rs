// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestEntry, sample_ship};
use serde_json::{Value, json};

use crate::audio_origin_assertions as proof;
use crate::support::{Install, TestResult};

const TOOL: &str = "entity_loadout";
const DCB: &str = "Data/Game2.dcb";
const ROOT: &str = "/structuredContent";

#[test]
fn loadout_identity_receipts_follow_the_original_records_on_cached_pages() -> TestResult {
    for encrypted in [false, true] {
        let bytes = sample_ship().build();
        let entry = TestEntry::deflated(DCB, bytes.as_slice());
        let entry = if encrypted { entry.encrypted() } else { entry };
        let install = Install::new(vec![entry])?;
        let mut server = install.serve(&["--archive", "LIVE"])?;
        let tools = server.tools()?;
        let schema = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!(TOOL)))
            .and_then(|tool| tool.get("outputSchema"))
            .ok_or("Loadout output schema")?;
        let schema = jsonschema::validator_for(schema)?;
        let mut args = json!({"entity":"TEST_Ship", "limit":1});
        let mut attached = 0;
        loop {
            let reply = server.ok(TOOL, &args)?;
            schema.validate(&reply).map_err(|error| {
                format!(
                    "loadout receipt at {} against {}: {error}",
                    error.instance_path(),
                    error.schema_path()
                )
            })?;
            proof::assert_marker(&reply, &format!("{ROOT}/archive"), false)?;
            check_record(&reply, "/entity", &bytes, encrypted)?;
            if reply.pointer("/ports/0/entity").is_some() {
                check_record(&reply, "/ports/0/entity", &bytes, encrypted)?;
                attached += 1;
            }
            let fields = reply["fieldOrigins"].as_object().ok_or("field origins")?;
            assert!(
                fields
                    .keys()
                    .all(|key| !key.starts_with(&format!("{ROOT}/ports/1"))),
                "positions are relative to this emitted page"
            );
            let Some(cursor) = reply.get("next_cursor") else {
                break;
            };
            args["cursor"] = cursor.clone();
        }
        assert!(attached > 1, "several attached Records on separate pages");
    }
    Ok(())
}

fn check_record(reply: &Value, entity: &str, bytes: &[u8], encrypted: bool) -> TestResult {
    for member in ["guid", "record_path", "name", "class_name"] {
        let path = format!("{ROOT}{entity}/{member}");
        proof::assert_marker(reply, &path, encrypted)?;
        for site in proof::uses(reply, &path)? {
            assert_eq!(proof::source_path(reply, site)?, DCB, "physical DCB owner");
        }
    }
    let uses = proof::uses(reply, &format!("{ROOT}{entity}/name"))?;
    let text = uses
        .into_iter()
        .find(|site| site.pointer("/location/namespace") == Some(&json!("dataforgeDecodedBytes")))
        .ok_or("original Record name bytes")?;
    let index = |ordinal: usize| -> TestResult<usize> {
        Ok(text
            .pointer(&format!("/location/indices/{ordinal}"))
            .and_then(Value::as_str)
            .ok_or("original byte coordinate")?
            .parse()?)
    };
    assert_eq!(
        bytes.get(index(0)?..index(1)?),
        reply
            .pointer(&format!("{entity}/name"))
            .and_then(Value::as_str)
            .map(str::as_bytes),
        "the attached identity points at exactly the bytes used"
    );
    Ok(())
}
