// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_media::{Identity, Source};
use p4kx_testkit::{TestCodec, TestWem};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(crate) const CONTAINER: &str = "audio.socpak";

pub(crate) struct Origins {
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    pub(crate) fn read(base: &Path) -> TestResult<Self> {
        Ok(Self {
            fields: read_rows(base.join("field-origins.jsonl"))?,
            uses: read_rows(base.join("origin-uses.jsonl"))?,
            sources: read_rows(base.join("origin-sources.jsonl"))?,
        })
    }

    pub(crate) fn field(
        &self,
        stream: &str,
        ordinal: usize,
        path: &str,
        encrypted: bool,
    ) -> TestResult<&Value> {
        let ordinal = ordinal.to_string();
        let field = self
            .fields
            .iter()
            .find(|row| {
                row.get("type").and_then(Value::as_str) == Some("field")
                    && row.pointer("/scope/stream/type").and_then(Value::as_str) == Some(stream)
                    && (stream != "records"
                        || row.pointer("/scope/stream/kind").and_then(Value::as_str)
                            == Some("audio"))
                    && row.pointer("/scope/ordinal").and_then(Value::as_str)
                        == Some(ordinal.as_str())
                    && row.get("field").and_then(Value::as_str) == Some(path)
            })
            .ok_or_else(|| format!("immediate Audio origin {stream} {ordinal} {path}"))?;
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&Value::Bool(true)),
            "{path}: complete source uses"
        );
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&Value::Bool(encrypted)),
            "{path}: immediate marker"
        );
        Ok(field)
    }

    pub(crate) fn require_owner(&self, receipt: &Value, owner: Owner) -> TestResult<Vec<&Value>> {
        let refs = receipt
            .get("sourceUsesRefs")
            .and_then(Value::as_array)
            .ok_or("source-use refs")?;
        assert!(
            !refs.is_empty(),
            "actual WEM field has physical source uses"
        );
        let mut used = Vec::new();
        for reference in refs {
            let observation = self
                .uses
                .iter()
                .find(|row| row.get("id") == Some(reference))
                .ok_or("source-use row")?;
            let source = self.physical(observation.get("source").ok_or("source-use owner")?)?;
            assert_eq!(
                source.pointer("/path/value"),
                Some(&json!(owner.path)),
                "original leaf path"
            );
            assert_eq!(
                source.get("entryIndex"),
                Some(&json!(owner.entry)),
                "same-name physical Entry index"
            );
            assert_eq!(
                source.get("storedEncryption"),
                Some(&json!(owner.stored)),
                "stored leaf encryption"
            );
            self.check_parent(source, owner)?;
            used.push(observation);
        }
        Ok(used)
    }

    fn check_parent(&self, source: &Value, owner: Owner) -> TestResult {
        let ancestors = source
            .get("ancestors")
            .and_then(Value::as_array)
            .ok_or("physical ancestry")?;
        match (ancestors.as_slice(), owner.parent) {
            ([], None) => {}
            ([reference], Some(expected)) => {
                let parent = self.physical(reference)?;
                assert_eq!(
                    parent.get("entryIndex"),
                    Some(&json!(expected)),
                    "containing Entry index"
                );
                assert_eq!(
                    parent.pointer("/path/value"),
                    Some(&json!(CONTAINER)),
                    "original containing Entry path"
                );
                assert_eq!(
                    parent.get("storedEncryption"),
                    Some(&json!("encrypted")),
                    "actual containing Entry proof"
                );
            }
            _ => return Err("wrong physical ancestry".into()),
        }
        Ok(())
    }

    fn physical(&self, reference: &Value) -> TestResult<&Value> {
        self.sources
            .iter()
            .find(|row| row.get("id") == Some(reference))
            .ok_or_else(|| "physical source row".into())
    }
}

pub(crate) fn read_rows<T: DeserializeOwned>(path: PathBuf) -> TestResult<Vec<T>> {
    fs::read_to_string(path)?
        .lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

#[derive(Clone, Copy)]
pub(crate) struct Owner {
    pub(crate) path: &'static str,
    pub(crate) entry: usize,
    pub(crate) parent: Option<usize>,
    pub(crate) stored: &'static str,
    pub(crate) metadata: bool,
    pub(crate) payload: bool,
}

pub(crate) fn check_identity(source: &Source, owner: Owner) -> TestResult {
    let Identity::Entry { containers, entry } = &source.identity else {
        return Err("physical WEM Entry".into());
    };
    assert_eq!(
        (entry.path.as_str(), entry.entry_index),
        (owner.path, owner.entry),
        "selected leaf identity"
    );
    assert_eq!(
        containers.len(),
        usize::from(owner.parent.is_some()),
        "ordered physical ancestor count"
    );
    if let Some(expected) = owner.parent {
        let parent = containers.first().ok_or("WEM containing Entry")?;
        assert_eq!(
            (parent.path.as_str(), parent.entry_index),
            (CONTAINER, expected),
            "exact unselected containing Entry"
        );
    }
    Ok(())
}

pub(crate) fn wem_bytes() -> Vec<u8> {
    let stereo_format = [1, 0, 2, 0, 0x80, 0xBB, 0, 0, 0, 0xEE, 2, 0, 4, 0, 16, 0];
    TestWem::pcm(TestCodec::Pcm, 1, 22_050, &[0, 1, 2, 3])
        .chunk(*b"fmt ", stereo_format)
        .tail_chunk(*b"fmt ", [0; 16])
        .tail_chunk(*b"data", [0; 32])
        .bytes()
}
