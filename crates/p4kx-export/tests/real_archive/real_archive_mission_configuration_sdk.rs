// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::mem;

use p4kx_export::catalog_queries::mission_properties::{
    DeclarationBinding, FlowSelection, MissionFlowOwnerRef, MissionPropertyOwnerSnapshot,
    MissionPropertyVisitSummary, SourceFieldRef, SourceValueRef, TemplateSource,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DatasetAvailability,
    ProvenanceMode,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};

use crate::real_archive_catalog_table_parity::{
    MissionDisposition, StreamedMissionDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_scoped_equipment_sdk::equipment_input::with_archive;
use crate::real_archive_verification::print_line;

#[path = "real_archive_mission_configuration_scope.rs"]
mod scope;

use scope::OriginalScope;

const PRODUCER: &str = "P4KX_MISSION_CONFIGURATION_CATALOG_PRODUCER_REVISION";
const DISPOSITIONS: &str = "P4KX_MISSION_CONFIGURATION_SDK_EXPECTED_OWNERS";

#[test]
#[ignore = "standalone installed mission property and flow visit with exact Build and owner scope"]
fn installed_mission_configuration_cold_visit() -> TestResult {
    let expected = parse_dispositions(&env::var(DISPOSITIONS)?)?;
    with_archive(|archive| read_all(archive, expected, |_| Ok(())).map(|_| ()))
}

#[test]
#[ignore = "same Build installed Archive and exact completed Normal producer"]
fn installed_mission_configuration_matches_one_complete_catalog() -> TestResult {
    let producer = env::var(PRODUCER)?;
    let expected = parse_dispositions(&env::var(DISPOSITIONS)?)?;
    with_bound_measurement_from_producer(&producer, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut evidence = StreamedMissionDigests::new(scratch.path());
        let (summary, original) = read_all(archive, expected, |snapshot| {
            evidence.retain_owner(snapshot)
        })?;
        let counts = measurement.verify_streamed_mission_configuration(
            original.availability(),
            original.dispositions,
            evidence,
        )?;
        let [properties, flows, templates, brokers] = counts.rows;
        let [expected_templates, expected_brokers] = expected;
        if properties != summary.property_count
            || flows != summary.flow_count
            || templates != expected_templates.owners
            || brokers != expected_brokers.owners
        {
            return Err(
                "retained mission rows or owner arenas disagree with the terminal visitor counts"
                    .into(),
            );
        }
        print_line(&format!(
            "complete mission configuration parity: properties={properties} flows={flows} templates={templates} brokers={brokers} sourceUses={} physicalSources={}",
            counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

fn read_all(
    archive: &Archive,
    expected: [MissionDisposition; 2],
    mut inspect: impl FnMut(&MissionPropertyOwnerSnapshot<'_>) -> TestResult,
) -> TestResult<(MissionPropertyVisitSummary, OriginalScope)> {
    let input = DataForgeEntry::read(archive).map_err(|_| "read the bound DataForge Entry")?;
    let original = OriginalScope::read(&input, expected)?;
    let mut census = Census::default();
    let session = CatalogQuerySession::from_entry(
        archive,
        &input,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?
    .with_provenance_mode(ProvenanceMode::Full);
    let summary = session.visit_mission_properties(
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            census.observe(&original, snapshot)?;
            inspect(snapshot)
        },
    )?;
    census.finish(archive, &original, &summary)?;
    let [templates, brokers] = original.dispositions;
    print_line(&format!(
        "mission configuration SDK: availability={:?} templates={} templateLeftovers={} brokers={} brokerLeftovers={} properties={} flows={} sourceUseOccurrences={} physicalSourceOccurrences={}",
        summary.availability,
        templates.owners,
        templates.skipped,
        brokers.owners,
        brokers.skipped,
        census.properties,
        census.flows,
        census.uses,
        census.sources,
    ));
    Ok((summary, original))
}

#[derive(Default)]
struct Census {
    owners: usize,
    properties: usize,
    flows: usize,
    uses: usize,
    sources: usize,
}

impl Census {
    fn observe(
        &mut self,
        original: &OriginalScope,
        snapshot: &MissionPropertyOwnerSnapshot<'_>,
    ) -> TestResult {
        original.check_owner(self.owners, &snapshot.source.owner)?;
        if !snapshot.fields_complete || snapshot.rows.is_empty() || snapshot.flows.is_empty() {
            return Err(format!(
                "mission owner ordinal {} does not have both original declaration surveys",
                self.owners
            )
            .into());
        }
        check_links(snapshot)?;
        let inventory = &snapshot.source_inventory;
        if inventory.use_count() == 0
            || !inventory
                .iter_sources()
                .any(|source| source.id() == snapshot.source.owner.physical().input_ref())
            || inventory
                .iter_uses()
                .any(|used| matches!(used.location(), SourceLocation::LocalizationLookup { .. }))
        {
            return Err(
                "mission authored declarations lack their original nonlocalized source inventory"
                    .into(),
            );
        }
        self.owners += 1;
        self.properties += snapshot.rows.len();
        self.flows += snapshot.flows.len();
        self.uses += inventory.use_count();
        self.sources += inventory.source_count();
        Ok(())
    }

    fn finish(
        &self,
        archive: &Archive,
        original: &OriginalScope,
        summary: &MissionPropertyVisitSummary,
    ) -> TestResult {
        if archive.build().is_none() || summary.build.as_ref() != archive.build() {
            return Err("the mission visit differs from the bound installed Build".into());
        }
        let [expected, _, _, _] = original.availability();
        if mem::discriminant(&summary.availability) != mem::discriminant(&expected)
            || matches!(expected, DatasetAvailability::Unavailable)
            || (matches!(expected, DatasetAvailability::Absent) && self.owners != 0)
            || self.owners != original.owners()
            || (
                summary.owner_count,
                summary.property_count,
                summary.flow_count,
            ) != (self.owners, self.properties, self.flows)
        {
            return Err(
                "mission visitor owner counts, row counts or declaration availability differ"
                    .into(),
            );
        }
        let coverage = &summary.coverage;
        let expected_skipped: usize = original
            .dispositions
            .iter()
            .map(|family| family.skipped)
            .sum();
        if !coverage.unreadable_headers.is_empty()
            || !coverage.unreadable_bodies.is_empty()
            || !coverage.incomplete_fields.is_empty()
            || coverage.skipped_leftovers != expected_skipped
        {
            return Err(format!(
                "incomplete mission visit: unreadableHeaders={} unreadableBodies={} incompleteFields={} skipped={} expectedSkipped={expected_skipped}",
                coverage.unreadable_headers.len(), coverage.unreadable_bodies.len(),
                coverage.incomplete_fields.len(), coverage.skipped_leftovers,
            ).into());
        }
        Ok(())
    }
}

fn check_links(snapshot: &MissionPropertyOwnerSnapshot<'_>) -> TestResult {
    let arena = &snapshot.source.arena;
    for row in &snapshot.rows {
        require_owner(row.owner(), &snapshot.source.owner)?;
        require_declaration(arena, row.declaration(), row.declaration_index())?;
        for value in row
            .owner_objective()
            .into_iter()
            .chain(row.properties().iter().copied())
        {
            require_value(arena, value)?;
        }
    }
    for row in &snapshot.flows {
        require_owner(row.owner(), &snapshot.source.owner)?;
        require_declaration(arena, row.declaration(), row.declaration_index())?;
        if let FlowSelection::Selected { value, .. } = row.selection() {
            require_value(arena, *value)?;
        }
        for &trigger in row.triggers() {
            require_value(arena, trigger)?;
        }
        for &field in row.referenced_fields() {
            require_field(arena, field)?;
        }
    }
    Ok(())
}

fn require_owner(owner: &MissionFlowOwnerRef, source: &MissionFlowOwnerRef) -> TestResult {
    let actual = owner.physical();
    let expected = source.physical();
    if mem::discriminant(owner) != mem::discriminant(source)
        || actual.input_ref() != expected.input_ref()
        || actual.record_index() != expected.record_index()
        || actual.record_guid() != expected.record_guid()
    {
        return Err("a mission configuration row refers to a foreign physical owner".into());
    }
    Ok(())
}

fn require_declaration(
    arena: &TemplateSource<'_>,
    binding: &DeclarationBinding,
    index: Option<u64>,
) -> TestResult {
    match (binding, index) {
        (DeclarationBinding::Present { field, .. }, Some(_)) => require_field(arena, *field),
        (DeclarationBinding::Absent { origin, .. }, None)
            if !origin.source_uses_refs().is_empty() =>
        {
            Ok(())
        }
        _ => Err("unknown or unsupported original declaration survey".into()),
    }
}

fn require_field(arena: &TemplateSource<'_>, field: SourceFieldRef) -> TestResult {
    arena
        .field(field)
        .ok_or("unresolved original mission field link")?;
    Ok(())
}

fn require_value(arena: &TemplateSource<'_>, value: SourceValueRef) -> TestResult {
    arena
        .value(value)
        .ok_or("unresolved original mission value link")?;
    Ok(())
}

pub(super) fn parse_dispositions(encoded: &str) -> TestResult<[MissionDisposition; 2]> {
    let (templates, brokers) = encoded
        .split_once(',')
        .ok_or("expected Template and Broker counts")?;
    Ok([parse_disposition(templates)?, parse_disposition(brokers)?])
}

fn parse_disposition(kind: &str) -> TestResult<MissionDisposition> {
    let (owners, skipped) = kind
        .split_once(':')
        .ok_or("expected owner and skipped counts")?;
    let parse = |count: &str| -> TestResult<usize> {
        if count.is_empty() || !count.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("invalid mission disposition count".into());
        }
        Ok(count.parse()?)
    };
    Ok(MissionDisposition {
        owners: parse(owners)?,
        skipped: parse(skipped)?,
    })
}

#[test]
fn mission_configuration_requires_two_exact_family_dispositions() -> TestResult {
    let [templates, brokers] = parse_dispositions("3:0,5:2")?;
    assert_eq!(
        (templates.owners, templates.skipped),
        (3, 0),
        "first pair belongs to Templates"
    );
    assert_eq!(
        (brokers.owners, brokers.skipped),
        (5, 2),
        "second pair belongs to Brokers"
    );
    for invalid in ["", "5:0", "5:0,4", "5:0,4:0,1:0", "5:-1,4:1", "+5:0,4:1"] {
        assert!(
            parse_dispositions(invalid).is_err(),
            "reject invalid disposition input"
        );
    }
    Ok(())
}
