// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

pub(crate) use super::source_fixture::{healthy_probe_database, probe_database, probe_inventory};
use serde_json::{Value, json};

use super::compare::Rule;
use super::inventory::FieldKey;
use super::path::{Path, Step};
use super::published_io::Dataset;
use super::published_model::{
    Availability, Build, Family, Field, File, METADATA, Provenance, Publication, Snapshot,
};
use super::units::Unit;

pub(crate) const GUID: &str = "11110000-0000-4000-8000-000000000001";
pub(crate) const OTHER: &str = "11110000-0000-4000-8000-000000000002";
pub(crate) const RESOURCE: &str = "11110000-0000-4000-8000-000000000003";
pub(crate) const REVISION: &str = "1111111111111111111111111111111111111111";
pub(crate) const PUBLICATION: Publication = Publication {
    revision: REVISION,
    game_version: "1.2.3",
    channel: "LIVE",
    change_number: "1234",
};

pub(crate) fn build() -> Build {
    Build {
        game_version: "1.2.3".to_owned(),
        channel: "LIVE".to_owned(),
        change_number: "1234".to_owned(),
    }
}

fn object<'a>(fields: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(name, value)| (name.to_owned(), value))
            .collect(),
    )
}

fn nested(path: &[&str], value: Value) -> Value {
    path.iter()
        .rev()
        .fold(value, |value, name| object([(*name, value)]))
}

pub(crate) fn row(guid: &str, value: Value) -> Value {
    object([
        ("Item", json!({ "reference": guid })),
        (
            "Raw",
            nested(&["Entity", "Components", "SyntheticParams", "value"], value),
        ),
    ])
}

pub(crate) fn catalog(rows: Vec<Value>) -> Value {
    object([("items", Value::Array(rows))])
}

pub(crate) fn item(guid: &str, value: Value) -> Value {
    object([
        ("guid", json!(guid)),
        ("stats", nested(&["probe", "value"], value)),
    ])
}

pub(crate) fn field() -> Field {
    Field {
        source: FieldKey::new("SyntheticParams", "value"),
        family: Family::Items,
        reference: Path::fields(&["Raw", "Entity", "Components", "SyntheticParams", "value"]),
        catalog: Path::fields(&["stats", "probe", "value"]),
        reference_unit: Unit::Dimensionless,
        catalog_unit: Unit::Dimensionless,
        rule: Rule::F32RoundTrip,
        resource_join: None,
    }
}

pub(crate) struct PublishedFixture {
    pub(crate) root: PathBuf,
    pub(crate) snapshot: Snapshot,
    _directory: tempfile::TempDir,
}

fn hex(bytes: &[u8]) -> super::Result<String> {
    use std::fmt::Write as _;
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}")?;
    }
    Ok(output)
}

impl PublishedFixture {
    pub(crate) fn new(rows: Vec<Value>) -> super::Result<Self> {
        let directory = tempfile::tempdir()?;
        let root = directory.path().to_path_buf();
        let document = json!({ "rows": Value::Array(rows) });
        let bytes = serde_json::to_vec(&document)?;
        let file = root.join("synthetic.json");
        std::fs::write(&file, &bytes)?;
        let fingerprint = p4kx_testkit::offline_seam::fingerprint(&file, 1 << 20, || false)?;
        let snapshot = Snapshot {
            schema_version: 1,
            provenance: Provenance {
                repository: "StarCitizenWiki/scunpacked-data".to_owned(),
                data_revision: REVISION.to_owned(),
                published_build: build(),
                generator_revision: Availability::Unknown {
                    reason: "not published".to_owned(),
                },
                dcb_sha256: Availability::Unknown {
                    reason: "not published".to_owned(),
                },
            },
            selection: Vec::new(),
            exclusions: Vec::new(),
            files: vec![File {
                path: "synthetic.json".to_owned(),
                bytes: fingerprint.bytes,
                sha256: hex(&fingerprint.sha256)?,
                family: Family::Items,
                rows: Path(vec![
                    Step::Field {
                        name: "rows".to_owned(),
                    },
                    Step::Each,
                ]),
                identity: Path::fields(&["Item", "reference"]),
            }],
            fields: vec![field()],
        };
        let fixture = Self {
            root,
            snapshot,
            _directory: directory,
        };
        fixture.save()?;
        Ok(fixture)
    }

    pub(crate) fn add_file(
        &mut self,
        name: &str,
        family: Family,
        rows: Vec<Value>,
    ) -> super::Result<()> {
        let bytes = serde_json::to_vec(&json!({ "rows": Value::Array(rows) }))?;
        let path = self.root.join(name);
        std::fs::write(&path, bytes)?;
        let fingerprint = p4kx_testkit::offline_seam::fingerprint(&path, 1 << 20, || false)?;
        let mut file = self
            .snapshot
            .files
            .first()
            .ok_or("file mapping absent")?
            .clone();
        name.clone_into(&mut file.path);
        file.family = family;
        file.bytes = fingerprint.bytes;
        file.sha256 = hex(&fingerprint.sha256)?;
        self.snapshot.files.push(file);
        Ok(())
    }

    pub(crate) fn save(&self) -> super::Result<()> {
        std::fs::write(
            self.root.join(METADATA),
            serde_json::to_vec(&self.snapshot)?,
        )?;
        Ok(())
    }

    pub(crate) fn load(&self) -> super::Result<Dataset> {
        Dataset::load(&self.root, &|| false)
    }
}
