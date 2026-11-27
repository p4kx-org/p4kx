// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{
    Archive, DataForge, DataForgeEntry, DataForgeOrigin, Field, RecordTree, SourceEncryption, Value,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value as Json, json};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Debug)]
pub(super) enum Input {
    Plain,
    Encrypted,
    Standalone,
}

impl Input {
    pub(super) const ALL: [Self; 3] = [Self::Plain, Self::Encrypted, Self::Standalone];

    pub(super) const fn marker(self) -> Option<bool> {
        match self {
            Self::Plain => Some(false),
            Self::Encrypted => Some(true),
            Self::Standalone => None,
        }
    }

    const fn encryption(self) -> (SourceEncryption, &'static str) {
        match self {
            Self::Plain => (SourceEncryption::Unencrypted, "unencrypted"),
            Self::Encrypted => (SourceEncryption::Encrypted, "encrypted"),
            Self::Standalone => (SourceEncryption::Unknown, "unknown"),
        }
    }

    const fn kind(self) -> &'static str {
        match self {
            Self::Plain | Self::Encrypted => "archiveEntry",
            Self::Standalone => "file",
        }
    }

    const fn filename(self) -> &'static str {
        match self {
            Self::Plain => "plain.p4k",
            Self::Encrypted => "encrypted.p4k",
            Self::Standalone => "standalone.dcb",
        }
    }
}

pub(super) struct Fixture<T> {
    _directory: tempfile::TempDir,
    pub(super) inputs: Vec<Case<T>>,
}

pub(super) struct Case<T> {
    pub(super) input: Input,
    path: PathBuf,
    pub(super) original: T,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Site {
    record: u32,
    ordinal: u64,
}

impl<T> Fixture<T> {
    pub(super) fn open(
        bytes: &[u8],
        modes: &[Input],
        read: impl Fn(&DataForge<'_>) -> TestResult<T>,
    ) -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let mut inputs = Vec::new();
        for &input in modes {
            let path = directory.path().join(input.filename());
            let entry = source_entry(&path, input, bytes)?;
            assert_eq!(
                entry.origin().encryption(),
                input.encryption().0,
                "{input:?} is verified at the original byte read"
            );
            let original = read(&entry.open()?)?;
            inputs.push(Case {
                input,
                path,
                original,
            });
        }
        Ok(Self {
            _directory: directory,
            inputs,
        })
    }
}

fn source_entry(path: &Path, input: Input, bytes: &[u8]) -> TestResult<DataForgeEntry> {
    if matches!(input, Input::Standalone) {
        fs::write(path, bytes)?;
        let entry = DataForgeEntry::read_file(path)?;
        assert!(
            matches!(entry.origin(), DataForgeOrigin::File(_)),
            "standalone bytes have their own File origin"
        );
        return Ok(entry);
    }
    let entry = TestEntry::zstd("Data\\Game2.dcb", bytes);
    let entry = if matches!(input, Input::Encrypted) {
        entry.encrypted()
    } else {
        entry
    };
    ArchiveBuilder::new().entry(entry).write_to(path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    assert!(
        entry.origin().matches_archive(&archive),
        "the DCB belongs to the exact opened Archive"
    );
    Ok(entry)
}

pub(super) fn site<'source>(
    tree: &RecordTree<'source>,
    selected: &Value<'source>,
) -> TestResult<Site> {
    let mut pending: Vec<_> = tree.fields().iter().rev().map(Field::value).collect();
    let mut ordinal = 0_u64;
    while let Some(value) = pending.pop() {
        if std::ptr::eq(value, selected) {
            return Ok(Site {
                record: tree.record_index(),
                ordinal,
            });
        }
        ordinal += 1;
        match value {
            Value::Struct(source) => {
                pending.extend(source.fields().iter().rev().map(Field::value));
            }
            Value::Array(values) => pending.extend(values.iter().rev()),
            _ => {}
        }
    }
    Err("the selected borrow belongs to the original tree".into())
}

impl<T> Case<T> {
    pub(super) fn run(&self, args: &[&str]) -> TestResult<Json> {
        let (document, stderr) = self.run_with_warnings(args)?;
        assert!(
            stderr.is_empty(),
            "the controlled read has no warnings: {stderr}"
        );
        Ok(document)
    }

    pub(super) fn run_with_warnings(&self, args: &[&str]) -> TestResult<(Json, String)> {
        let output = p4kx_testkit::p4kx()?
            .args(["dcb", "refs"])
            .arg(&self.path)
            .args(args)
            .output()?;
        assert_eq!(
            output.status.code(),
            Some(0),
            "the existing command succeeds: {output:?}"
        );
        let document = serde_json::from_slice(&output.stdout)?;
        Ok((document, String::from_utf8(output.stderr)?))
    }

    pub(super) fn uses<'document>(
        &self,
        document: &'document Json,
        row: &Json,
        field: &str,
    ) -> TestResult<Vec<&'document Json>> {
        let origin = field_origin(row, field)?;
        assert_eq!(
            origin.get("containsEncryptedSource"),
            Some(&json!(self.input.marker())),
            "{:?} {field}",
            self.input
        );
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(true)),
            "the controlled {field} has its complete input set"
        );
        let ids = array(origin, "sourceUsesRefs")?;
        assert!(
            !ids.is_empty(),
            "the original evidence for {field} is observable"
        );
        let inventory = document.get("provenance").ok_or("physical inventory")?;
        let observations = array(inventory, "uses")?;
        let sources = array(inventory, "sources")?;
        let (_, encryption) = self.input.encryption();
        ids.iter()
            .map(|id| {
                let used = unique(observations, id)?;
                let physical = unique(sources, used.get("source").ok_or("use source ref")?)?;
                assert_eq!(used.get("scope"), Some(&json!("payload")), "{field}");
                assert_eq!(
                    used.get("effectiveEncryption"),
                    Some(&json!(encryption)),
                    "the actual use's effective marker"
                );
                assert_eq!(
                    physical.get("storedEncryption"),
                    Some(&json!(encryption)),
                    "the actual input's stored marker"
                );
                assert_eq!(
                    physical.get("kind"),
                    Some(&json!(self.input.kind())),
                    "the actual input's physical kind"
                );
                Ok(used)
            })
            .collect()
    }
}

pub(super) fn array<'a>(object: &'a Json, name: &str) -> TestResult<&'a [Json]> {
    object
        .get(name)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("actual array {name}").into())
}

pub(super) fn field_origin<'a>(row: &'a Json, field: &str) -> TestResult<&'a Json> {
    row.get("fieldOrigins")
        .and_then(|origins| origins.get(field))
        .ok_or_else(|| format!("immediate Reference field origin {field}").into())
}

fn unique<'a>(rows: &'a [Json], id: &Json) -> TestResult<&'a Json> {
    let mut matching = rows.iter().filter(|row| row.get("id") == Some(id));
    let found = matching.next().ok_or("resolved actual origin reference")?;
    assert!(
        matching.next().is_none(),
        "each origin reference resolves once"
    );
    Ok(found)
}

pub(super) fn uses_part(
    uses: &[&Json],
    namespace: &str,
    indices: &Json,
    field: &str,
    role: &str,
) -> bool {
    let location = json!({
        "type": "subresource",
        "namespace": namespace,
        "indices": indices,
        "field": field
    });
    uses.iter().any(|used| {
        used.get("location") == Some(&location)
            && used.get("role").and_then(Json::as_str) == Some(role)
    })
}

pub(super) fn has_value(uses: &[&Json], site: Site, part: &str, role: &str) -> bool {
    let indices = json!([site.record.to_string(), site.ordinal.to_string()]);
    uses_part(uses, "dataforgeValue", &indices, part, role)
}

pub(super) fn value(uses: &[&Json], site: Site, part: &str, role: &str) {
    assert!(
        has_value(uses, site, part, role),
        "the original {role} occurrence {site:?} {part} contributes: {uses:?}"
    );
}

pub(super) fn route(uses: &[&Json], site: Site, part: &str, array_step: Option<usize>) {
    let mut indices = vec![site.record.to_string(), site.ordinal.to_string()];
    indices.extend(array_step.map(|step| step.to_string()));
    let indices = json!(indices);
    assert!(
        uses_part(
            uses,
            "dataforgeReferenceRoute",
            &indices,
            part,
            "conversion"
        ),
        "the actual route {indices} {part} contributes: {uses:?}"
    );
}

pub(super) fn has_header(uses: &[&Json], index: u32, field: &str, role: &str) -> bool {
    let indices = json!([index.to_string()]);
    uses_part(uses, "dataforgeRecordHeader", &indices, field, role)
}

pub(super) fn header(uses: &[&Json], index: u32, field: &str, role: &str) {
    assert!(
        has_header(uses, index, field, role),
        "the original {role} header {index} {field} contributes: {uses:?}"
    );
}
