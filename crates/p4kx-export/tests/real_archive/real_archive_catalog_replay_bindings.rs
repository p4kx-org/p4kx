// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::Path;

use p4kx_export::{Kind, SCHEMA_VERSION};
use p4kx_extraction::{Archive, utc_rfc3339};
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::TestResult;
use super::inventory::Inventory;

#[path = "real_archive_catalog_replay_health.rs"]
mod health;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Receipt {
    channel: String,
    archive_before: ArchiveStamp,
    archive_after: ArchiveStamp,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ArchiveStamp {
    manifest_sha256: String,
    build: MeasuredBuild,
    archive_bytes: u64,
    archive_created_utc: String,
    archive_modified_utc: String,
}

#[derive(Deserialize, PartialEq, Eq)]
struct MeasuredBuild {
    channel: String,
    #[serde(flatten)]
    build: Build,
}

#[derive(Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Build {
    branch: String,
    version: String,
    change_number: String,
    #[serde(rename = "buildId")]
    id: String,
}

pub(crate) struct Binding {
    channel: String,
    archive: ArchiveStamp,
}

impl Receipt {
    pub(crate) fn read(directory: &Path) -> TestResult<Self> {
        let input = BufReader::new(File::open(directory.join("receipt.private.json"))?);
        Ok(serde_json::from_reader(input)?)
    }

    pub(crate) fn binding(self) -> TestResult<Binding> {
        let archive = self.archive_before;
        if archive != self.archive_after || archive.build.channel != self.channel {
            return Err("measurement archive stamps or channels disagree".into());
        }
        if archive.manifest_sha256.len() != 64
            || !archive
                .manifest_sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid manifest digest in measurement stamp".into());
        }
        Ok(Binding {
            channel: self.channel,
            archive,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct IndexedPass {
    schema_version: u32,
    source: CatalogSource,
    #[serde(deserialize_with = "read_kind_selection")]
    completeness: BTreeMap<String, Selection>,
    export_health: Publication,
    #[serde(rename = "provenance")]
    pub(super) inventory: Inventory,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogMetadata {
    schema_version: u32,
    source: CatalogSource,
    completeness: BTreeMap<String, Selection>,
    export_health: Publication,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogSource {
    channel: Option<String>,
    build: Option<Build>,
    archive_size: u64,
    archive_modified: Option<String>,
}

#[derive(Deserialize)]
struct Selection {
    status: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Publication {
    #[serde(rename = "publicationMode")]
    mode: String,
    status: String,
    read: String,
    projection: String,
    kinds: Option<Vec<health::KindReport>>,
    selected_kind_records: Option<u64>,
    written_rows: Option<u64>,
}

impl Publication {
    fn check_accounting(&self) -> TestResult {
        health::check(
            self.kinds.as_deref().ok_or("missing catalog kind health")?,
            self.selected_kind_records
                .ok_or("missing selectedKindRecords total")?,
            self.written_rows.ok_or("missing writtenRows total")?,
        )
    }
}

impl IndexedPass {
    pub(super) fn read(input: impl Read) -> serde_json::Result<Self> {
        serde_json::from_reader(input)
    }
}

impl Binding {
    pub(super) fn check(&self, pass: &IndexedPass) -> TestResult {
        self.check_catalog_parts(
            pass.schema_version,
            &pass.source,
            &pass.completeness,
            &pass.export_health,
        )?;
        pass.export_health.check_accounting()
    }

    pub(crate) fn check_catalog(&self, selected: &Value) -> TestResult {
        let metadata = CatalogMetadata::deserialize(selected)?;
        self.check_catalog_parts(
            metadata.schema_version,
            &metadata.source,
            &metadata.completeness,
            &metadata.export_health,
        )
    }

    pub(crate) fn check_source_value(&self, value: &Value) -> TestResult {
        self.check_source(&CatalogSource::deserialize(value)?)
    }

    pub(crate) fn check_archive(&self, archive: &Archive) -> TestResult {
        let build = archive.build().ok_or("missing installed Build identity")?;
        let expected = &self.archive.build.build;
        let modified = utc_rfc3339(
            archive
                .file_modified()
                .ok_or("missing installed Archive modification time")?,
        );
        if p4kx_channels::channel_name(archive.path()).as_deref() != Some(self.channel.as_str())
            || archive.file_len() != self.archive.archive_bytes
            || modified != stamp_seconds(&self.archive.archive_modified_utc)?
            || build.branch() != expected.branch
            || build.version() != expected.version
            || build.change_number() != expected.change_number
            || build.build_id() != expected.id
        {
            return Err("the installed Archive identity differs from the measurement".into());
        }
        let manifest = archive.path().with_file_name("build_manifest.id");
        let bytes = fs::read(manifest).map_err(|_| "read the installed Build manifest")?;
        let mut actual = String::with_capacity(64);
        for byte in Sha256::digest(bytes) {
            write!(actual, "{byte:02x}")?;
        }
        if !actual.eq_ignore_ascii_case(&self.archive.manifest_sha256) {
            return Err("the installed Build manifest digest differs from the measurement".into());
        }
        Ok(())
    }

    fn check_catalog_parts(
        &self,
        schema_version: u32,
        source: &CatalogSource,
        completeness: &BTreeMap<String, Selection>,
        publication: &Publication,
    ) -> TestResult {
        if schema_version != SCHEMA_VERSION {
            return Err("replay requires catalog schema v3".into());
        }
        self.check_source(source)?;
        if publication.mode != "normal"
            || [
                &publication.status,
                &publication.read,
                &publication.projection,
            ]
            .iter()
            .any(|status| status.as_str() != "complete")
        {
            return Err("replay requires a complete normal publication".into());
        }
        check_complete_selection(completeness)
    }

    fn check_source(&self, source: &CatalogSource) -> TestResult {
        if source.channel.as_deref() != Some(self.channel.as_str())
            || source.build.as_ref() != Some(&self.archive.build.build)
            || source.archive_size != self.archive.archive_bytes
            || source.archive_modified.as_deref()
                != Some(stamp_seconds(&self.archive.archive_modified_utc)?.as_str())
        {
            return Err("catalog source identity differs from the measurement".into());
        }
        Ok(())
    }
}

pub(crate) fn check_catalog_selection(value: &Value) -> TestResult {
    check_complete_selection(&BTreeMap::deserialize(value)?)
}

pub(crate) fn catalog_selection_names() -> impl Iterator<Item = &'static str> {
    Kind::ALL
        .into_iter()
        .map(Kind::name)
        .chain(["shipStoreSlugs"])
}

#[test]
fn selection_registry_agrees_with_catalog_contract() -> TestResult {
    use std::collections::BTreeSet;

    let schema = p4kx_export::catalog_schema();
    let fields = schema
        .pointer("/$defs/Completeness/properties")
        .and_then(Value::as_object)
        .ok_or("Catalog completeness contract")?;
    let expected = fields.keys().map(String::as_str).collect::<BTreeSet<_>>();
    assert_eq!(catalog_selection_names().collect::<BTreeSet<_>>(), expected);
    Ok(())
}
fn check_complete_selection(completeness: &BTreeMap<String, Selection>) -> TestResult {
    let expected = catalog_selection_names().collect::<Vec<_>>();
    let missing: Vec<_> = expected
        .iter()
        .copied()
        .filter(|name| !completeness.contains_key(*name))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "replay requires a complete unfiltered selection (missing: {})",
            missing.join(", "),
        )
        .into());
    }
    if completeness.len() != expected.len() {
        return Err("replay found an unexpected Catalog Kind selection".into());
    }
    if completeness
        .values()
        .any(|selection| selection.status != "complete")
    {
        return Err("replay requires a complete unfiltered selection (incomplete coverage)".into());
    }
    Ok(())
}

fn read_kind_selection<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Selection>, D::Error> {
    deserializer.deserialize_map(UniqueKindSelection)
}

struct UniqueKindSelection;

impl<'de> Visitor<'de> for UniqueKindSelection {
    type Value = BTreeMap<String, Selection>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a map of unique Catalog Kind selections")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut input: A) -> Result<Self::Value, A::Error> {
        let mut selection = BTreeMap::new();
        while let Some((name, value)) = input.next_entry()? {
            if selection.insert(name, value).is_some() {
                return Err(de::Error::custom("duplicate Catalog Kind selection"));
            }
        }
        Ok(selection)
    }
}

fn stamp_seconds(stamp: &str) -> TestResult<String> {
    let utc = stamp
        .strip_suffix('Z')
        .ok_or("archive modification UTC suffix")?;
    let (seconds, fractional) = utc.split_once('.').unwrap_or((utc, "0"));
    if seconds.len() != 19
        || fractional.is_empty()
        || !fractional.bytes().all(|digit| digit.is_ascii_digit())
    {
        return Err("archive modification stamp is not a UTC timestamp".into());
    }
    Ok(format!("{seconds}Z"))
}
