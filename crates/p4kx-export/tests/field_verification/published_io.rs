// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use p4kx_testkit::offline_seam::{self, Fingerprint, Limits};
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::error::{self, Error, Kind};
use super::published_model::{self as model, METADATA, Snapshot};

pub(crate) struct Dataset {
    pub(crate) root: PathBuf,
    pub(crate) snapshot: Snapshot,
    pub(crate) documents: BTreeMap<String, Value>,
    identities: BTreeMap<PathBuf, Fingerprint>,
    pub(crate) outside_selection_files: usize,
}

pub(crate) fn read_json<T: DeserializeOwned>(
    path: &Path,
    limit: u64,
    cancelled: &impl Fn() -> bool,
) -> super::Result<T> {
    let loaded = offline_seam::load(path, limit, cancelled)?;
    Ok(serde_json::from_slice(&loaded.bytes)?)
}

fn sha256(text: &str) -> super::Result<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(Error::new(Kind::Identity, "invalid canonical SHA-256").into());
    }
    let mut bytes = [0; 32];
    let positions = bytes.iter_mut().enumerate();
    for (index, value) in positions {
        let pair = text
            .get(index * 2..index * 2 + 2)
            .ok_or_else(|| Error::new(Kind::Identity, "incomplete SHA-256"))?;
        *value = u8::from_str_radix(pair, 16)?;
    }
    Ok(bytes)
}

fn validate_fingerprint(file: &model::File, measured: Fingerprint) -> super::Result<()> {
    if measured.bytes != file.bytes || measured.sha256 != sha256(&file.sha256)? {
        return Err(Error::new(
            Kind::Identity,
            "reference bytes differ from the local snapshot claim",
        )
        .into());
    }
    Ok(())
}

impl Dataset {
    pub(crate) fn load(root: &Path, cancelled: &impl Fn() -> bool) -> super::Result<Self> {
        let root = offline_seam::direct_root(root)?;
        let metadata = offline_seam::contained_file(&root, METADATA, 4096)?;
        let metadata_file = offline_seam::load(&metadata, model::MAX_METADATA_BYTES, cancelled)?;
        let snapshot: Snapshot = serde_json::from_slice(&metadata_file.bytes)?;
        super::published_validate::validate(&snapshot)?;
        let inventory = offline_seam::inventory(
            &root,
            Limits {
                path_bytes: 4096,
                entries: 1_000_000,
            },
            Some(METADATA),
            cancelled,
        )?;
        let mut documents = BTreeMap::new();
        let mut identities = BTreeMap::from([(metadata, metadata_file.fingerprint)]);
        let mut size = 0_u64;
        for file in &snapshot.files {
            error::check(cancelled)?;
            let key = offline_seam::relative_path(&file.path, 4096)?.to_lowercase();
            let path = inventory
                .get(&key)
                .ok_or_else(|| Error::new(Kind::Input, "selected reference file is missing"))?;
            let loaded = offline_seam::load(
                path,
                model::MAX_JSON_BYTES.min(model::MAX_DATASET_BYTES - size),
                cancelled,
            )?;
            size = size
                .checked_add(loaded.fingerprint.bytes)
                .filter(|size| *size <= model::MAX_DATASET_BYTES)
                .ok_or_else(|| {
                    Error::new(
                        Kind::Bounds,
                        "selected dataset exceeds its total byte bound",
                    )
                })?;
            validate_fingerprint(file, loaded.fingerprint)?;
            if documents
                .insert(key, serde_json::from_slice(&loaded.bytes)?)
                .is_some()
            {
                return Err(Error::new(Kind::Identity, "duplicate reference file mapping").into());
            }
            identities.insert(path.clone(), loaded.fingerprint);
        }
        let outside_selection_files = inventory.len().saturating_sub(documents.len());
        Ok(Self {
            root,
            snapshot,
            documents,
            identities,
            outside_selection_files,
        })
    }

    pub(crate) fn unchanged(&self, cancelled: &impl Fn() -> bool) -> super::Result<()> {
        let root = offline_seam::direct_root(&self.root)?;
        for (path, fingerprint) in &self.identities {
            let relative = path
                .strip_prefix(&root)?
                .to_str()
                .ok_or_else(|| Error::new(Kind::Input, "reference identity path is not UTF-8"))?;
            let path = offline_seam::contained_file(&root, relative, 4096)?;
            if offline_seam::fingerprint(&path, model::MAX_JSON_BYTES, cancelled)? != *fingerprint {
                return Err(Error::new(
                    Kind::Identity,
                    "published reference changed during comparison",
                )
                .into());
            }
        }
        Ok(())
    }
}
