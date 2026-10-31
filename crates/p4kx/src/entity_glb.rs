// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use anyhow::Result;
use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, CreatureAnimations, CreatureClips,
    ForgePalettes, LoadoutResolver, creature_animations,
};
use p4kx_extraction::{
    ConversionError, DataForgeEntry, GlbOptions, GlbSummary, GltfError, MaterialMode, is_same_file,
    write_replacing,
};
use thiserror::Error;

use crate::cli_entity::EntityGlbArgs;
use crate::cli_entity::EntityMaterialsArg;
use crate::entity_glb_animation::print_animations;
use crate::entity_glb_json::GlbReport;
use crate::entity_glb_report::print_report;

/// The output of `p4kx entity glb` is the Archive it reads.
#[derive(Debug, Error)]
#[error("{} is the Archive the Entity is read from", .0.display())]
pub(crate) struct GlbOutputIsArchive(pub(crate) PathBuf);

/// Runs `p4kx entity glb`.
pub(crate) fn run(args: &EntityGlbArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    let changes = args.changes.changes()?;
    let archive = crate::key::open(&args.archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    if !args.expand_docked_vehicles {
        resolver = resolver.stopping_at_docked_vehicles();
    }
    let record = resolver.entities().find(&args.entity)?;
    let applied = resolver.resolve_changed(&record, &changes)?;
    crate::entity_ignored::warn(&applied)?;
    let tree = applied.node();
    let models = ArchiveModels::new(&archive);
    let options = AssemblyOptions::default().with_lod(args.lod);
    let palettes = ForgePalettes::new(&forge);
    let assembly = Assembly::build_with_palettes(tree, &models, &palettes, &options)?;
    crate::entity_root::check(&assembly)?;
    let assembly = if args.interior {
        crate::interior_forge::with_interior(assembly, (&archive, &forge), tree, args.lod)?
    } else {
        assembly
    };
    let found = if args.animations {
        creature_animations(&models, &assembly, tree)?
    } else {
        CreatureClips::default()
    };
    let animated = assembly.animate(&found.set.animations);
    let summary = write(&assembly, &animated, &models, args)?;
    if as_json {
        return GlbReport {
            archive: archive.path(),
            args,
            tree,
            assembly: &assembly,
            summary: &summary,
            applied: &applied,
            changes: &changes,
            animations: (&found, &animated),
        }
        .print();
    }
    let printed = print_report(&assembly, &summary, &args.out, tree, &changes)
        .and_then(|()| print_animations(&summary, &found, &animated));
    match printed {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(()),
    }
}

fn write(
    assembly: &Assembly,
    animated: &CreatureAnimations<'_>,
    models: &ArchiveModels<'_>,
    args: &EntityGlbArgs,
) -> Result<GlbSummary> {
    let mode = match args.materials {
        EntityMaterialsArg::Colours => MaterialMode::Colours,
        EntityMaterialsArg::Textures => MaterialMode::Textures,
    };
    let mut options = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        options = options.without_vertex_colours();
    }
    let name = assembly
        .root()
        .map_or_else(String::new, |root| root.class_name().to_owned());
    let out: &Path = &args.out;
    let textures = models.textures();
    Ok(write_replacing(out, |writer| {
        assembly
            .write_glb_animated_with(animated, &options, &textures, writer)
            .map_err(|error| match error {
                GltfError::Write(source) => ConversionError::Write {
                    path: out.to_path_buf(),
                    source,
                },
                source => ConversionError::Glb { file: name, source },
            })
    })?)
}
