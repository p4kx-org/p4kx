// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Representative media cases preserve source capabilities and existing geometry thresholds.

mod armour;
mod measurement;
mod verify;

use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_gltf::MaterialMode;
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{self, Case, DockedVehicles, Family, Index, Requirements};

const TEXTURE_MIP: u32 = 3;

struct ModelCase {
    label: &'static str,
    family: Family,
    lod: u32,
    least_parts: usize,
    shares_meshes: bool,
    item_size: Option<u32>,
}

const CASES: [ModelCase; 5] = [
    ModelCase {
        label: "small_ship",
        family: Family::Gladius,
        lod: 0,
        least_parts: 20,
        shares_meshes: true,
        item_size: None,
    },
    ModelCase {
        label: "medium_ship",
        family: Family::Constellation,
        lod: 0,
        least_parts: 40,
        shares_meshes: true,
        item_size: None,
    },
    ModelCase {
        label: "capital_ship",
        family: Family::Idris,
        lod: 2,
        least_parts: 100,
        shares_meshes: true,
        item_size: None,
    },
    ModelCase {
        label: "ground_vehicle",
        family: Family::Cyclone,
        lod: 0,
        least_parts: 10,
        shares_meshes: false,
        item_size: None,
    },
    ModelCase {
        label: "weapon",
        family: Family::SizeThreeWeapon,
        lod: 0,
        least_parts: 1,
        shares_meshes: false,
        item_size: Some(3),
    },
];

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn documented_entities_assemble_into_valid_complete_glbs() {
    media_fixtures::run(verify);
}

fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let index = Index::new(Arc::new(Entities::new(forge)), files)?;
        let directory = tempfile::tempdir()?;
        let mut report = RunReport::default();
        for case in &CASES {
            let fixture = Case::from_family(
                case.label,
                case.family,
                Requirements {
                    geometry_lod: Some(case.lod),
                    item_size: case.item_size,
                    requires_interior: false,
                    requires_audio: false,
                    docked: DockedVehicles::Stop,
                },
            );
            for mode in [MaterialMode::Colours, MaterialMode::Textures] {
                report.run(format!("{}_{mode:?}", case.label), || {
                    let Some(choice) = index.choose(archive, &fixture)? else {
                        return Ok(media_fixtures::absent());
                    };
                    media_fixtures::selected(&fixture, &choice);
                    let path = directory
                        .path()
                        .join(format!("{}_{mode:?}.glb", case.label));
                    let measured = measurement::measure(
                        archive,
                        forge,
                        &choice.capabilities,
                        case.lod,
                        mode,
                        &path,
                    )?;
                    measurement::report(case.label, mode, &measured);
                    verify::verify(case, mode, &measured)?;
                    Ok(CaseOutcome::Completed(()))
                });
            }
        }
        media_fixtures::finish(&report)
    })
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn worn_armour_is_real_geometry_and_not_boxes() {
    media_fixtures::run(armour::verify);
}
