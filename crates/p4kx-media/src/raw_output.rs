// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use crate::error;
use crate::hashing;
use crate::output_path;
use crate::{Artifact, ByteScope, Digest, MediaError, MediaWriter, RawReference};

pub(crate) fn place(
    payload: &Path,
    digest: Digest,
    size: u64,
    writer: &mut MediaWriter<'_>,
) -> Result<(RawReference, PathBuf), MediaError> {
    if !output_path::file(payload)? {
        return Err(MediaError::Destination(
            "retained source payload is missing".into(),
        ));
    }
    let relative = format!("raw/{}", digest.hex);
    let retained = writer.root.join(&relative);
    if output_path::file(&retained)? {
        let (existing, length) = hashing::file(
            &retained,
            ByteScope::DecodedEntryPayload,
            size,
            &writer.checkpoint,
        )?;
        if length != size || existing != digest {
            return Err(MediaError::Contract(
                "raw content-address collision or corruption".into(),
            ));
        }
    } else {
        fs::rename(payload, &retained).map_err(|source| error::io(&retained, source))?;
        fs::OpenOptions::new()
            .write(true)
            .open(&retained)
            .and_then(|file| file.sync_all())
            .map_err(|source| error::io(&retained, source))?;
    }
    writer.artifact(&Artifact {
        path: relative.clone(),
        size,
        digest: digest.clone(),
    })?;
    Ok((
        RawReference {
            path: relative,
            size,
            digest,
        },
        retained,
    ))
}
