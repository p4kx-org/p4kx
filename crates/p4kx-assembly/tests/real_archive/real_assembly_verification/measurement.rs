// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::time::{Duration, Instant};

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, Placement};
use p4kx_extraction::{Archive, DataForge};
use p4kx_gltf::{GlbOptions, GlbSummary, MaterialMode};
use p4kx_model::Model;

use crate::media_fixtures::Prepared;
use crate::real_entity_interior::{self, Bounds};

use super::TEXTURE_MIP;

const BOX_VERTICES: usize = 24;
const BOX_TRIANGLES: usize = 12;
const HULL_MARGIN_RATIO: f32 = 0.5;
const PLACEMENT_TOLERANCE: f32 = 1e-3;

pub(super) struct Measured {
    pub(super) parts: usize,
    pub(super) modelled: usize,
    pub(super) skipped: usize,
    pub(super) boxes: usize,
    pub(super) origin: usize,
    pub(super) placed_children: usize,
    pub(super) near_hull: usize,
    pub(super) misparented: usize,
    pub(super) summary: GlbSummary,
    pub(super) built: Duration,
    pub(super) written: Duration,
}

pub(super) fn is_box(model: &Model, lod: u32) -> bool {
    let Some(level) = model
        .lods
        .iter()
        .find(|level| level.level == lod)
        .or_else(|| model.lods.first())
    else {
        return false;
    };
    level.vertex_count() > 0
        && level.vertex_count() <= BOX_VERTICES
        && level.triangle_count() <= BOX_TRIANGLES
}

fn near_hull(point: [f32; 3], (min, max): &Bounds) -> bool {
    let extent = min
        .iter()
        .zip(max)
        .map(|(low, high)| high - low)
        .fold(0.0_f32, f32::max);
    let margin = extent * HULL_MARGIN_RATIO;
    point
        .iter()
        .zip(min.iter().zip(max))
        .all(|(value, (low, high))| *value >= low - margin && *value <= high + margin)
}

pub(super) fn measure<'f>(
    archive: &Archive,
    forge: &'f DataForge<'f>,
    prepared: &Prepared,
    lod: u32,
    mode: MaterialMode,
    path: &Path,
) -> crate::media_fixtures::Result<Measured> {
    let models = ArchiveModels::new(archive);
    let palettes = ForgePalettes::new(forge);
    let started = Instant::now();
    let assembly = Assembly::build_with_palettes(
        prepared.node()?,
        &models,
        &palettes,
        &AssemblyOptions::default().with_lod(lod),
    )?;
    let built = started.elapsed();
    let started = Instant::now();
    let summary = write(&assembly, &models, lod, mode, path)?;
    let written = started.elapsed();
    let mut measured = Measured {
        parts: assembly.parts().len(),
        modelled: assembly.modelled_parts(),
        skipped: assembly
            .issues()
            .iter()
            .filter(|issue| issue.kind().skips())
            .count(),
        boxes: 0,
        origin: 0,
        placed_children: 0,
        near_hull: 0,
        misparented: 0,
        summary,
        built,
        written,
    };
    geometry(&assembly, lod, &mut measured);
    Ok(measured)
}

fn write(
    assembly: &Assembly,
    models: &ArchiveModels<'_>,
    lod: u32,
    mode: MaterialMode,
    path: &Path,
) -> crate::media_fixtures::Result<GlbSummary> {
    use std::io::Write as _;

    let options = GlbOptions::default()
        .with_lod(lod)
        .with_materials(mode)
        .with_texture_mip(TEXTURE_MIP);
    let mut writer = BufWriter::new(File::create(path)?);
    let summary = assembly.write_glb_with(&options, &models.textures(), &mut writer)?;
    writer.flush()?;
    drop(writer);
    p4kx_testkit::check_glb(&std::fs::read(path)?)?;
    std::fs::remove_file(path)?;
    Ok(summary)
}

fn geometry(assembly: &Assembly, lod: u32, measured: &mut Measured) {
    let hull = assembly
        .root()
        .and_then(|part| part.model())
        .and_then(real_entity_interior::hull_bounds);
    let children = assembly.parts().iter().skip(1);
    for part in children {
        let parent = part.parent().and_then(|index| assembly.parts().get(index));
        measured.misparented += usize::from(parent.is_none_or(|parent| {
            let expected = parent.world().then(part.local());
            !expected
                .rows
                .iter()
                .flatten()
                .zip(part.world().rows.iter().flatten())
                .all(|(a, b)| (a - b).abs() < PLACEMENT_TOLERANCE)
        }));
        measured.boxes += usize::from(part.model().is_some_and(|model| is_box(model, lod)));
        if part.model().is_none() {
            continue;
        }
        measured.placed_children += 1;
        measured.origin += usize::from(matches!(part.placement(), Placement::Origin));
        if let Some(bounds) = &hull {
            measured.near_hull += usize::from(near_hull(part.world().translation(), bounds));
        }
    }
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate geometry and conversion measurements are reported"
)]
pub(super) fn report(label: &str, mode: MaterialMode, measured: &Measured) {
    let summary = &measured.summary;
    println!(
        "media case {label} {mode:?}: parts={} modelled={} skipped={} boxes={} origin={} placed={} near_hull={} misparented={} nodes={} meshes={} shared_meshes={} materials={} images={} pixels={} texture_fallbacks={} triangles={} bytes={} build_ms={} write_ms={}",
        measured.parts,
        measured.modelled,
        measured.skipped,
        measured.boxes,
        measured.origin,
        measured.placed_children,
        measured.near_hull,
        measured.misparented,
        summary.nodes,
        summary.meshes,
        summary.shared_meshes,
        summary.materials,
        summary.textures,
        summary.texture_pixels,
        summary.texture_fallbacks.len(),
        summary.triangles,
        summary.bytes,
        measured.built.as_millis(),
        measured.written.as_millis()
    );
}
