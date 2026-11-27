// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_archive::{Archive, SourceEncryption};
use p4kx_dataforge::DataForge;
use p4kx_extraction::{SourceLocation, SourceUseRole};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry};

use crate::{
    ArchiveFiles, Entities, LoadoutDocumentKind, LoadoutFileObservation, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutResolver, VehicleAttributeEvidence, VehicleTotalFieldUse,
    VehicleTotalMetric, VehicleXmlSite,
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const BASE: &str = "Scripts/Entities/Vehicles/Implementations/Xml/totals.xml";
const PATCH: &str = "Scripts/Entities/Vehicles/Implementations/Xml/Modifications/totals.xml";
const DEFINITION: &[u8] = br#"
<Vehicle>
    <Parts>
        <Part id="Base" mass="2.12555" damageMax="10"/>
        <Part id="Patched" mass="2" damageMax="20"/>
        <Part id="Hidden" mass="3" damageMax="999"><ItemPort flags="invisible"/></Part>
        <Part id="Elem" mass="4" damageMax="6"/>
        <Part id="Empty"/>
        <Part id="Invalid" mass="garbage" damageMax="0"/>
        <Part id="Skipped" skipPart="1" mass="1000" damageMax="1000"/>
    </Parts>
    <Modifications>
        <Modification name="Observed" patchFile="Modifications/totals.xml">
            <Elems><Elem idRef="Elem" name="mass" value="7"/></Elems>
        </Modification>
    </Modifications>
</Vehicle>"#;

#[derive(Default)]
struct Observer {
    parsed: Mutex<Vec<LoadoutFileObservation>>,
}

impl LoadoutReadObserver for Observer {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        if let LoadoutReadEvent::FileProjection { observation, .. } = event {
            self.parsed
                .lock()
                .expect("vehicle projection witness")
                .push(observation.clone());
        }
        ControlFlow::Continue(())
    }

    fn observes_vehicle_totals(&self) -> bool {
        true
    }
}

#[test]
fn effective_vehicle_total_fields_keep_base_patch_and_elem_receipts() -> TestResult {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let ship = forge.record(0).ok_or("synthetic vehicle")?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    for pass in ["initial parse", "resolver cache reuse"] {
        let node = resolver.resolve(&ship)?;
        let vehicle = node.vehicle().ok_or("vehicle with effective parts")?;
        let mut uses = Vec::new();
        let totals = vehicle.totals_with_field_uses(|site| uses.push(site));
        assert_eq!(
            totals,
            vehicle.totals(),
            "{pass} preserves the interpreted numbers"
        );
        assert!(
            (totals.mass() - 17.12555).abs() < f64::EPSILON,
            "{pass} sums effective masses"
        );
        assert!(
            (totals.health() - 51.0).abs() < f64::EPSILON,
            "{pass} excludes hidden and skipped damage"
        );
        assert_eq!(
            totals.mass_parts(),
            4,
            "{pass} preserves four consumed mass values"
        );
        assert_eq!(
            totals.health_parts(),
            3,
            "{pass} preserves three consumed damage values"
        );
        verify_assignments(&uses)?;
        verify_selectors(&uses);
        let parsed = observer
            .parsed
            .lock()
            .map_err(|_| "vehicle projection witness")?;
        assert_eq!(parsed.len(), 2, "{pass} keeps one actual parse per XML");
        verify_actual_sites(&uses, &parsed);
    }
    Ok(())
}

#[test]
fn provenance_disabled_remains_unknown_for_the_same_effective_values() -> TestResult {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let ship = forge.record(0).ok_or("synthetic vehicle")?;
    let mut resolver =
        LoadoutResolver::with_entities(Arc::new(Entities::new(&forge))).with_files(&files);
    let node = resolver.resolve(&ship)?;
    let vehicle = node.vehicle().ok_or("vehicle with effective parts")?;
    let mut attempts = 0;
    let totals = vehicle.totals_with_field_uses(|field| {
        assert!(
            field.evidence().is_none(),
            "ordinary reads do not fabricate XML sites"
        );
        assert_eq!(
            field.role(),
            SourceUseRole::Attempted,
            "unobserved field use stays unverified"
        );
        attempts += 1;
    });
    assert!(attempts > 0, "the scoped values were still consulted");
    assert_eq!(
        totals,
        vehicle.totals(),
        "observation leaves the interpreted result unchanged"
    );
    Ok(())
}

fn verify_assignments(uses: &[VehicleTotalFieldUse<'_>]) -> TestResult {
    let mass = conversions(uses, VehicleTotalMetric::Mass)?;
    let health = conversions(uses, VehicleTotalMetric::HullHealth)?;
    assert_eq!(
        mass.iter()
            .map(|datum| field(datum.original()))
            .collect::<Vec<_>>(),
        [Some("mass"), Some("MAsS"), Some("mass"), Some("value")],
        "base, exact patch spelling and Elem value stay in original effective part order",
    );
    assert_eq!(
        health.len(),
        3,
        "hidden port damage does not enter hull health"
    );
    let patch = mass[1];
    assert_eq!(
        encryption(patch.original())?,
        SourceEncryption::Encrypted,
        "actual patch entry supplies the value"
    );
    assert_eq!(
        patch
            .selectors()
            .iter()
            .map(encryption)
            .collect::<TestResult<Vec<_>>>()?,
        [SourceEncryption::Encrypted, SourceEncryption::Unencrypted],
        "the original donor and target IDs supply the match",
    );
    assert!(
        patch
            .selectors()
            .iter()
            .all(|site| field(site) == Some("id")),
        "no name-based ID reconstruction"
    );
    let elem = mass[3];
    assert_eq!(
        encryption(elem.original())?,
        SourceEncryption::Unencrypted,
        "Elem resides in implementation XML"
    );
    assert_eq!(
        elem.selectors().iter().map(field).collect::<Vec<_>>(),
        [Some("name"), Some("idRef"), Some("name"), Some("id")],
        "Elem assignment, selected modification and target identity are distinct sites",
    );
    Ok(())
}

fn verify_selectors(uses: &[VehicleTotalFieldUse<'_>]) {
    assert!(
        uses.iter().any(|use_| {
            use_.metric() == VehicleTotalMetric::HullHealth
                && use_.role() == SourceUseRole::Selector
                && use_.evidence().and_then(|proof| field(proof.original())) == Some("flags")
        }),
        "the original hull exclusion flag was consulted"
    );
    assert!(
        uses.iter().any(|use_| {
            use_.metric() == VehicleTotalMetric::Mass
                && use_.role() == SourceUseRole::Selector
                && use_.evidence().is_some_and(|proof| {
                    matches!(
                        proof.original().location(),
                        Some(SourceLocation::Subresource { namespace, field, .. })
                            if namespace == "textXmlElement" && field == "mass"
                    )
                })
        }),
        "a checked missing attribute is an original negative field observation"
    );
}

fn verify_actual_sites(uses: &[VehicleTotalFieldUse<'_>], parsed: &[LoadoutFileObservation]) {
    for record in uses.iter().filter_map(|use_| use_.evidence()) {
        for source in std::iter::once(record.original()).chain(record.selectors()) {
            assert!(
                parsed
                    .iter()
                    .any(|original| source.file().is_same_operation(original)),
                "each field receipt reuses its one original physical parse operation",
            );
            assert!(
                matches!(
                    source.file().kind(),
                    LoadoutDocumentKind::VehicleImplementation | LoadoutDocumentKind::VehiclePatch
                ),
                "unrelated encrypted files cannot enter the field-use set"
            );
        }
    }
}

fn conversions<'vehicle>(
    uses: &[VehicleTotalFieldUse<'vehicle>],
    metric: VehicleTotalMetric,
) -> TestResult<Vec<&'vehicle VehicleAttributeEvidence>> {
    uses.iter()
        .filter(|use_| use_.metric() == metric && use_.role() == SourceUseRole::Conversion)
        .map(|use_| {
            use_.evidence()
                .ok_or_else(|| "consumed exact XML field".into())
        })
        .collect()
}

fn field(site: &VehicleXmlSite) -> Option<&str> {
    match site.location()? {
        SourceLocation::Subresource { field, .. } => Some(field),
        _ => None,
    }
}

fn encryption(site: &VehicleXmlSite) -> TestResult<SourceEncryption> {
    Ok(site
        .file()
        .source()
        .entry_origin()
        .ok_or("archive-bound vehicle XML")?
        .encryption())
}

fn archive() -> TestResult<(tempfile::TempDir, Archive)> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(format!("Data/{BASE}"), DEFINITION))
        .entry(
            TestEntry::stored(
                format!("Data/{PATCH}"),
                br#"<Patch><Part id="Patched" MAsS="5" damageMax="35"/></Patch>"#.as_slice(),
            )
            .encrypted(),
        )
        .entry(TestEntry::stored("Data/unrelated.xml", b"<Unrelated/>".as_slice()).encrypted())
        .write_to(&archive)?;
    Ok((
        directory,
        Archive::open_with_key(archive, Some(p4kx_testkit::test_key()))?,
    ))
}

fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(71, "TotalsFixture", "synthetic/totals.xml")
                .vehicle(BASE, "Observed")
                .record(),
        )
        .build()
}
