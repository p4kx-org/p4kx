// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Seek, Write};

use p4kx_export::{CatalogError, Compression, Kind, Language, SCHEMA_VERSION};
use p4kx_extraction::LocalizationSet;
use serde_json::{Value, json};

use super::{TestResult, TextPaths, pipe, write_catalog};
use crate::catalog_v3_consumer;
use crate::commodity_v3_fixture::{self as fixture, DcbOrigin};
use crate::real_archive_canonical_texts::{TextIndex, kind_coverage};
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_texts::{Coverage, check_coverage, text_fields};

#[test]
fn text_census_matches_a_published_synthetic_catalog() -> TestResult<()> {
    let data = fixture::with_keys("@shared", "@shared", "@shared");
    let (_folder, archive) = fixture::open(&data, DcbOrigin::Plain)?;
    let options = fixture::options();
    let (file_summary, mut file) =
        p4kx_export::write_catalog(&archive, &options, Compression::None, tempfile::tempfile)?;
    file.rewind()?;
    let catalog: Value = serde_json::from_reader(io::BufReader::new(file))?;
    let mut writing = false;
    let (pipe_summary, projected) = write_catalog(&archive, &options, || writing = true)?;
    assert!(writing, "the actual writer reached its sink");
    assert_eq!(pipe_summary.size(), file_summary.size());
    assert_eq!(projected["source"], catalog["source"]);
    assert_eq!(projected["texts"], catalog["texts"]);
    assert!(projected.get("placements").is_none());
    assert!(projected.get("provenance").is_none());

    let locales = LocalizationSet::read(&archive, &[Language::English, Language::German])?;
    let english = Oracle::of(&locales, Language::English)?;
    let german = Oracle::of(&locales, Language::German)?;
    let fields = text_fields();
    for kind in Kind::ALL {
        let result = |document| compare(document, kind, &fields, &english, &german);
        assert_eq!(
            result(&projected),
            result(&catalog),
            "text checks for {kind}"
        );
    }

    let mut corrupted = catalog;
    corrupt_german_display(&mut corrupted)?;
    let serialized = serde_json::to_vec(&corrupted)?;
    let filtered = TextPaths::catalog().read(serialized.as_slice())?;
    let expected = compare(&corrupted, Kind::ResourceTypes, &fields, &english, &german)?;
    assert!(
        expected
            .failures
            .iter()
            .any(|failure| failure.contains("de texts contradict")),
        "an invented German text must fail",
    );
    assert_eq!(
        compare(&filtered, Kind::ResourceTypes, &fields, &english, &german)?,
        expected,
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct CheckedKind {
    row_count: usize,
    english: Coverage,
    german: Coverage,
    failures: Vec<String>,
}

fn compare(
    document: &Value,
    kind: Kind,
    fields: &[String],
    english: &Oracle,
    german: &Oracle,
) -> Result<CheckedKind, String> {
    let rows = catalog_v3_consumer::rows(document, kind).map_err(|error| error.to_string())?;
    let index = TextIndex::new(document).map_err(|error| error.to_string())?;
    let (english, german) = kind_coverage(document, &index, kind, fields, english, german)
        .map_err(|error| error.to_string())?;
    let mut failures = Vec::new();
    check_coverage(kind, &english, &german, false, rows.len(), &mut failures);
    Ok(CheckedKind {
        row_count: rows.len(),
        english,
        german,
        failures,
    })
}

fn corrupt_german_display(catalog: &mut Value) -> TestResult<()> {
    let row = catalog
        .pointer_mut("/texts/publicNames")
        .and_then(Value::as_array_mut)
        .ok_or("public names")?
        .iter_mut()
        .find(|row| row.pointer("/key/value").and_then(Value::as_str) == Some("@shared"))
        .ok_or("canonical German name")?;
    let shown = row
        .pointer_mut("/translations/value/de/displayText/value")
        .ok_or("German display payload")?;
    *shown = json!("Invented German text");
    Ok(())
}

#[test]
fn text_census_preserves_mission_positions_and_rejects_invalid_json() -> TestResult<()> {
    let input = json!({
        "schemaVersion": SCHEMA_VERSION,
        "definitions": {
            "contractTemplates": {
                "availability": "available",
                "rows": [{"source": {"values": [null, {"theAuthoredValue": "@third"}, null]}}]
            }
        },
        "configurations": {
            "contractTemplates": {
                "availability": "available",
                "rows": [{"localizations": [{"source": {"valueIndex": 1}}]}]
            },
            "irrelevant": [1, 2]
        },
        "projections": {"contracts": {"availability": "absent", "rows": []}},
        "provenance": {"ignored": ["list", "of", "coordinates"]},
        "futureField": true
    });
    let paths = TextPaths::catalog();
    let mut encoded = serde_json::to_vec(&input)?;
    let projected = paths.read(encoded.as_slice())?;
    assert_eq!(projected["definitions"], input["definitions"]);
    assert_eq!(
        projected["configurations"]["contractTemplates"],
        input["configurations"]["contractTemplates"],
    );
    assert_eq!(projected["projections"], input["projections"]);
    assert!(projected["configurations"].get("irrelevant").is_none());
    assert!(projected.get("provenance").is_none());
    assert!(projected.get("futureField").is_none());
    encoded.extend_from_slice(b" {}");
    assert!(
        paths.read(encoded.as_slice()).is_err(),
        "unexpected trailing document"
    );
    assert!(
        paths.read(br#"{"provenance":[oops]}"#.as_slice()).is_err(),
        "discarded subtree still needs valid JSON",
    );
    Ok(())
}

#[test]
fn text_census_pipe_closes_on_producer_or_consumer_failure() {
    const PAYLOAD_LEN: usize = 256 * 1024;
    let paths = TextPaths::catalog();
    let canceled = pipe::transfer::<()>(&paths, |_writer| Err(CatalogError::Cancelled))
        .expect_err("an early producer failure closes the pipe");
    assert!(matches!(
        canceled.downcast_ref::<CatalogError>(),
        Some(CatalogError::Cancelled)
    ));

    let producer = pipe::transfer::<()>(&paths, |mut writer| {
        writer
            .write_all(b"{\"source\":")
            .map_err(CatalogError::Write)?;
        Err(CatalogError::Write(io::Error::other(
            "fixture write failed",
        )))
    })
    .expect_err("a failed producer closes the pipe");
    assert!(matches!(
        producer.downcast_ref::<CatalogError>(),
        Some(CatalogError::Write(source)) if source.to_string() == "fixture write failed"
    ));

    let input = vec![b'['; PAYLOAD_LEN];
    let consumer = pipe::transfer(&paths, |mut writer| {
        writer.write_all(&input).map_err(CatalogError::Write)?;
        Ok(((), writer))
    })
    .expect_err("an early parser failure closes its read end");
    assert!(consumer.downcast_ref::<serde_json::Error>().is_some());
}
