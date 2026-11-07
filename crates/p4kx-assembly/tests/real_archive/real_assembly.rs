// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Representative assemblies retain GLB, model-share and skipped-part checks.

use std::sync::Arc;
use std::time::Instant;

use p4kx_assembly::{ArchiveModels, Assembly, AssemblyOptions, Entities, Placement};
use p4kx_gltf::GlbOptions;
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{self, Case, Check, DockedVehicles, Family, Index, Requirements};

const FAMILIES: [Family; 5] = [
    Family::Gladius,
    Family::Avenger,
    Family::Hornet,
    Family::Constellation,
    Family::Cyclone,
];
const LOD: u32 = 2;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn documented_ships_assemble_into_valid_glbs_with_few_skipped_parts() {
    media_fixtures::run(verify);
}

fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let index = Index::new(Arc::new(Entities::new(forge)), files)?;
        let models = ArchiveModels::new(archive);
        let mut report = RunReport::default();
        let cases = FAMILIES.into_iter().enumerate();
        for (ordinal, family) in cases {
            let case = Case::from_family(
                "basic_assembly",
                family,
                Requirements {
                    geometry_lod: Some(LOD),
                    item_size: None,
                    requires_interior: false,
                    requires_audio: false,
                    docked: DockedVehicles::Expand,
                },
            );
            report.run(format!("assembly_{ordinal}"), || {
                let Some(choice) = index.choose(archive, &case)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&case, &choice);
                let started = Instant::now();
                let assembly = Assembly::build(
                    choice.capabilities.node()?,
                    &models,
                    &AssemblyOptions::default().with_lod(LOD),
                )?;
                let mut bytes = Vec::new();
                let summary =
                    assembly.write_glb(&GlbOptions::default().with_lod(LOD), &mut bytes)?;
                p4kx_testkit::check_glb(&bytes)?;
                let skipped = assembly
                    .issues()
                    .iter()
                    .filter(|issue| issue.kind().skips())
                    .count();
                measurements(&assembly, summary.bytes, skipped, started);
                media_fixtures::require(assembly.parts().len() > 20, Check::MinimumParts)?;
                media_fixtures::require(
                    assembly.modelled_parts() * 3 >= assembly.parts().len(),
                    Check::ModelledShare,
                )?;
                media_fixtures::require(
                    skipped * 10 <= assembly.parts().len() + skipped,
                    Check::SkippedShare,
                )?;
                media_fixtures::require(
                    summary.parts >= assembly.modelled_parts().saturating_sub(1),
                    Check::ExportedParts,
                )?;
                Ok(CaseOutcome::Completed(()))
            });
        }
        media_fixtures::finish(&report)
    })
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate assembly measurements are reported"
)]
fn measurements(assembly: &Assembly, bytes: u64, skipped: usize, started: Instant) {
    let placed = assembly
        .parts()
        .iter()
        .filter(|part| matches!(part.placement(), Placement::Helper(_) | Placement::PortNode))
        .count();
    println!(
        "media assembly totals: parts={} modelled={} placed={} skipped={} bytes={} duration_ms={}",
        assembly.parts().len(),
        assembly.modelled_parts(),
        placed,
        skipped,
        bytes,
        started.elapsed().as_millis()
    );
}
