// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;

use p4kx_assembly::{ArchiveModels, CharacterAnimations};
use p4kx_extraction::{
    Archive, ArchiveWriteProtection, CancelToken, ConversionError, FileReceipt, GlbSummary,
    GltfError, WriteProtectionError,
};

use super::Request;
use crate::assembly_export::{Observed, StoppableTextures};

pub(super) fn publish(
    archive: &Archive,
    request: &Request,
    cancel: &CancelToken,
    written: &AtomicU64,
) -> Result<(GlbSummary, PathBuf), ConversionError> {
    let protection =
        ArchiveWriteProtection::new(archive.path(), cancel.flag()).map_err(|error| {
            if error.is_cancelled() {
                ConversionError::Cancelled
            } else {
                ConversionError::Installation(error)
            }
        })?;
    let receipt =
        FileReceipt::for_staged_replacement(&request.file, Some(&protection), cancel.clone())
            .map_err(|error| uncommitted_error(request, cancel, error))?;
    let models = ArchiveModels::new(archive);
    let archive_textures = models.textures();
    let textures = StoppableTextures {
        textures: &archive_textures,
        cancel,
    };
    let options = request.options();
    let mut committed = false;
    let (summary, _) = receipt
        .publish_observing_commit(
            |writer| {
                let mut observed = Observed {
                    inner: writer,
                    cancel,
                    written,
                };
                request
                    .armour
                    .write_glb_with_origins(
                        &CharacterAnimations::default(),
                        &options,
                        &textures,
                        &mut observed,
                    )
                    .map_err(io::Error::other)
            },
            || committed = true,
            |writer, binding, (_, origins)| origins.write_companion(writer, binding),
        )
        .map_err(|error| {
            if committed {
                ConversionError::committed_glb_publication(&request.file, error)
            } else {
                uncommitted_error(request, cancel, error)
            }
        })?;
    Ok((summary, receipt.path().to_path_buf()))
}

fn uncommitted_error(request: &Request, cancel: &CancelToken, error: io::Error) -> ConversionError {
    let source = match error.downcast::<GltfError>() {
        Ok(GltfError::Write(_)) if cancel.is_cancelled() => return ConversionError::Cancelled,
        Ok(GltfError::Write(source)) | Err(source) => source,
        Ok(source) => {
            return ConversionError::Glb {
                file: request.name.clone(),
                source,
            };
        }
    };
    if FileReceipt::is_precommit_cancellation(&source, cancel) {
        return ConversionError::Cancelled;
    }
    if let Some(protection) = source
        .get_ref()
        .and_then(|error| error.downcast_ref::<WriteProtectionError>())
    {
        return ConversionError::Installation(protection.clone());
    }
    ConversionError::Write {
        path: request.file.clone(),
        source,
    }
}
