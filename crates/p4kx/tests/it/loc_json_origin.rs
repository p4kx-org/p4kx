// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::TestEntry;

use super::{TestResult, archive, p4kx, stdout_of};

#[derive(serde::Deserialize, serde::Serialize)]
struct LookupPayload {
    key: String,
    language: String,
    fallback: Option<String>,
    status: String,
    text_language: Option<String>,
    text: Option<String>,
}

pub(super) fn payload_snapshot(json: &str) -> Result<String, Box<dyn Error>> {
    let payload: LookupPayload = serde_json::from_str(json)?;
    let mut text = serde_json::to_string(&payload)?;
    text.push('\n');
    Ok(text)
}

#[test]
fn get_json_marks_the_winning_translation_and_the_consulted_entries_separately() -> TestResult {
    let dir = tempfile::tempdir()?;
    let input = archive(
        dir.path(),
        vec![
            TestEntry::stored(
                "Data/Localization/english/global.ini",
                b"same=First\nsame,P=Alternate\nsame=Final\\nline\nempty=Fallback\n".to_vec(),
            ),
            TestEntry::stored(
                "Data/Localization/german_(germany)/global.ini",
                b"empty=\nother=Eigen\n".to_vec(),
            )
            .encrypted(),
        ],
    )?;
    let get = |key| -> Result<serde_json::Value, Box<dyn Error>> {
        let json = stdout_of(
            p4kx()?
                .args([
                    "--json",
                    "--key",
                    &p4kx_testkit::test_key().to_hex(),
                    "loc",
                    "get",
                ])
                .arg(&input)
                .args([key, "--lang", "de", "--fallback", "en"]),
            0,
        )?;
        Ok(serde_json::from_str(&json)?)
    };
    check_fallback_origin(&get("same")?)?;

    let own = get("other")?;
    assert_eq!(own["text"], "Eigen", "the preferred language wins");
    assert_origin(&own, "/text", true, 2)?;
    assert_origin(&own, "/status", true, 1)?;
    assert_eq!(
        own["provenance"]["sources"]
            .as_array()
            .ok_or("sources")?
            .len(),
        1,
        "the unused fallback file contributes no provenance"
    );

    let empty = get("empty")?;
    assert_eq!(
        empty["text"], "Fallback",
        "an empty value triggers fallback"
    );
    assert_origin(&empty, "/text", true, 3)?;
    assert_origin(&empty, "/status", true, 2)?;
    Ok(())
}

fn check_fallback_origin(fallback: &serde_json::Value) -> TestResult {
    assert_eq!(
        fallback["text"], "Final\nline",
        "the last plain line wins over the alternate"
    );
    assert_eq!(
        fallback["status"], "fallback",
        "the selected language has no matching text"
    );
    assert_origin(fallback, "/text", true, 4)?;
    assert_origin(fallback, "/status", true, 2)?;
    assert_origin(fallback, "/text_language", true, 2)?;
    assert_origin(fallback, "/key", false, 0)?;
    let sources = fallback
        .pointer("/provenance/uses")
        .ok_or("use inventory")?
        .as_array()
        .ok_or("source uses")?;
    let text_refs = fallback
        .pointer("/fieldOrigins/~1text/sourceUsesRefs")
        .ok_or("text origin")?
        .as_array()
        .ok_or("text source references")?;
    let winner = sources
        .iter()
        .find(|row| row["role"] == "payload" && text_refs.contains(&row["id"]))
        .ok_or("winning payload")?;
    assert_eq!(
        winner["location"],
        serde_json::json!({"type":"subresource","namespace":"globalIniDecodedText","indices":["33","44"],"field":"value"}),
        "the winning payload has its exact decoded byte range"
    );
    assert_eq!(
        winner["effectiveEncryption"], "unencrypted",
        "the actual translation input is plain"
    );

    Ok(())
}

fn assert_origin(
    report: &serde_json::Value,
    field: &str,
    encrypted: bool,
    count: usize,
) -> TestResult {
    let origin = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(field))
        .ok_or("field origin")?;
    assert_eq!(
        origin
            .get("containsEncryptedSource")
            .and_then(serde_json::Value::as_bool),
        Some(encrypted),
        "{field} has the exact encryption marker"
    );
    assert_eq!(
        origin
            .get("sourceSetComplete")
            .and_then(serde_json::Value::as_bool),
        Some(true),
        "{field} has a complete source set"
    );
    let refs = origin
        .get("sourceUsesRefs")
        .ok_or("field source uses")?
        .as_array()
        .ok_or("source references")?;
    assert_eq!(
        refs.len(),
        count,
        "{field} has the exact number of contributing uses"
    );
    let uses = report
        .pointer("/provenance/uses")
        .ok_or("use inventory")?
        .as_array()
        .ok_or("source uses")?;
    assert!(
        refs.iter()
            .all(|id| uses.iter().any(|row| row.get("id") == Some(id))),
        "{field} refers only to present source uses"
    );
    Ok(())
}
