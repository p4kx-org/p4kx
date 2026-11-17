// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, Checksum};
use p4kx_media::{Identity, MediaRecord, OriginProfile, ShaderPayload, ShaderProvider};
use p4kx_shader::Limits;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry, test_key};
use serde_json::{Value, json};
use sha2::Digest as _;

use crate::material_origin_support as origin_rows;

use super::support;

type Check<T = ()> = Result<T, Box<dyn Error>>;

const CACHE: &str = r"Data\Shared.CFIB";
const CONTAINER: &str = r"Data\Encrypted.pak";
const BYTES: &[u8] = b"opaque synthetic cache";

#[test]
fn shader_cache_inventory_uses_its_actual_directory_and_keeps_raw_payload_origins() -> Check {
    let folder = tempfile::tempdir()?;
    let archive = cache_archive(folder.path())?;
    let output = folder.path().join("media");
    let limits = Limits {
        source_bytes: 1,
        ..Limits::default()
    };
    let registry = support::registry(ShaderProvider::new_with_limits(limits))?;
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &registry,
    )?)?;
    let records = support::records(&output, &index)?;
    assert_eq!(
        records.len(),
        2,
        "both physical cache occurrences are inventoried"
    );
    let origins = Origins::open(output.join(&index.artifact_base), &index)?;
    let mut owning_directories = BTreeSet::new();
    for (ordinal, record) in records.iter().enumerate() {
        owning_directories.insert(verify_record(&origins, ordinal, record)?);
    }
    assert_eq!(
        owning_directories,
        BTreeSet::from([None, Some(1)]),
        "root and nested cache each appear"
    );
    let summary = index.kinds.first().ok_or("shader summary")?;
    assert_eq!(
        summary.counts.source_entries.written, 2,
        "inventory count is unchanged"
    );
    Ok(())
}

fn verify_record(origins: &Origins, ordinal: usize, record: &MediaRecord) -> Check<Option<usize>> {
    let Identity::Entry {
        entry, containers, ..
    } = &record.source.identity
    else {
        return Err("cache Entry expected".into());
    };
    assert_eq!(
        entry.path, CACHE,
        "same display path in both owning directories"
    );
    let ancestor = match containers.as_slice() {
        [] => None,
        [container] => {
            assert_eq!(container.path, CONTAINER, "exact containing Entry");
            Some(container.entry_index)
        }
        _ => return Err("only one containing Entry expected".into()),
    };
    let ShaderPayload::CompiledCache(cache) = serde_json::from_value(record.data.clone())? else {
        return Err("compiled cache inventory".into());
    };
    assert_eq!(
        (cache.compiled_caches, cache.definitions, cache.properties),
        (1, 0, 0),
        "counts distinguish inventory from decoding"
    );
    origins.verify(ordinal, entry.entry_index, ancestor)?;

    let raw = record.source.raw.as_ref().ok_or("original cache bytes")?;
    assert_eq!(
        fs::read(origins.base.join(&raw.path))?,
        BYTES,
        "original bytes still retained"
    );
    assert_eq!(
        raw.digest.hex,
        Checksum::Sha256(sha2::Sha256::digest(BYTES).into()).hex(),
        "raw digest still binds the emitted bytes"
    );
    Ok(ancestor)
}

#[test]
fn the_cache_origin_profile_is_in_the_machine_readable_vocabulary() -> Check {
    let schema = serde_json::to_value(schemars::schema_for!(OriginProfile))?;
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(&json!("shaderCacheInventory")),
        "the cache inventory profile is in the generated vocabulary"
    );
    assert!(
        validator.is_valid(&json!("unmigrated")),
        "historical profiles remain readable"
    );
    assert!(
        !validator.is_valid(&json!("inventedProfile")),
        "the profile vocabulary stays closed"
    );
    Ok(())
}

fn cache_archive(folder: &Path) -> Check<Archive> {
    let path = folder.join("Data.p4k");
    let inside = TestContainer::new().entry(TestEntry::stored(CACHE, BYTES));
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(CACHE, BYTES).encrypted())
        .entry(TestEntry::zstd(CONTAINER, inside.build()).encrypted())
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

struct Origins {
    base: PathBuf,
    fields: Vec<Value>,
    uses: Vec<Value>,
    sources: Vec<Value>,
}

impl Origins {
    fn open(base: PathBuf, index: &p4kx_media::Index) -> Check<Self> {
        let receipts = index
            .field_origins
            .as_ref()
            .ok_or("current origin receipts")?;
        Ok(Self {
            fields: origin_rows::rows(base.join(&receipts.fields.path))?,
            uses: origin_rows::rows(base.join(&receipts.uses.path))?,
            sources: origin_rows::rows(base.join(&receipts.sources.path))?,
            base,
        })
    }

    fn verify(&self, ordinal: usize, entry: usize, ancestor: Option<usize>) -> Check {
        let nested = ancestor.is_some();
        let ordinal = ordinal.to_string();
        let occurrence = |row: &&Value| {
            row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("records")
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("shader")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
        };
        let field = |path| {
            self.fields
                .iter()
                .filter(occurrence)
                .find(|row| row["type"] == "field" && row["field"] == path)
                .ok_or("immediate cache field")
        };
        let family = field("/data/payload/family")?;
        let count = field("/data/payload/compiled_caches")?;
        let family_use = self.single_use(family, nested)?;
        let count_use = self.single_use(count, nested)?;
        assert_eq!(family_use["role"], "selector", "extension selects family");
        assert_eq!(
            count_use["role"], "conversion",
            "matched entry contributes one cache"
        );
        for evidence in [family_use, count_use] {
            assert_eq!(evidence["scope"], "archiveDirectoryMetadata", "{evidence}");
            assert_eq!(
                evidence.pointer("/location/field").and_then(Value::as_str),
                Some("path"),
                "{evidence}",
            );
        }
        assert_eq!(
            family_use["source"], count_use["source"],
            "same physical entry"
        );
        self.verify_owner(&family_use["source"], entry, ancestor)?;

        for path in [
            "/data/payload/semantic_decode",
            "/data/payload/definitions",
            "/data/payload/properties",
        ] {
            let value = field(path)?;
            assert_eq!(value["containsEncryptedSource"], false, "{path}");
            assert_eq!(value["sourceSetComplete"], true, "{path}");
            assert!(origin_rows::references(value)?.is_empty(), "{path}");
        }
        let raw = self.single_use(field("/source/raw/digest/hex")?, true)?;
        assert_eq!(raw["scope"], "payload", "raw digest consumes decoded bytes");
        assert_eq!(
            raw["source"], family_use["source"],
            "one original physical leaf"
        );
        assert!(
            self.fields
                .iter()
                .filter(occurrence)
                .any(|row| row["type"] == "occurrence" && row["profile"] == "shaderCacheInventory"),
            "the producer selects the cache inventory profile"
        );
        Ok(())
    }

    fn single_use(&self, field: &Value, encrypted: bool) -> Check<&Value> {
        assert_eq!(
            field.get("containsEncryptedSource"),
            Some(&json!(encrypted)),
            "{field}"
        );
        assert_eq!(
            field.get("sourceSetComplete"),
            Some(&json!(true)),
            "{field}"
        );
        let [id] = origin_rows::references(field)?.as_slice() else {
            return Err("one actual contributing use".into());
        };
        Self::find(&self.uses, id)
    }

    fn verify_owner(&self, id: &Value, entry: usize, ancestor: Option<usize>) -> Check {
        let owner = Self::find(&self.sources, id)?;
        assert_eq!(owner["entryIndex"], entry, "{owner}");
        let containers = owner["ancestors"].as_array().ok_or("physical ancestry")?;
        match (ancestor, containers.as_slice()) {
            (None, []) => assert_eq!(owner["storedEncryption"], "encrypted", "{owner}"),
            (Some(index), [parent_id]) => {
                assert_eq!(
                    owner["storedEncryption"], "unknown",
                    "the inner directory declares no independent encryption state"
                );
                let parent = Self::find(&self.sources, parent_id)?;
                assert_eq!(parent["entryIndex"], index, "{parent}");
                assert_eq!(parent["storedEncryption"], "encrypted", "{parent}");
                assert_eq!(
                    parent.pointer("/path/value"),
                    Some(&json!(CONTAINER)),
                    "{parent}"
                );
            }
            _ => return Err("exact physical cache ancestry".into()),
        }
        Ok(())
    }

    fn find<'a>(inventory: &'a [Value], id: &Value) -> Check<&'a Value> {
        inventory
            .iter()
            .find(|row| row.get("id") == Some(id))
            .ok_or_else(|| "original inventory reference".into())
    }
}
