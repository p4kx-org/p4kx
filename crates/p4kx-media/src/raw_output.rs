// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::origin_input::MediaOriginInput;
use p4kx_extraction::InputScope;

use std::fs;
use std::path::{Path, PathBuf};

use p4kx_extraction::OwnedDirectoryPermit;

use crate::error;
use crate::hashing;
use crate::output_path;
use crate::{Artifact, ByteScope, Digest, MediaError, MediaWriter, RawReference};

pub(crate) fn place(
    payload: &Path,
    source_namespace: &OwnedDirectoryPermit,
    digest: Digest,
    size: u64,
    origin: &MediaOriginInput,
    writer: &mut MediaWriter<'_>,
) -> Result<(RawReference, PathBuf), MediaError> {
    if !output_path::file(payload)? {
        return Err(MediaError::Destination(
            "retained source payload is missing".into(),
        ));
    }
    let relative = format!("raw/{}", digest.hex);
    let retained = writer.root.join(&relative);
    writer
        .namespace
        .ensure_namespace_writable(&retained, writer.checkpoint.cancel.flag())?;
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
        source_namespace.ensure_namespace_writable(payload, writer.checkpoint.cancel.flag())?;
        fs::rename(payload, &retained).map_err(|source| error::io(&retained, source))?;
        writer
            .namespace
            .ensure_namespace_writable(&retained, writer.checkpoint.cancel.flag())
            .map_err(|cause| MediaError::RetainedOutput {
                path: retained.clone(),
                cause: Box::new(cause.into()),
            })?;
    }
    writer.artifact_with_input(
        &Artifact {
            path: relative.clone(),
            size,
            digest: digest.clone(),
        },
        origin,
        InputScope::Payload,
    )?;
    Ok((
        RawReference {
            path: relative,
            size,
            digest,
        },
        retained,
    ))
}
