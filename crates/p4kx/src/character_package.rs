// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsStr;
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::character::{CHF_SIZE, Chf};
use p4kx_assembly::{
    AnimationFolders, ArchiveFiles, ArchiveModels, Armour, AssemblyOptions, BodyVariant,
    Expressions, ForgeItems, ForgePalettes, Head, LoadoutResolver, Piece, character_animations,
};
use p4kx_blender::{
    CharacterScene, PackageInfo, PackageSummary, PackageWriteContext, check_output_with_context,
    write_character_package_with_context,
};
use p4kx_channels::channel_name;
use p4kx_extraction::{Archive, DataForgeEntry, GlbOptions, MaterialMode, is_same_file};

use crate::armour_loadout::{Files, Selection};
use crate::chf::{InvalidChf, read_capped};
use crate::chf_glb::{expressions_choice, failure};
use crate::cli_armour::BodyArg;
use crate::cli_character::{CharacterAction, CharacterArgs, CharacterPackageArgs};
use crate::cli_chf::ChfMaterialsArg;
use crate::entity_glb::GlbOutputIsArchive;
use crate::entity_package::build_info;
use crate::extract::cancel_on_interrupt;

mod report;

/// Runs a `p4kx character` subcommand.
pub(crate) fn run(args: &CharacterArgs, as_json: bool) -> Result<()> {
    match &args.action {
        CharacterAction::Package(args) => package(args, as_json),
    }
}

fn glb_options(args: &CharacterPackageArgs) -> GlbOptions {
    let mode = match args.materials {
        ChfMaterialsArg::Colours => MaterialMode::Colours,
        ChfMaterialsArg::Textures => MaterialMode::Textures,
    };
    let glb = GlbOptions::default()
        .with_lod(args.lod)
        .with_materials(mode)
        .with_texture_mip(args.texture_mip);
    if args.no_vertex_colours {
        glb.without_vertex_colours()
    } else {
        glb
    }
}

fn package(args: &CharacterPackageArgs, as_json: bool) -> Result<()> {
    if is_same_file(&args.out, &args.archive) {
        return Err(GlbOutputIsArchive(args.out.clone()).into());
    }
    let chf = args.chf.as_deref().map(read_chf).transpose()?;
    let archive = crate::key::open(&args.archive)?;
    let cancel = cancel_on_interrupt()?;
    let protection = p4kx_extraction::ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let context = PackageWriteContext::new(&protection, cancel.flag());
    check_output_with_context(&args.out, &context)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = Files::open(ArchiveFiles::new(&archive), args.loadout.as_deref())?;
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let expressions = expressions_choice(&args.expressions, args.no_expressions);
    let mut selection = Selection::gather(
        &mut resolver,
        &files,
        (&args.pieces, args.loadout.as_deref()),
        &expressions,
    )?;
    let head = chf
        .as_ref()
        .map(|chf| dress(&expressions, chf, &mut resolver))
        .transpose()?;
    let (variant, pieces) = worn(args, &mut selection, head.as_ref());
    let models = ArchiveModels::new(&archive);
    let options = AssemblyOptions::default().with_lod(args.lod);
    let armour = Armour::build_with_palettes(
        variant,
        &pieces,
        &models,
        &ForgePalettes::new(&forge),
        &options,
    )?;
    let folders = animation_folders(args, &selection, &models, variant);
    let found = character_animations(
        &models,
        variant,
        (&args.animations, &args.animation_sources),
        &folders,
    )?;
    let animated = armour.animate(&found.animations, &models);

    let name = name_of(args, variant);
    let chf_name = origin_chf_name(args);
    let scene = CharacterScene::new(&name, &armour, &animated)
        .with_origin(chf_name, args.loadout.as_deref());
    let summary = store(args, &archive, &models, &scene, &context)?;

    let shown = report::Shown {
        archive: archive.path(),
        args,
        name: &name,
        head: head.as_ref(),
        variant,
        selection: &selection,
        armour: &armour,
        found: &found,
    };
    report::print(&shown, &summary, as_json)?;
    failure(&armour, &args.out).map_or(Ok(()), Err)
}

fn origin_chf_name(args: &CharacterPackageArgs) -> Option<&str> {
    args.chf
        .as_deref()
        .and_then(Path::file_name)
        .and_then(OsStr::to_str)
}

/// Returns the body variant and the pieces to wear: the CHF's head decides the body and takes the
/// place of the head a Loadout gives, else `--body`, else the Loadout's head, else the male body.
fn worn(
    args: &CharacterPackageArgs,
    selection: &mut Selection<'_>,
    head: Option<&Head>,
) -> (BodyVariant, Vec<Piece>) {
    let variant = head
        .map(Head::variant)
        .or_else(|| args.body.map(BodyVariant::from))
        .or_else(|| selection.variant())
        .unwrap_or_else(|| BodyVariant::from(BodyArg::Male));
    let mut pieces = std::mem::take(&mut selection.pieces);
    if let Some(head) = head {
        if let Some(npc) = &selection.head {
            pieces.retain(|piece| !npc.pieces().contains(piece));
        }
        pieces.extend(head.pieces().iter().cloned());
    }
    (variant, pieces)
}

fn animation_folders(
    args: &CharacterPackageArgs,
    selection: &Selection<'_>,
    models: &ArchiveModels<'_>,
    variant: BodyVariant,
) -> AnimationFolders {
    if !args.animation_folders.is_empty() {
        return AnimationFolders::only(&args.animation_folders);
    }
    selection
        .head
        .as_ref()
        .and_then(|head| head.folder())
        .map_or_else(AnimationFolders::default, |folder| {
            AnimationFolders::of_npc(models, variant, folder)
        })
}

fn store(
    args: &CharacterPackageArgs,
    archive: &Archive,
    models: &ArchiveModels<'_>,
    scene: &CharacterScene<'_>,
    context: &PackageWriteContext<'_>,
) -> Result<PackageSummary> {
    let info = PackageInfo::new(channel_name(archive.path()), build_info(archive)?);
    Ok(write_character_package_with_context(
        scene,
        &models.textures(),
        &glb_options(args),
        &info,
        &args.out,
        context,
    )?)
}

fn read_chf(path: &Path) -> Result<Chf> {
    let bytes = read_capped(path, CHF_SIZE as u64)?;
    Ok(Chf::read(&bytes).map_err(|source| InvalidChf {
        path: path.to_owned(),
        source,
    })?)
}

fn dress(expressions: &Expressions, chf: &Chf, resolver: &mut LoadoutResolver<'_>) -> Result<Head> {
    Ok(
        Head::dressed(&chf.character, &mut ForgeItems::new(resolver))?
            .with_expressions(expressions),
    )
}

fn name_of(args: &CharacterPackageArgs, variant: BodyVariant) -> String {
    args.name
        .clone()
        .or_else(|| {
            args.chf
                .as_ref()
                .and_then(|path| path.file_stem())
                .map(|stem| stem.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| variant.code().to_owned())
}
