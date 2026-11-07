// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Generic ship families retain character-definition and helper-completeness checks.

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveModels, Assembly, AssemblyOptions, Entities, EntityNode, IssueKind, Part, PortSource,
};
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{self, Case, Check, DockedVehicles, Family, Index, Requirements};

const FAMILIES: [Family; 10] = [
    Family::Gladius,
    Family::Avenger,
    Family::Hornet,
    Family::Constellation,
    Family::Cyclone,
    Family::RacerM50,
    Family::Racer350,
    Family::Gladiator,
    Family::Cutter,
    Family::Archimedes,
];

#[derive(Default)]
struct Census {
    ships: usize,
    cdf_parts: usize,
    cdf_modelled: usize,
    cdf_unreadable: usize,
    attachment_notes: usize,
    empty_ports: usize,
    helper_without_parent_model: usize,
}

impl Census {
    const fn add(&mut self, other: &Self) {
        self.ships += other.ships;
        self.cdf_parts += other.cdf_parts;
        self.cdf_modelled += other.cdf_modelled;
        self.cdf_unreadable += other.cdf_unreadable;
        self.attachment_notes += other.attachment_notes;
        self.empty_ports += other.empty_ports;
        self.helper_without_parent_model += other.helper_without_parent_model;
    }

    fn verify(&self) -> media_fixtures::Result<()> {
        media_fixtures::require(self.cdf_unreadable == 0, Check::CdfUnreadable)?;
        media_fixtures::require(self.cdf_modelled == self.cdf_parts, Check::CdfModelled)?;
        media_fixtures::require(self.helper_without_parent_model == 0, Check::HelperIssues)
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn character_definitions_and_vehicle_only_ports_of_documented_ships() {
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
                "character_definitions",
                family,
                Requirements {
                    geometry_lod: Some(0),
                    item_size: None,
                    requires_interior: false,
                    requires_audio: false,
                    docked: DockedVehicles::Expand,
                },
            );
            report.run(format!("character_definitions_{ordinal}"), || {
                let Some(choice) = index.choose(archive, &case)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&case, &choice);
                let tree = choice.capabilities.node()?;
                let assembly = Assembly::build(tree, &models, &AssemblyOptions::default())?;
                let census = tally(tree, &assembly);
                measurements(&census);
                census.verify()?;
                Ok(CaseOutcome::Completed(census))
            });
        }
        let mut total = Census::default();
        for case in report.cases() {
            if let Ok(CaseOutcome::Completed(found)) = &case.outcome {
                total.add(found);
            }
        }
        measurements(&total);
        let coverage = media_fixtures::finish(&report);
        media_fixtures::require(total.ships >= 5, Check::MinimumParts)?;
        media_fixtures::require(total.cdf_parts > 0, Check::CdfParts)?;
        total.verify()?;
        coverage
    })
}

fn tally(tree: &EntityNode, assembly: &Assembly) -> Census {
    let mut census = Census {
        ships: 1,
        ..Census::default()
    };
    census.empty_ports = tree
        .walk()
        .iter()
        .filter(|(_, port)| port.source() == PortSource::Empty)
        .count();
    for part in assembly.parts() {
        if part
            .geometry()
            .is_some_and(|geometry| geometry.to_ascii_lowercase().ends_with(".cdf"))
        {
            census.cdf_parts += 1;
            census.cdf_modelled += usize::from(part.model().is_some());
        }
    }
    for issue in assembly.issues() {
        match issue.kind() {
            IssueKind::GeometryUnreadable { path, .. }
                if path.to_ascii_lowercase().ends_with(".cdf") =>
            {
                census.cdf_unreadable += 1;
            }
            IssueKind::AttachmentsLeftOut { .. } => census.attachment_notes += 1,
            IssueKind::HelperNotFound { .. } => {
                let parent = assembly
                    .part_at(issue.path())
                    .and_then(Part::parent)
                    .and_then(|index| assembly.parts().get(index));
                census.helper_without_parent_model +=
                    usize::from(parent.is_some_and(|part| part.model().is_none()));
            }
            _ => {}
        }
    }
    census
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate character-definition and helper counts are reported"
)]
fn measurements(census: &Census) {
    println!(
        "media definition totals: fixtures={} cdf_parts={} cdf_modelled={} cdf_unreadable={} attachment_notes={} empty_ports={} helper_without_parent_model={}",
        census.ships,
        census.cdf_parts,
        census.cdf_modelled,
        census.cdf_unreadable,
        census.attachment_notes,
        census.empty_ports,
        census.helper_without_parent_model
    );
}
