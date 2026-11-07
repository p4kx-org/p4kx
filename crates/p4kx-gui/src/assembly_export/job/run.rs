// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyError, AssemblyOptions, EntityInterior, EntityNode,
    ForgePalettes, Interior, InteriorForge, InteriorOptions, InteriorProgress, InteriorWatch,
    LoadoutChanges,
};
use p4kx_extraction::{
    Archive, ArchiveWriteProtection, CancelToken, ConversionError, DataForge, GlbSummary,
    GltfError, Guid, PathIndex, write_replacing_protected,
};

use super::stop::{Observed, Stoppable, StoppableTextures};
use super::{AssemblyExportStage, Ended, Request, Shared, Subject, failure_of, package};
use crate::assembly_export::{AssemblyExportForm, AssemblyExportSummary};
use crate::entities::{ResolveFailure, resolve};
use crate::forge_cache::ReadFailure;
use crate::model::describe;
use crate::{EntryIndex, ModelExportFailure, NoEntries, PortMismatch};

pub(super) fn run(
    index: &EntryIndex,
    request: &Request,
    cancel: &CancelToken,
    shared: &Shared,
) -> Ended {
    match &request.subject {
        Subject::Entity {
            guid,
            changes,
            interior,
        } => entity(
            index,
            request,
            (*guid, changes, *interior),
            (cancel, shared),
        ),
        Subject::Container { chain } => container(index, request, chain, (cancel, shared)),
    }
}

fn with_watch<T>(
    (cancel, shared): (&CancelToken, &Shared),
    build: impl FnOnce(&InteriorWatch<'_>) -> T,
) -> T {
    build(&InteriorWatch {
        cancelled: &|| cancel.is_cancelled(),
        progress: &|progress| match progress {
            InteriorProgress::Container { done, total } => shared.containers(done, total),
            InteriorProgress::Models { done, total } => shared.models(done, total),
            _ => {}
        },
    })
}

fn interior_ended(index: &EntryIndex, cancel: &CancelToken, error: AssemblyError) -> Ended {
    match error {
        AssemblyError::Cancelled => Ended::Cancelled,
        _ if cancel.is_cancelled() => Ended::Cancelled,
        AssemblyError::Provider(error) => failed(index, &*error),
        other => failed(index, &other),
    }
}

fn entity(
    index: &EntryIndex,
    request: &Request,
    (guid, changes, interior): (Guid, &LoadoutChanges, bool),
    (cancel, shared): (&CancelToken, &Shared),
) -> Ended {
    let applied = match resolve(index, guid, changes, &|| cancel.is_cancelled()) {
        Ok(applied) => applied,
        Err(ResolveFailure::Cancelled) => return Ended::Cancelled,
        Err(ResolveFailure::Lost) => return Ended::Failed(ModelExportFailure::Stopped),
        Err(ResolveFailure::ArchiveChanged) => {
            return Ended::Failed(ModelExportFailure::ArchiveChanged);
        }
        Err(ResolveFailure::NoRecord(guid)) => {
            return Ended::Failed(ModelExportFailure::NoRecord(guid));
        }
        Err(ResolveFailure::Failed(reason)) => {
            return Ended::Failed(ModelExportFailure::Failed(reason));
        }
    };
    let mismatches = PortMismatch::all(applied.mismatches());
    let tree = applied.into_node();
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    shared.enter(AssemblyExportStage::Reading);
    #[cfg(feature = "test-hooks")]
    request.pause.wait(|| cancel.is_cancelled());
    let Ok(archive) = index.entries_archive() else {
        return NoEntries.into();
    };
    let archive_models = ArchiveModels::new(archive);
    let assembly = match assembled(
        index,
        (&tree, interior),
        (request.lod, &archive_models),
        (cancel, shared),
    ) {
        Ok(assembly) => assembly,
        Err(ended) => return ended,
    };
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    shared.enter(AssemblyExportStage::Writing);
    let models = (&assembly, &archive_models);
    let ended = if request.form == AssemblyExportForm::Package {
        package::write(index, request, models, (changes, cancel))
    } else {
        write_glb(index, request, models, (changes, cancel, shared))
    };
    match ended {
        Ended::Written(summary) => Ended::Written(Box::new(summary.with_mismatches(mismatches))),
        other => other,
    }
}

/// Places the parts of `tree` and, when `interior` is set, its Interior, reading the DataForge of
/// the Archive once for both.
fn assembled(
    index: &EntryIndex,
    (tree, interior): (&EntityNode, bool),
    (lod, archive_models): (u32, &ArchiveModels<'_>),
    (cancel, shared): (&CancelToken, &Shared),
) -> Result<Assembly, Ended> {
    let source = match index.forge().read(index, || {}) {
        Ok(source) => source,
        Err(ReadFailure::ArchiveChanged(_)) => {
            return Err(Ended::Failed(ModelExportFailure::ArchiveChanged));
        }
        Err(ReadFailure::Unreadable(reason)) => {
            return Err(Ended::Failed(ModelExportFailure::Failed(reason)));
        }
    };
    let forge = match source.entry.open() {
        Ok(forge) => forge,
        Err(error) => {
            return Err(Ended::Failed(ModelExportFailure::Failed(describe(&error))));
        }
    };
    let assembly = hull(index, (&forge, tree), lod, (cancel, archive_models))?;
    if !interior {
        return Ok(assembly);
    }
    shared.enter(AssemblyExportStage::Interior);
    let classes = InteriorForge::new(&forge);
    let built = interior_of(index, (&assembly, tree), (lod, &classes), (cancel, shared))?;
    Ok(assembly.with_interior(built))
}

fn write_glb(
    index: &EntryIndex,
    request: &Request,
    (assembly, archive_models): (&Assembly, &ArchiveModels<'_>),
    (changes, cancel, shared): (&LoadoutChanges, &CancelToken, &Shared),
) -> Ended {
    let protection = match ArchiveWriteProtection::new(index.path(), cancel.flag()) {
        Ok(protection) => protection,
        Err(error) if error.is_cancelled() => return Ended::Cancelled,
        Err(error) => return Ended::Failed(ModelExportFailure::Refused(error)),
    };
    let written =
        write_replacing_protected(&request.file, Some(&protection), Some(cancel), |writer| {
            let mut observed = Observed {
                inner: writer,
                cancel,
                written: &shared.done,
            };
            let textures = archive_models.textures();
            let textures = StoppableTextures {
                textures: &textures,
                cancel,
            };
            assembly
                .write_glb_with(&request.options(), &textures, &mut observed)
                .map_err(|error| glb_error(error, request, cancel))
        });
    ended(index, written, |written| {
        AssemblyExportSummary::new(
            (request.file.clone(), request.name.clone()),
            assembly,
            (written, changes),
        )
    })
}

/// Reads the models of the parts of `tree` and places them.
fn hull(
    index: &EntryIndex,
    (forge, tree): (&DataForge<'_>, &EntityNode),
    lod: u32,
    (cancel, archive_models): (&CancelToken, &ArchiveModels<'_>),
) -> Result<Assembly, Ended> {
    let palettes = ForgePalettes::new(forge);
    let models = Stoppable {
        models: archive_models,
        cancel,
    };
    let options = AssemblyOptions::default().with_lod(lod);
    Assembly::build_with_palettes(tree, &models, &palettes, &options).map_err(|error| match error {
        _ if cancel.is_cancelled() => Ended::Cancelled,
        AssemblyError::Provider(error) => failed(index, &*error),
        other => failed(index, &other),
    })
}

fn interior_of(
    index: &EntryIndex,
    (assembly, tree): (&Assembly, &EntityNode),
    (lod, classes): (u32, &InteriorForge<'_>),
    state: (&CancelToken, &Shared),
) -> Result<EntityInterior, Ended> {
    let archive = index.entries_archive().map_err(Ended::from)?;
    let options = InteriorOptions::default().with_lod(lod);
    with_watch(state, |watch| {
        EntityInterior::build_with(archive, assembly, tree, &options, watch, Some(classes))
    })
    .map_err(|error| interior_ended(index, state.0, error))
}

/// Runs `build` with the class Records of the Archive's DataForge, or without them when the
/// Archive has no readable one; fails when the Archive changed on disk.
fn with_classes<T>(
    index: &EntryIndex,
    build: impl FnOnce(Option<&InteriorForge<'_>>) -> T,
) -> Result<T, Ended> {
    let source = match index.forge().read(index, || {}) {
        Ok(source) => source,
        Err(ReadFailure::ArchiveChanged(_)) => {
            return Err(Ended::Failed(ModelExportFailure::ArchiveChanged));
        }
        Err(ReadFailure::Unreadable(_)) => return Ok(build(None)),
    };
    let Ok(forge) = source.entry.open() else {
        return Ok(build(None));
    };
    let classes = InteriorForge::new(&forge);
    Ok(build(Some(&classes)))
}

fn container_interior(
    index: &EntryIndex,
    (layers, entry): (&[&Archive], &str),
    lod: u32,
    state: (&CancelToken, &Shared),
) -> Result<Interior, Ended> {
    let options = InteriorOptions::default().with_lod(lod);
    with_classes(index, |classes| {
        with_watch(state, |watch| {
            Interior::build_with(layers, entry, &options, watch, classes)
        })
    })?
    .map_err(|error| interior_ended(index, state.0, error))
}

fn container(
    index: &EntryIndex,
    request: &Request,
    chain: &[String],
    (cancel, shared): (&CancelToken, &Shared),
) -> Ended {
    shared.enter(AssemblyExportStage::Interior);
    #[cfg(feature = "test-hooks")]
    request.pause.wait(|| cancel.is_cancelled());
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    let opened = match open_chain(index, chain) {
        Ok(opened) => opened,
        Err(failure) => return Ended::Failed(failure),
    };
    let Ok(archive) = index.entries_archive() else {
        return NoEntries.into();
    };
    let layers: Vec<&Archive> = std::iter::once(archive).chain(opened.iter()).collect();
    let entry = chain.last().map_or("", String::as_str);
    let interior = match container_interior(index, (&layers, entry), request.lod, (cancel, shared))
    {
        Ok(interior) => interior,
        Err(ended) => return ended,
    };
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    if index.watch().check().is_err() {
        return Ended::Failed(ModelExportFailure::ArchiveChanged);
    }
    shared.enter(AssemblyExportStage::Writing);
    let archive_models = ArchiveModels::new(archive);
    if request.form == AssemblyExportForm::Package {
        return package::write_interior(index, request, &interior, (&archive_models, cancel));
    }
    let protection = match ArchiveWriteProtection::new(index.path(), cancel.flag()) {
        Ok(protection) => protection,
        Err(error) if error.is_cancelled() => return Ended::Cancelled,
        Err(error) => return Ended::Failed(ModelExportFailure::Refused(error)),
    };
    let written =
        write_replacing_protected(&request.file, Some(&protection), Some(cancel), |writer| {
            let mut observed = Observed {
                inner: writer,
                cancel,
                written: &shared.done,
            };
            let textures = archive_models.textures();
            let textures = StoppableTextures {
                textures: &textures,
                cancel,
            };
            interior
                .write_glb_with(&request.options(), &textures, &mut observed)
                .map_err(|error| glb_error(error, request, cancel))
        });
    ended(index, written, |written| {
        AssemblyExportSummary::of_interior(
            (request.file.clone(), request.name.clone()),
            &interior,
            written,
        )
    })
}

/// Opens the containers of `chain` one inside the other, the first from the Archive.
fn open_chain(index: &EntryIndex, chain: &[String]) -> Result<Vec<Archive>, ModelExportFailure> {
    let archive = index.entries_archive()?;
    let mut opened: Vec<Archive> = Vec::with_capacity(chain.len());
    for path in chain {
        let next = match opened.last() {
            Some(current) => open_in(index, current, &PathIndex::new(current), path)?,
            None => open_in(index, archive, &index.path_index().ok_or(NoEntries)?, path)?,
        };
        opened.push(next);
    }
    Ok(opened)
}

fn open_in(
    index: &EntryIndex,
    layer: &Archive,
    paths: &PathIndex<'_>,
    path: &str,
) -> Result<Archive, ModelExportFailure> {
    let Some(entry) = paths.find(path) else {
        return Err(ModelExportFailure::Failed(format!(
            "{path} is not in the Archive or container it was selected in"
        )));
    };
    layer
        .open_container(entry)
        .map_err(|error| failure_of(index, &error))
}

fn glb_error(error: GltfError, request: &Request, cancel: &CancelToken) -> ConversionError {
    match error {
        GltfError::Write(_) if cancel.is_cancelled() => ConversionError::Cancelled,
        GltfError::Write(source) => ConversionError::Write {
            path: request.file.clone(),
            source,
        },
        source => ConversionError::Glb {
            file: request.name.clone(),
            source,
        },
    }
}

fn ended(
    index: &EntryIndex,
    written: Result<GlbSummary, ConversionError>,
    summary: impl FnOnce(GlbSummary) -> AssemblyExportSummary,
) -> Ended {
    match written {
        Ok(written) => Ended::Written(Box::new(summary(written))),
        Err(ConversionError::Cancelled) => Ended::Cancelled,
        Err(error) => failed(index, &error),
    }
}

pub(super) fn failed(index: &EntryIndex, error: &(dyn std::error::Error + 'static)) -> Ended {
    Ended::Failed(failure_of(index, error))
}
