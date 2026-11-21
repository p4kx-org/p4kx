// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Generic layered-material cases keep their lighting-effect requirement.

use std::collections::BTreeSet;
use std::sync::Arc;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, Entities, ForgePalettes};
use p4kx_model::Material;
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{self, Case, Check, DockedVehicles, Family, Index, Requirements};

const FAMILIES: [Family; 3] = [Family::Polaris, Family::Gladius, Family::Constellation];
const DARK: f32 = 0.05;
const BLACK: f32 = 0.005;

fn luminance([red, green, blue]: [f32; 3]) -> f32 {
    0.0722_f32.mul_add(blue, 0.2126_f32.mul_add(red, 0.7152 * green))
}

#[derive(Default)]
struct Census {
    materials: usize,
    layered: usize,
    layers_resolved: usize,
    black_before: usize,
    black_after: usize,
    lightened: usize,
    darkened: usize,
    dark_before: usize,
    dark_after: usize,
    layered_dark_before: usize,
    layered_dark_after: usize,
    unresolved: BTreeSet<String>,
}

impl Census {
    fn material(&mut self, material: &Material) {
        let mut plain = material.clone();
        plain.layers.clear();
        let shade_before = luminance(p4kx_gltf::base_colour_factor(&plain));
        let shade_after = luminance(p4kx_gltf::base_colour_factor(material));
        let before = shade_before < DARK;
        let after = shade_after < DARK;
        self.black_before += usize::from(shade_before < BLACK);
        self.black_after += usize::from(shade_after < BLACK);
        self.materials += 1;
        self.dark_before += usize::from(before);
        self.dark_after += usize::from(after);
        self.lightened += usize::from(before && !after);
        self.darkened += usize::from(after && !before);
        if !material.layers.is_empty() {
            self.layered += 1;
            self.layered_dark_before += usize::from(before);
            self.layered_dark_after += usize::from(after);
            self.layers_resolved +=
                usize::from(material.layers.iter().all(|layer| layer.surface.is_some()));
        }
    }
}

fn census(assembly: &Assembly) -> Census {
    let mut seen = BTreeSet::new();
    let mut found = Census::default();
    for part in assembly.parts() {
        let (Some(model), Some(library)) = (part.model(), part.materials().library()) else {
            continue;
        };
        found
            .unresolved
            .extend(library.unresolved_layers.iter().cloned());
        let Some(lod) = model.lods.first() else {
            continue;
        };
        let ids: BTreeSet<u16> = lod
            .meshes
            .iter()
            .flat_map(|mesh| &mesh.submeshes)
            .map(|submesh| submesh.material_id)
            .collect();
        for id in ids {
            let position = if library.single { 0 } else { usize::from(id) };
            let Some(material) = library.materials.get(position) else {
                continue;
            };
            if material.hidden() || material.shader.eq_ignore_ascii_case("MeshDecal") {
                continue;
            }
            let key = (
                std::ptr::from_ref(library) as usize,
                part.palette()
                    .map_or(0, |palette| std::ptr::from_ref(palette) as usize),
                position,
            );
            if !seen.insert(key) {
                continue;
            }
            let tinted = part.palette().and_then(|palette| palette.apply(material));
            found.material(tinted.as_ref().unwrap_or(material));
        }
    }
    found
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_layers_lighten_the_dark_materials_of_the_ships() {
    media_fixtures::run(verify);
}

fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let index = Index::new(Arc::new(Entities::new(forge)), files)?;
        let models = ArchiveModels::new(archive);
        let palettes = ForgePalettes::new(forge);
        let mut report = RunReport::default();
        let cases = FAMILIES.into_iter().enumerate();
        for (ordinal, family) in cases {
            let case = Case::from_family(
                "layered_material",
                family,
                Requirements {
                    geometry_lod: Some(0),
                    item_size: None,
                    requires_interior: false,
                    requires_audio: false,
                    docked: DockedVehicles::Expand,
                },
            );
            report.run(format!("layered_{ordinal}"), || {
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
                let found = census(&assembly);
                measurements(&found);
                media_fixtures::require(found.lightened > 0, Check::LayerEffect)?;
                Ok(CaseOutcome::Completed(()))
            });
        }
        media_fixtures::finish(&report)
    })
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate material-effect measurements are reported"
)]
fn measurements(found: &Census) {
    println!(
        "media layer totals: materials={} dark_before={} dark_after={} layered={} layered_dark_before={} layered_dark_after={} layers_resolved={} unresolved_files={} black_before={} black_after={} lightened={} darkened={}",
        found.materials,
        found.dark_before,
        found.dark_after,
        found.layered,
        found.layered_dark_before,
        found.layered_dark_after,
        found.layers_resolved,
        found.unresolved.len(),
        found.black_before,
        found.black_after,
        found.lightened,
        found.darkened
    );
}
