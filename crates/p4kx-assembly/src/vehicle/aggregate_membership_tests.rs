// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::DataForge;
use p4kx_extraction::{SourceLocation, SourceUseRole};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

use super::support::{self, Observer, TestResult};
use crate::{
    ArchiveFiles, Entities, LoadoutRequest, LoadoutResolver, VehicleImplementation,
    VehicleTotalMetric,
};

#[test]
fn skipped_subtree_and_known_optional_absence_keep_original_membership() -> TestResult {
    let text = br#"<Vehicle><Parts>
        <Part id="keep" mass="2" damageMax="3"/>
        <Part id="skip" skipPart="true" mass="100"><Part mass="999"/></Part>
        <Parts><Part mass="-1" damageMax="4"/></Parts>
        <Part mass="" damageMax="garbage"/>
    </Parts></Vehicle>"#;
    support::with_vehicle(text, None, "", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert_eq!(evidence.issues(), &[], "membership is completely observed");
        let totals = vehicle.totals();
        assert_eq!(totals.mass(), 1.0);
        assert_eq!(totals.health(), 7.0);
        assert_eq!(totals.mass_parts(), 2);
        assert_eq!(totals.unreadable().len(), 1);
        assert!(
            evidence
                .selectors()
                .filter_map(|site| support::base_fragment(site, text))
                .any(|value| value.contains("skipPart=\"true\""))
        );
        assert!(
            !evidence
                .selectors()
                .filter_map(|site| support::base_fragment(site, text))
                .any(|value| value.contains("mass=\"999\""))
        );
        Ok(())
    })
}

#[test]
fn cryxml_empty_parts_and_skipping_use_original_element_indices() -> TestResult {
    let root = TestElement::new("Vehicle").child(
        TestElement::new("Parts").child(
            TestElement::new("Part")
                .attribute("skipPart", "1")
                .child(TestElement::new("Part").attribute("mass", "999")),
        ),
    );
    let bytes = root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    support::with_vehicle(&bytes, None, "", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert_eq!(vehicle.totals().mass_parts(), 0);
        assert_eq!(vehicle.totals().health_parts(), 0);
        assert_eq!(evidence.issues(), &[], "CryXml membership is observed");
        assert!(evidence.selectors().all(|site| matches!(
            site.location(),
            Some(SourceLocation::Subresource { namespace, .. })
                if namespace.starts_with("cryxml")
        )));
        Ok(())
    })
}

#[test]
fn ordinary_observer_does_not_allocate_original_aggregate_scope() -> TestResult {
    support::with_vehicle(
        b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>",
        None,
        "",
        false,
        |vehicle, _| {
            assert!(vehicle.aggregate_evidence().is_none());
            assert_eq!(vehicle.totals().mass(), 2.0);
            Ok(())
        },
    )
}

#[test]
fn repaired_cryxml_text_preserves_source_identity_without_certified_coordinates() -> TestResult {
    let root = TestElement::new("Vehicle")
        .attribute("name", "REPAIR_THIS_TEXT")
        .child(TestElement::new("Parts").child(TestElement::new("Part").attribute("mass", "2")));
    let mut bytes = root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let at = bytes
        .windows(b"REPAIR_THIS_TEXT".len())
        .position(|value| value == b"REPAIR_THIS_TEXT")
        .ok_or("authored string table value")?;
    bytes[at] = 0xff;
    support::with_vehicle(&bytes, None, "", true, |vehicle, _| {
        let evidence = vehicle
            .aggregate_evidence()
            .ok_or("opt-in aggregate scope")?;
        assert!(!evidence.has_complete_coordinates());
        assert!(evidence.selectors().all(|site| site.location().is_none()));
        assert!(
            evidence
                .selectors()
                .all(|site| site.file().source().entry_origin().is_some())
        );
        assert_eq!(vehicle.totals().mass(), 2.0);
        Ok(())
    })
}

#[test]
fn stopping_the_actual_vehicle_callback_returns_no_successful_resolution() -> TestResult {
    let (_directory, archive) = support::archive(
        b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>",
        None,
    )?;
    let files = ArchiveFiles::new(&archive);
    let bytes = support::database("");
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("physical source")?;
    let observer = Arc::new(Observer::stopping());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    assert!(matches!(
        resolver.resolve(&record),
        Err(crate::LoadoutError::ObservationStopped)
    ));
    assert_eq!(
        observer
            .callers
            .lock()
            .map_err(|_| "current caller witness")?
            .len(),
        1
    );
    Ok(())
}

#[test]
fn same_guid_physical_callers_keep_their_current_definition_on_vehicle_cache_reuse() -> TestResult {
    let (_directory, archive) = support::archive(
        b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>",
        None,
    )?;
    let files = ArchiveFiles::new(&archive);
    let bytes = support::database("");
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first physical source")?;
    let second = forge.record(1).ok_or("second physical source")?;
    assert_eq!(first.guid(), second.guid());
    assert_ne!(first.index(), second.index());
    let observer = Arc::new(Observer::new(true));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone())
            .with_files(&files);
    for record in [first, second] {
        let node = resolver.resolve(&record)?;
        support::assert_original_sites(node.vehicle().ok_or("vehicle")?, &observer)?;
    }
    assert_eq!(
        observer
            .parsed
            .lock()
            .map_err(|_| "original parse witness")?
            .len(),
        1
    );
    assert_eq!(
        observer
            .read_paths
            .lock()
            .map_err(|_| "original file-read witness")?
            .len(),
        1
    );
    let callers = observer
        .callers
        .lock()
        .map_err(|_| "current caller witness")?;
    assert_eq!(callers.len(), 2);
    assert_eq!(callers[0].0, 0);
    assert_eq!(callers[1].0, 1);
    let definition = |request: &LoadoutRequest| match request {
        LoadoutRequest::Vehicle { definition, .. } => definition.value().to_owned(),
        _ => panic!("actual Vehicle caller"),
    };
    assert_eq!(definition(&callers[0].1), support::BASE);
    assert_eq!(
        definition(&callers[1].1),
        support::BASE.to_ascii_lowercase()
    );
    Ok(())
}

const CASE_VEHICLE: &[u8] = br#"<Vehicle>
    <Parts><Part id="body" mass="2" damageMax="3"/></Parts>
    <Modifications>
        <Modification name="Tune"><Elems>
            <Elem idRef="body" name="mass" value="11"/>
        </Elems></Modification>
        <Modification name="TUNE"><Elems>
            <Elem idRef="body" name="mass" value="29"/>
        </Elems></Modification>
    </Modifications>
</Vehicle>"#;

#[test]
fn exact_modification_variants_have_distinct_vehicle_cache_entries() -> TestResult {
    let (_directory, archive) = support::archive(CASE_VEHICLE, None)?;
    let files = ArchiveFiles::new(&archive);
    let bytes = support::database_with_modifications("Tune", "TUNE");
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first original caller")?;
    let second = forge.record(1).ok_or("second original caller")?;
    assert_eq!(
        first.guid(),
        second.guid(),
        "the physical callers share a nominal GUID"
    );
    assert_eq!(
        first.name()?,
        second.name()?,
        "the original class name is the same"
    );
    let mixed = (first.index(), "Tune", 11);
    let upper = (second.index(), "TUNE", 29);
    for order in [[mixed, upper, mixed, upper], [upper, mixed, upper, mixed]] {
        let observer = Arc::new(Observer::new(true));
        let mut resolver = LoadoutResolver::with_observed_entities(
            Arc::new(Entities::new(&forge)),
            observer.clone(),
        )
        .with_files(&files);
        for (index, requested, mass) in order {
            let record = forge.record(index).ok_or("actual physical caller")?;
            let node = resolver.resolve(&record)?;
            let vehicle = node.vehicle().ok_or("selected implementation")?;
            support::assert_original_sites(vehicle, &observer)?;
            assert_exact_modification(vehicle, requested, mass);
        }
        assert_cache_receipts(&observer, &order)?;
    }
    Ok(())
}

fn assert_cache_receipts(observer: &Observer, order: &[(u32, &str, u32)]) -> TestResult {
    let base = format!("Data/{}", support::BASE);
    let base_reads = observer
        .read_paths
        .lock()
        .map_err(|_| "original FileRead witness")?
        .iter()
        .filter(|path| path.eq_ignore_ascii_case(&base))
        .count();
    assert_eq!(
        base_reads, 2,
        "each exact spelling reads its original base once and reuses only its own result"
    );
    let callers = observer
        .callers
        .lock()
        .map_err(|_| "physical caller witness")?;
    assert_eq!(
        callers.len(),
        order.len(),
        "each cold and cached caller is observed"
    );
    for ((index, caller), &(physical, requested, _)) in callers.iter().zip(order) {
        let LoadoutRequest::Vehicle { modification, .. } = caller else {
            return Err("observed actual Vehicle request".into());
        };
        assert_eq!(
            *index, physical,
            "actual physical caller survives cache selection"
        );
        assert_eq!(
            modification.value(),
            requested,
            "the retained caller supplies its own exact spelling"
        );
    }
    drop(callers);
    Ok(())
}

fn assert_exact_modification(vehicle: &VehicleImplementation, name: &str, mass: u32) {
    assert_eq!(
        vehicle.modification(),
        Some(name),
        "effective exact Modification"
    );
    let wanted_selector = format!("name=\"{name}\"");
    let mut original_values = Vec::new();
    let totals = vehicle.totals_with_field_uses(|field| {
        if field.metric() != VehicleTotalMetric::Mass || field.role() != SourceUseRole::Conversion {
            return;
        }
        let evidence = field.evidence().expect("original selected mass conversion");
        assert!(
            evidence.selectors().iter().any(|site| {
                support::base_fragment(site, CASE_VEHICLE) == Some(wanted_selector.as_str())
            }),
            "numeric use must retain the actual selected original Modification"
        );
        original_values.push(
            support::base_fragment(evidence.original(), CASE_VEHICLE)
                .expect("original mass XML value"),
        );
    });
    assert_eq!(
        original_values,
        [format!("value=\"{mass}\"").as_str()],
        "mass is sourced from this exact original Elem"
    );
    assert_eq!(
        totals.mass().to_bits(),
        f64::from(mass).to_bits(),
        "effective exact aggregate"
    );
}
