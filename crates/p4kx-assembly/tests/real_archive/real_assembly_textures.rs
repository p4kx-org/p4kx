// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Palette and texture cases verify every requested mode without exposing source data.

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, Entities, ForgePalettes, IssueKind};
use p4kx_gltf::{GlbOptions, MaterialMode};
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{self, Case, Check, DockedVehicles, Family, Index, Requirements};

const FAMILIES: [Family; 2] = [Family::Gladius, Family::Hornet];
const MODES: [(MaterialMode, u32); 4] = [
    (MaterialMode::Colours, 0),
    (MaterialMode::Textures, 3),
    (MaterialMode::Textures, 1),
    (MaterialMode::Textures, 0),
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn documented_ships_are_written_with_textures_and_palettes() {
    media_fixtures::run(verify);
}

fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let index = Index::new(Arc::new(Entities::new(forge)), files)?;
        let models = ArchiveModels::new(archive);
        let palettes = ForgePalettes::new(forge);
        let directory = tempfile::tempdir()?;
        let mut report = RunReport::default();
        let cases = FAMILIES.into_iter().enumerate();
        for (ordinal, family) in cases {
            let case = Case::from_family(
                "palette_source",
                family,
                Requirements {
                    geometry_lod: Some(0),
                    item_size: None,
                    requires_interior: false,
                    requires_audio: false,
                    docked: DockedVehicles::Expand,
                },
            );
            report.run(format!("palette_{ordinal}"), || {
                let Some(choice) = index.choose(archive, &case)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&case, &choice);
                let assembly = Assembly::build_with_palettes(
                    choice.capabilities.node()?,
                    &models,
                    &palettes,
                    &AssemblyOptions::default(),
                )?;
                palette_counts(&assembly)?;
                let mut conversions = RunReport::default();
                for (mode, mip) in MODES {
                    conversions.run(format!("palette_{ordinal}_{mode:?}_{mip}"), || {
                        let path = directory
                            .path()
                            .join(format!("palette_{ordinal}_{mode:?}_{mip}.glb"));
                        write(&assembly, &models, mode, mip, &path)?;
                        Ok(CaseOutcome::Completed(()))
                    });
                }
                media_fixtures::finish(&conversions)?;
                Ok(CaseOutcome::Completed(()))
            });
        }
        media_fixtures::finish(&report)
    })
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate palette counts are reported"
)]
fn palette_counts(assembly: &Assembly) -> media_fixtures::Result<()> {
    let painted = assembly
        .parts()
        .iter()
        .filter(|part| part.palette().is_some())
        .count();
    let issues = assembly
        .issues()
        .iter()
        .filter(|issue| matches!(issue.kind(), IssueKind::Palette { .. }))
        .count();
    println!(
        "media palette totals: parts={} painted={} palette_issues={}",
        assembly.parts().len(),
        painted,
        issues
    );
    media_fixtures::require(painted > 0, Check::PalettePresent)
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate GLB and texture counts are reported"
)]
fn write(
    assembly: &Assembly,
    models: &ArchiveModels<'_>,
    mode: MaterialMode,
    mip: u32,
    path: &Path,
) -> media_fixtures::Result<()> {
    use std::io::Write as _;

    let options = GlbOptions::default()
        .with_materials(mode)
        .with_texture_mip(mip);
    let mut writer = BufWriter::new(File::create(path)?);
    let summary = assembly.write_glb_with(&options, &models.textures(), &mut writer)?;
    writer.flush()?;
    drop(writer);
    p4kx_testkit::check_glb(&std::fs::read(path)?)?;
    std::fs::remove_file(path)?;
    println!(
        "media palette mode {mode:?} mip {mip}: bytes={} nodes={} meshes={} materials={} textured={} shared_materials={} images={} pixels={} shared_images={} fallbacks={} shared_meshes={}",
        summary.bytes,
        summary.nodes,
        summary.meshes,
        summary.materials,
        summary.textured_materials,
        summary.shared_materials,
        summary.textures,
        summary.texture_pixels,
        summary.shared_textures,
        summary.texture_fallbacks.len(),
        summary.shared_meshes
    );
    if mode == MaterialMode::Textures {
        media_fixtures::require(summary.textures > 0, Check::Images)?;
    }
    Ok(())
}
