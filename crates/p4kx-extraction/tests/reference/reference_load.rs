// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::{MANIFEST_NAME, MAX_MANIFEST_BYTES, Manifest};
use super::reference_paths;

pub(crate) struct Loaded {
    pub(crate) manifest: Manifest,
    pub(crate) fingerprint: super::reference_manifest::Fingerprint,
}

pub(crate) fn read(root: &Path) -> Result<Loaded> {
    let canonical = reference_paths::root(root)?;
    let root = canonical.as_path();
    let path = root.join(MANIFEST_NAME);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(ReferenceError::new(
                Code::MissingManifest,
                "configured root has no reference manifest",
            ));
        }
        Err(error) => return Err(ReferenceError::new(Code::Manifest, error.to_string())),
        Ok(_) => {}
    }
    let path = reference_paths::resolve(root, MANIFEST_NAME)?;
    let file =
        File::open(path).map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    if file
        .metadata()
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?
        .len()
        > MAX_MANIFEST_BYTES
    {
        return Err(ReferenceError::new(
            Code::Bounds,
            "manifest exceeds its byte bound",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "manifest grew beyond its byte bound",
        ));
    }
    super::reference_limits::json_tokens(&bytes)?;
    let fingerprint = super::reference_digest::reader(bytes.as_slice(), MAX_MANIFEST_BYTES)?;
    let manifest = serde_json::from_slice(&bytes)
        .map_err(|error| ReferenceError::new(Code::Manifest, error.to_string()))?;
    Ok(Loaded {
        manifest,
        fingerprint,
    })
}
