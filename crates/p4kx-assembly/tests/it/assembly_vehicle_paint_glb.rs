// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, CreatureAnimations, LoadoutChanges};
use p4kx_extraction::{Archive, GlbMeshOrigins, GlbOptions};
use p4kx_testkit::{fitted_paint_vehicle_entries, test_key};

use super::{BASE_MATERIAL, TestResult, assemble_archive, fixture};

#[test]
fn observed_painted_glb_matches_the_existing_writer_bytes() -> TestResult {
    let fixture = fixture(fitted_paint_vehicle_entries())?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let models = ArchiveModels::new(&archive);
    let textures = models.textures();
    let options = GlbOptions::default();
    for entity in ["TEST_Hull_Red", "TEST_Hull_Blue"] {
        let assembly = assemble_archive(
            &archive,
            entity,
            &LoadoutChanges::new(),
            Some(BASE_MATERIAL),
        )?;
        let animated = CreatureAnimations::default();
        let paint = assembly
            .paint_provenance()?
            .ok_or("legacy paint provenance")?;
        let mut legacy = Vec::new();
        paint.write_glb_animated_with(&animated, &options, &textures, &mut legacy)?;

        let mut origins = GlbMeshOrigins::new();
        let paint = assembly
            .observe_glb_paint(&mut origins)?
            .ok_or("observed paint provenance")?;
        let mut observed = Vec::new();
        assembly.write_glb_animated_with_mesh_origins(
            &animated,
            &options,
            &textures,
            &mut observed,
            Some(&paint),
            &mut origins,
        )?;
        origins.finish()?;
        assert_eq!(observed, legacy, "{entity} paints and GLB bytes are stable");
    }
    Ok(())
}
