// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_testkit::TestEntry;
use serde_json::{Value, json};

use super::{TestResult, database, run, write_archive};

const ENTRY: &str = r"Data\Game2.dcb";
const FILE: &str = "standalone.dcb";
const HEADER_FIELDS: [(&str, &str, usize); 4] = [
    ("version", "version", 4),
    ("record_count", "recordCount", 32),
    ("struct_count", "structCount", 16),
    ("enum_count", "enumCount", 24),
];

#[derive(Clone, Copy, Debug)]
enum Input {
    Plain,
    Encrypted,
    File,
}

impl Input {
    fn write(self, directory: &Path, bytes: &[u8]) -> Result<PathBuf, Box<dyn std::error::Error>> {
        if matches!(self, Self::File) {
            let path = directory.join(FILE);
            fs::write(&path, bytes)?;
            return Ok(path);
        }
        let entry = TestEntry::zstd(ENTRY, bytes.to_vec());
        let entry = if matches!(self, Self::Encrypted) {
            entry.encrypted()
        } else {
            entry
        };
        write_archive(
            directory,
            vec![TestEntry::stored(r"Data\Game.dcb", Vec::new()), entry],
        )
    }

    const fn marker(self) -> Option<bool> {
        match self {
            Self::Plain => Some(false),
            Self::Encrypted => Some(true),
            Self::File => None,
        }
    }

    const fn name_scope(self) -> (&'static str, &'static str, Option<bool>) {
        match self {
            Self::Plain | Self::Encrypted => (ENTRY, "archiveDirectoryMetadata", Some(false)),
            Self::File => (FILE, "fileMetadata", None),
        }
    }
}

#[test]
fn dcb_info_distinguishes_original_header_and_entry_name_sources() -> TestResult {
    for (version, input) in [(6, Input::Plain), (8, Input::Encrypted), (8, Input::File)] {
        let directory = tempfile::tempdir()?;
        let bytes = database().version(version).build();
        let path = input.write(directory.path(), &bytes)?;
        let (output, _) = run(&path, true, 0)?;
        let document: Value = serde_json::from_str(&output)?;
        for (field, name, offset) in HEADER_FIELDS {
            let word = bytes
                .get(offset..offset + 4)
                .ok_or("original header word")?;
            assert_eq!(
                document.get(field),
                Some(&json!(u32::from_le_bytes(word.try_into()?))),
                "{input:?} retains the actual header word for {field}"
            );
            let (observed, _) = source_use(&document, field, input.marker())?;
            assert_eq!(
                observed.get("scope"),
                Some(&json!("payload")),
                "{field} is a payload word"
            );
            assert_eq!(
                observed.get("location"),
                Some(
                    &json!({"type": "subresource", "namespace": "dataforgeHeader", "indices": [], "field": name})
                ),
                "{field} retains its distinct original header coordinate"
            );
        }
        assert_name_source(&document, input)?;
        let path_origin = origin(&document, "archive", Some(false))?;
        assert_eq!(
            path_origin.get("sourceUsesRefs"),
            Some(&json!([])),
            "the caller path is command metadata"
        );
    }
    Ok(())
}

fn assert_name_source(document: &Value, input: Input) -> TestResult {
    let (name, scope, marker) = input.name_scope();
    assert_eq!(
        document.get("entry"),
        Some(&json!(name)),
        "the selected input name"
    );
    let (observed, physical) = source_use(document, "entry", marker)?;
    assert_eq!(
        observed.get("scope"),
        Some(&json!(scope)),
        "the name uses its metadata domain"
    );
    assert_eq!(
        observed.get("location"),
        Some(&json!({"type": "metadata", "field": "path"})),
        "actual input path metadata"
    );
    let kind = if matches!(input, Input::File) {
        "file"
    } else {
        "archiveEntry"
    };
    assert_eq!(
        physical.get("kind"),
        Some(&json!(kind)),
        "the actual physical input kind"
    );
    let (payload, _) = source_use(document, "version", input.marker())?;
    assert_eq!(
        payload.get("source"),
        observed.get("source"),
        "payload and name share the actual input identity"
    );
    Ok(())
}

fn origin<'a>(
    document: &'a Value,
    field: &str,
    marker: Option<bool>,
) -> Result<&'a Value, Box<dyn std::error::Error>> {
    let cell = document
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/{field}")))
        .ok_or_else(|| format!("immediate DataForge info origin for {field}"))?;
    assert_eq!(
        cell.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "{field} retains its source-domain marker"
    );
    assert_eq!(
        cell.get("sourceSetComplete"),
        Some(&json!(true)),
        "{field} has complete source evidence"
    );
    Ok(cell)
}

fn source_use<'a>(
    document: &'a Value,
    field: &str,
    marker: Option<bool>,
) -> Result<(&'a Value, &'a Value), Box<dyn std::error::Error>> {
    let references = origin(document, field, marker)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("immediate contributor references")?;
    let [id] = references.as_slice() else {
        return Err("expected one exact input".into());
    };
    let observed = document
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .and_then(|uses| uses.iter().find(|used| used.get("id") == Some(id)))
        .ok_or("physical use in the original inventory")?;
    assert_eq!(
        observed.get("role"),
        Some(&json!("payload")),
        "{field} retains the actual datum"
    );
    let physical = document
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .find(|source| source.get("id") == observed.get("source"))
        })
        .ok_or("physical input for the original use")?;
    Ok((observed, physical))
}
