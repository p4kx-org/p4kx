// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{Fixture, Input, TestResult};
use p4kx_extraction::{Archive, Entry};
use p4kx_testkit::{DataForgeBuilder, TestContainer, TestEntry, TestRecord, TestStruct};
use serde_json::{Value, json};

const STORED_INNER: &str = "entdata/subject.entxml";
pub(crate) const INNER: &str = "entdata\\subject.entxml";
pub(crate) fn outer(system: &str) -> String {
    format!("Data\\ObjectContainers\\PU\\system\\{system}\\{system}system.socpak")
}
pub(crate) fn container(system: &str, entity: &[u8], encrypted: bool) -> TestEntry {
    let bytes = TestContainer::new()
        .entry(TestEntry::stored(
            format!("{system}system.xml"),
            b"<ObjectContainer/>".as_slice(),
        ))
        .entry(TestEntry::stored(STORED_INNER, entity))
        .build();
    let entry = TestEntry::stored(outer(system), bytes);
    if encrypted { entry.encrypted() } else { entry }
}
pub(crate) fn read(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    use std::io::Read as _;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}
pub(crate) fn original<'a>(archive: &'a Archive, path: &str) -> TestResult<&'a Entry> {
    archive
        .entries()
        .iter()
        .find(|entry| entry.path() == path)
        .ok_or_else(|| format!("original Entry {path}").into())
}
pub(crate) fn failure<'a>(
    catalog: &'a Value,
    kind: &str,
    operation: &str,
) -> TestResult<Vec<&'a Value>> {
    Ok(check::rows(catalog, "/exportHealth/failedEntries")?
        .iter()
        .filter(|row| {
            row.get("kind") == Some(&json!(kind)) && row.get("operation") == Some(&json!(operation))
        })
        .collect())
}
pub(crate) fn metadata_paths(catalog: &Value, carrier: &Value) -> TestResult<Vec<String>> {
    let mut paths = Vec::new();
    for used in check::uses(catalog, carrier)? {
        if used.get("role") == Some(&json!("attempted"))
            || used.get("role") == Some(&json!("reference"))
        {
            continue;
        }
        assert_eq!(
            used.get("scope"),
            Some(&json!("archiveDirectoryMetadata")),
            "failed payload bytes do not contribute to copied directory paths"
        );
        assert_eq!(
            check::at(used, "/location/type")?,
            &json!("metadata"),
            "actual directory metadata has its own field coordinate"
        );
        assert_eq!(
            check::at(used, "/location/field")?,
            &json!("path"),
            "unrelated directory fields do not become path contributors"
        );
        let source = check::source(catalog, used)?;
        assert_eq!(
            source.get("kind"),
            Some(&json!("archiveEntry")),
            "actual Entry facts retain original Entry owners"
        );
        paths.push(
            check::at(source, "/path/value")?
                .as_str()
                .ok_or("original Entry source path")?
                .to_owned(),
        );
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

pub(crate) fn with_starmap(entries: Vec<TestEntry>) -> TestResult<Fixture> {
    const RECORD_PATH: &str = "libs/foundry/records/starmap/pu/system/alpha/health";
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("StarMapObject"))
        .record(TestRecord::new(
            0x5712,
            "StarMapObject",
            "StarMapObject.Health",
            format!("{RECORD_PATH}.xml"),
        ))
        .build();
    let source = Fixture::open(&bytes, Input::Plain, entries)?;
    {
        let forge = source.entry.open()?;
        assert_eq!(
            forge.records().len(),
            1,
            "one original placement trigger exists"
        );
        let record = forge.record(0).ok_or("original placement trigger Record")?;
        assert_eq!(
            record.type_name()?,
            "StarMapObject",
            "the actual source type is selected"
        );
        assert_eq!(
            record.record_path()?,
            RECORD_PATH,
            "the actual source path admits placements"
        );
        assert!(
            record.tree().is_ok(),
            "the original placement trigger has a readable body"
        );
    }
    Ok(source)
}
