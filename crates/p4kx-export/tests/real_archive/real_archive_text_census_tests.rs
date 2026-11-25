// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Seek, Write};

use p4kx_export::{CatalogError, Compression, Kind, Language, SCHEMA_VERSION};
use p4kx_extraction::LocalizationSet;
use serde_json::{Value, json};

use super::{TestResult, catalog_paths, pipe, write_catalog};
use crate::catalog_v3_consumer;
use crate::commodity_v3_fixture::{self as fixture, DcbOrigin};
use crate::real_archive_canonical_texts::{TextIndex, kind_coverage};
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_texts::{Coverage, check_coverage, text_fields, text_rows};

#[test]
fn text_census_matches_a_published_synthetic_catalog() -> TestResult<()> {
    let data = fixture::with_keys("@shared", "@shared", "@shared");
    let (_folder, archive) = fixture::open(&data, DcbOrigin::Plain)?;
    let options = fixture::options();
    let (file_summary, mut file) = p4kx_export::write_catalog(
        &archive,
        options.with_full_provenance(),
        Compression::None,
        tempfile::tempfile,
    )?;
    file.rewind()?;
    let catalog: Value = serde_json::from_reader(io::BufReader::new(file))?;
    let mut writing = false;
    let (pipe_summary, projected) = write_catalog(&archive, &options, || writing = true)?;
    assert!(writing, "the actual writer reached its sink");
    assert_eq!(pipe_summary.size(), file_summary.size());
    assert_eq!(projected["source"], catalog["source"]);
    assert_eq!(projected["texts"], catalog["texts"]);
    assert!(projected.get("placements").is_none());
    assert!(
        projected.pointer("/provenance/uses").is_none(),
        "the text census omits use rows"
    );
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
    let filtered = catalog_paths().read(serialized.as_slice())?;
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

#[test]
fn text_census_streams_loadout_rows_without_losing_legacy_candidates() -> TestResult<()> {
    let english = Oracle::parse("shared=Fixture\nother=Other\n");
    let german = Oracle::parse("shared=Beispiel\nother=Anders\n");
    let fields = text_fields();
    let input = loadout_text_fixture();
    let encoded = serde_json::to_vec(&input)?;
    let projected = catalog_paths().read(encoded.as_slice())?;

    let original = compare(&input, Kind::Loadouts, &fields, &english, &german)?;
    let reduced = compare(&projected, Kind::Loadouts, &fields, &english, &german)?;
    assert_eq!(
        reduced, original,
        "counts, repeated candidate order and German oracle verdicts are identical"
    );
    assert_eq!(
        reduced.row_count, 3,
        "one retained slot per authored loadout row"
    );
    assert!(
        reduced
            .failures
            .iter()
            .any(|failure| failure.contains("de texts contradict")),
        "the nested authored candidate still contradicts the German oracle"
    );

    let collect_keys = |document: &Value| -> TestResult<Vec<String>> {
        let loadouts = document.get("loadouts").ok_or("loadout array")?;
        let mut found = Vec::new();
        crate::real_archive_texts::keyed_texts(loadouts, &fields, &mut found);
        Ok(found.into_iter().map(|(key, _)| key.to_owned()).collect())
    };
    assert_eq!(
        collect_keys(&projected)?,
        collect_keys(&input)?,
        "the precise recursive candidate order and repetitions are retained"
    );
    assert_eq!(
        projected.pointer("/loadouts/1"),
        Some(&Value::Null),
        "a text-free Loadout still occupies its authored row position"
    );
    assert!(
        projected.pointer("/loadouts/0/ports/0/unrelated").is_none(),
        "unrelated loadout payload is discarded"
    );
    assert_eq!(
        projected.get("items"),
        input.get("items"),
        "the capture behavior of the other rows is unchanged"
    );
    assert!(
        catalog_paths()
            .read(br#"{"loadouts":{"ports":[]}}"#.as_slice())
            .is_err(),
        "a non-array loadout section cannot silently become an empty row set"
    );
    Ok(())
}
fn loadout_text_fixture() -> Value {
    json!({
        "texts": {"publicNames": [], "gameTexts": []},
        "items": [{"unchanged": "outside the loadout projection"}],
        "loadouts": [
            {
                "entityGuid": "fixture-loadout",
                "ports": [{
                    "name": {
                        "key": "@shared", "en": "Fixture", "de": "Beispiel",
                        "description": {"key": "@other", "en": "Other", "de": "Wrong"}
                    },
                    "ports": [{"name": {"key": "@shared", "en": "Fixture", "de": "Beispiel"}}],
                    "unrelated": {"numbers": [1, 2, 3]}
                }]
            },
            {"ports": [{"name": "raw field", "description": {"key": "@loc_placeholder"}}]},
            {"ports": [[{"name": {"key": "@missing", "en": null, "de": null}}]]}
        ]
    })
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
    let rows = text_rows(document, kind).map_err(|error| error.to_string())?;
    let index = TextIndex::new(document).map_err(|error| error.to_string())?;
    let (english, german) = kind_coverage(document, &index, kind, rows, fields, english, german)
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

#[test]
fn text_census_keeps_physical_entry_identities_without_retaining_the_use_inventory()
-> TestResult<()> {
    let source_path = r"Data\Libs\GameAudio\Fixture.xml";
    let normalized_path = "Data/Libs/GameAudio/Fixture.xml";
    let radar = entry_table("entry-radar", 7, source_path, normalized_path);
    let flare = entry_table("entry-flare", 11, source_path, normalized_path);
    let input = json!({
        "schemaVersion": SCHEMA_VERSION,
        "configurations": {"radarAudio": radar},
        "definitions": {"flareLibraries": flare},
        "provenance": {
            "sources": [
                {
                    "id": "entry-radar", "kind": "archiveEntry", "entryIndex": 7,
                    "path": sourced_path(source_path), "unrelated": {"detail": true}
                },
                {
                    "id": "entry-flare", "kind": "archiveEntryProof", "entryIndex": "11",
                    "details": "captureLimited"
                },
                {"id": "directory", "kind": "archiveDirectory", "admittedEntryCount": "15"}
            ],
            "uses": [{"id": "fixture"}]
        }
    });
    let paths = catalog_paths();
    let mut projected = paths.read(serde_json::to_vec(&input)?.as_slice())?;
    for kind in [Kind::RadarAudio, Kind::FlareLibraries] {
        assert_eq!(
            catalog_v3_consumer::rows(&projected, kind)?,
            catalog_v3_consumer::rows(&input, kind)?,
            "the same physical Entry identity is verified after filtering",
        );
    }
    assert!(
        projected.pointer("/provenance/uses").is_none(),
        "the large use inventory is outside the text census",
    );
    assert_eq!(
        projected["provenance"]["sources"],
        json!([
            {
                "id": "entry-radar", "kind": "archiveEntry", "entryIndex": 7,
                "path": sourced_path(source_path)
            },
            {"id": "entry-flare", "kind": "archiveEntryProof", "entryIndex": "11"}
        ]),
        "strip unrelated evidence while preserving both Entry identity forms",
    );
    projected["provenance"]["sources"][0]["entryIndex"] = json!(8);
    assert!(
        catalog_v3_consumer::rows(&projected, Kind::RadarAudio).is_err(),
        "the compact capture still detects a mismatched physical Entry index",
    );
    Ok(())
}

#[test]
fn text_census_checks_particle_path_and_physical_binding() -> TestResult<()> {
    let source_path = r"Data/Libs\Particles/Fixture.XML";
    let particles = entry_table(
        "entry-particle",
        7,
        source_path,
        r"data\libs\particles\fixture.xml",
    );
    let input = json!({
        "schemaVersion": SCHEMA_VERSION,
        "definitions": {"particleLibraries": particles},
        "provenance": {"sources": [{
            "id": "entry-particle", "kind": "archiveEntry", "entryIndex": 7,
            "path": sourced_path(source_path)
        }]}
    });
    let encoded = serde_json::to_vec(&input)?;
    let mut projected = catalog_paths().read(encoded.as_slice())?;
    assert_eq!(
        catalog_v3_consumer::rows(&projected, Kind::ParticleLibraries)?.len(),
        1,
        "the Particle Library retains a source-bound Archive Entry",
    );

    let path = projected
        .pointer_mut("/definitions/particleLibraries/rows/0/identity/normalizedPath/value")
        .ok_or("projected Particle Library path")?;
    *path = json!("data/libs/particles/fixture.xml");
    let failure = catalog_v3_consumer::rows(&projected, Kind::ParticleLibraries)
        .expect_err("the Particle Library key uses archive separators");
    assert!(
        failure
            .to_string()
            .contains("normalized Entry path differs"),
        "reject an incorrect normalized path after capture",
    );
    Ok(())
}

fn entry_table(id: &str, index: u32, path: &str, normalized: &str) -> Value {
    json!({
        "availability": "available",
        "rows": [{
            "identity": {
                "inputRef": id,
                "entryIndex": index,
                "sourcePath": sourced_path(path),
                "normalizedPath": sourced_path(normalized)
            }
        }]
    })
}

fn sourced_path(value: &str) -> Value {
    json!({
        "state": "present",
        "value": value,
        "containsEncryptedSource": false,
        "sourceSetComplete": true,
        "sourceUsesRefs": ["fixture"]
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
    let paths = catalog_paths();
    let mut encoded = serde_json::to_vec(&input)?;
    let projected = paths.read(encoded.as_slice())?;
    assert_eq!(projected["definitions"], input["definitions"]);
    assert_eq!(
        projected["configurations"]["contractTemplates"],
        input["configurations"]["contractTemplates"],
    );
    assert_eq!(projected["projections"], input["projections"]);
    assert!(projected["configurations"].get("irrelevant").is_none());
    assert!(
        projected.pointer("/provenance/uses").is_none(),
        "the text census omits use rows"
    );
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
    let paths = catalog_paths();
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
