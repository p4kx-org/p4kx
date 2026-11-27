// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Representative Interior cases preserve placement and source-integrity checks.

use std::sync::Arc;
use std::time::Instant;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, Entities, EntityInterior, InteriorOptions, IssueKind,
};
use p4kx_extraction::Archive;
use p4kx_gltf::GlbOptions;
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{
    self, Case, Check, DockedVehicles, Family, Index, Prepared, Requirements,
};
use crate::real_entity_interior;

struct InteriorCase {
    label: &'static str,
    family: Family,
    least_containers: usize,
    least_models: usize,
}

const CASES: [InteriorCase; 5] = [
    InteriorCase {
        label: "small_interior",
        family: Family::Cutlass,
        least_containers: 1,
        least_models: 5,
    },
    InteriorCase {
        label: "medium_interior",
        family: Family::Constellation,
        least_containers: 2,
        least_models: 50,
    },
    InteriorCase {
        label: "freighter_interior",
        family: Family::Freelancer,
        least_containers: 1,
        least_models: 20,
    },
    InteriorCase {
        label: "capital_interior",
        family: Family::Idris,
        least_containers: 1,
        least_models: 100,
    },
    InteriorCase {
        label: "corvette_interior",
        family: Family::Polaris,
        least_containers: 1,
        least_models: 100,
    },
];

const MAX_UNRESOLVED_SHARE_PERCENT: usize = 10;
const MAX_OUTSIDE_SHARE_PERCENT: usize = 1;

#[derive(Debug, Clone, Copy)]
enum Verification {
    Geometry,
    Relations,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn ships_with_interiors_write_valid_glbs_with_rooms_inside_the_hull() {
    media_fixtures::run(|| verify(Verification::Geometry));
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_interiors_of_the_ships_have_no_cycles_and_no_missing_containers() {
    media_fixtures::run(|| verify(Verification::Relations));
}

fn verify(verification: Verification) -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let index = Index::new(Arc::new(Entities::new(forge)), files)?;
        let mut report = RunReport::default();
        for case in &CASES {
            let fixture = Case::from_family(
                case.label,
                case.family,
                Requirements {
                    geometry_lod: Some(0),
                    item_size: None,
                    requires_interior: true,
                    requires_audio: false,
                    docked: DockedVehicles::Stop,
                },
            );
            report.run(case.label, || {
                let Some(choice) = index.choose(archive, &fixture)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&fixture, &choice);
                let built = build(archive, &choice.capabilities)?;
                match verification {
                    Verification::Geometry => geometry(case, &built)?,
                    Verification::Relations => relations(&built)?,
                }
                Ok(CaseOutcome::Completed(()))
            });
        }
        media_fixtures::finish(&report)
    })
}

struct Built {
    assembly: Assembly,
    bytes: usize,
    nodes: usize,
    milliseconds: u128,
}

fn build(archive: &Archive, prepared: &Prepared) -> media_fixtures::Result<Built> {
    let started = Instant::now();
    let tree = prepared.node()?;
    let models = ArchiveModels::new(archive);
    let assembly = Assembly::build(tree, &models, &AssemblyOptions::default())?;
    let interior = EntityInterior::build(archive, &assembly, tree, &InteriorOptions::default())?;
    let assembly = assembly.with_interior(interior);
    let mut glb = Vec::new();
    assembly.write_glb(&GlbOptions::default(), &mut glb)?;
    let checked = p4kx_testkit::check_glb(&glb)?;
    Ok(Built {
        assembly,
        bytes: glb.len(),
        nodes: checked.document.nodes().count(),
        milliseconds: started.elapsed().as_millis(),
    })
}

fn interior(built: &Built) -> media_fixtures::Result<&EntityInterior> {
    built
        .assembly
        .interior()
        .ok_or_else(|| Check::Containers.into())
}

fn geometry(case: &InteriorCase, built: &Built) -> media_fixtures::Result<()> {
    let interior = interior(built)?;
    let (placed, outside) =
        real_entity_interior::placement_against_hull(&built.assembly, interior)?;
    let unresolved = interior.model_parts() - interior.modelled_parts();
    measurements(case.label, built, placed, outside, unresolved)?;
    media_fixtures::require(
        interior.containers().len() >= case.least_containers,
        Check::Containers,
    )?;
    media_fixtures::require(placed >= case.least_models, Check::PlacedModels)?;
    media_fixtures::require(
        outside * 100 <= placed * MAX_OUTSIDE_SHARE_PERCENT,
        Check::OutsideShare,
    )?;
    media_fixtures::require(
        unresolved * 100 <= interior.model_parts() * MAX_UNRESOLVED_SHARE_PERCENT,
        Check::UnresolvedShare,
    )
}

fn relations(built: &Built) -> media_fixtures::Result<()> {
    let invalid = interior(built)?.issues().iter().any(|issue| {
        matches!(
            issue.kind(),
            IssueKind::ContainerCycle { .. }
                | IssueKind::ContainerMissing { .. }
                | IssueKind::ContainerUnreadable { .. }
                | IssueKind::InvalidOffset { .. }
                | IssueKind::TooManyContainers { .. }
                | IssueKind::TooManyParts { .. }
                | IssueKind::TooManyVertices { .. }
        )
    });
    media_fixtures::require(!invalid, Check::InteriorIssues)
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate Interior and conversion counts are reported"
)]
fn measurements(
    label: &str,
    built: &Built,
    placed: usize,
    outside: usize,
    unresolved: usize,
) -> media_fixtures::Result<()> {
    let interior = interior(built)?;
    println!(
        "media case {label}: duration_ms={} bytes={} nodes={} containers={} parts={} models={} unresolved={} lights={} placed={} outside={}",
        built.milliseconds,
        built.bytes,
        built.nodes,
        interior.containers().len(),
        interior.parts(),
        interior.model_parts(),
        unresolved,
        interior.lights(),
        placed,
        outside
    );
    Ok(())
}
