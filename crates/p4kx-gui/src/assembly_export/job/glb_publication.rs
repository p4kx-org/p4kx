// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::io;
use std::path::PathBuf;

use p4kx_assembly::{ArchiveModels, Assembly, Interior};
use p4kx_extraction::{
    Archive, ArchiveWriteProtection, CancelToken, ConversionError, FileReceipt, GlbMeshOrigins,
    GlbOptions, GlbSummary, GltfError, TextureLoader, WriteProtectionError,
};

use thiserror::Error;

use super::stop::{Observed, StoppableTextures};
use super::{Request, Shared};
use crate::EntryIndex;

pub(super) enum Scene<'a> {
    Entity(&'a Assembly),
    Interior {
        interior: &'a Interior,
        opened: &'a [Archive],
    },
}

pub(super) fn publish(
    index: &EntryIndex,
    request: &Request,
    scene: Scene<'_>,
    archive_models: &ArchiveModels<'_>,
    (cancel, shared): (&CancelToken, &Shared),
) -> Result<(GlbSummary, PathBuf), ConversionError> {
    let protection = ArchiveWriteProtection::new(index.path(), cancel.flag()).map_err(|error| {
        if error.is_cancelled() {
            ConversionError::Cancelled
        } else {
            ConversionError::Installation(error)
        }
    })?;
    let receipt =
        FileReceipt::for_staged_replacement(&request.file, Some(&protection), cancel.clone())
            .map_err(|error| conversion_error(request, cancel, error))?;
    let textures = archive_models.textures();
    let textures = StoppableTextures {
        textures: &textures,
        cancel,
    };
    let committed = Cell::new(false);
    let (summary, _) = receipt
        .publish_observing_commit(
            |writer| {
                let mut observed = Observed {
                    inner: writer,
                    cancel,
                    written: &shared.done,
                };
                let mut origins = GlbMeshOrigins::new();
                let summary = scene
                    .write(&request.options(), &textures, &mut observed, &mut origins)
                    .map_err(io::Error::other)?;
                Ok((summary, origins.finish()?))
            },
            || committed.set(true),
            |writer, binding, (_, origins)| origins.write_companion(writer, binding),
        )
        .map_err(|error| {
            if committed.get() {
                output_error(request, io::Error::other(CommittedGlbPublication(error)))
            } else {
                conversion_error(request, cancel, error)
            }
        })?;
    Ok((summary, receipt.path().to_path_buf()))
}

impl Scene<'_> {
    fn write(
        self,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        writer: &mut impl io::Write,
        origins: &mut GlbMeshOrigins,
    ) -> Result<GlbSummary, GltfError> {
        match self {
            Self::Entity(assembly) => assembly.write_glb_with_vertex_origins(
                options,
                textures,
                writer,
                &mut |mesh, _, sources| {
                    origins.observe_mesh(mesh.position_accessor, mesh.origin, sources)
                },
            ),
            Self::Interior { interior, opened } => {
                origins
                    .observe_containers(opened)
                    .map_err(GltfError::Write)?;
                interior.write_glb_with_vertex_origins(
                    options,
                    textures,
                    writer,
                    &mut |mesh, _, sources| {
                        origins.observe_mesh(mesh.position_accessor, mesh.origin, sources)
                    },
                )
            }
        }
    }
}

fn conversion_error(request: &Request, cancel: &CancelToken, error: io::Error) -> ConversionError {
    match error.downcast::<GltfError>() {
        Ok(GltfError::Write(_)) if cancel.is_cancelled() => ConversionError::Cancelled,
        Err(error) if cancel.is_cancelled() && interrupted(&error) => ConversionError::Cancelled,
        Ok(GltfError::Write(source)) | Err(source) => output_error(request, source),
        Ok(source) => ConversionError::Glb {
            file: request.name.clone(),
            source,
        },
    }
}

fn interrupted(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::Interrupted
        || error
            .get_ref()
            .and_then(|source| source.downcast_ref::<WriteProtectionError>())
            .is_some_and(WriteProtectionError::is_cancelled)
}

fn output_error(request: &Request, source: io::Error) -> ConversionError {
    ConversionError::Write {
        path: request.file.clone(),
        source,
    }
}

#[derive(Debug, Error)]
#[error("the GLB was committed, but origin publication did not finish: {0}")]
struct CommittedGlbPublication(#[source] io::Error);
