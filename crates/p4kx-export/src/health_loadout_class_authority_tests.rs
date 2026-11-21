// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutClassSelection, LoadoutReadObserver, LoadoutResolver};
use p4kx_extraction::{Record, SourceEvidenceError, SourceInventory, Value};

use crate::build::original_input::test_support::{self, TestResult};
use crate::build::original_input::{OriginalCatalogInput, ship_pool_test_input};
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::catalog_read::original::{
    EntryPathCapture, LoadoutClassNameReceipt, LoadoutTargetReceipt, OriginalFlowFields,
};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

use super::fixture;

#[path = "health_loadout_class_probe.rs"]
mod probe;

#[test]
fn class_name_capture_requires_its_exact_borrowed_scope_and_original_value_before_source_uses()
-> TestResult {
    test_support::with_input(&fixture::database(false), |forge, input| {
        fixture::original(forge)?;
        fixture::original_class_locations(forge, input)?;
        let record = forge.record(1).ok_or("original class referrer")?;
        let tree = record.tree()?;
        let first = input.record_scope(&record, &tree)?;
        let second = input.record_scope(&record, &tree)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = EntryPathCapture::new(input, &admission, &first)?;
        let own = OriginalFlowFields::read(&first, None)?;
        let other = OriginalFlowFields::read(&second, None)?;
        let own_fact = own
            .facts()
            .iter()
            .find(|fact| fact.field().name() == "entityClassName")
            .ok_or("original own class field")?;
        let copied = own_fact.value().clone();
        assert_eq!(
            copied,
            Value::String("Self"),
            "the equal copy preserves nominal class text"
        );
        assert!(
            matches!(
                own.fact(&copied),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "copied text cannot mint actual tree membership"
        );
        let before = input.origins().uses.freeze()?;
        let genuine = LoadoutClassNameReceipt::record(
            &capture,
            own.guard_parent(own_fact)?,
            own_fact,
            &own.ancestors(own_fact)?,
        )?;
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "a genuine unretained getter adds no source uses"
        );
        reject_second_scope(&capture, &other, input, &before)?;
        let selected = forge.record(0).ok_or("the original chosen target")?;
        let target = LoadoutTargetReceipt::selected_class(input, genuine, &selected)?;
        assert_eq!(
            target.value()?,
            selected.guid(),
            "the genuine selected Record supplies its original GUID"
        );
        Ok(())
    })
}

fn reject_second_scope<'scope, 'read, 'source>(
    capture: &EntryPathCapture<'scope, 'read, 'source>,
    other: &OriginalFlowFields<'scope, 'read, 'source>,
    input: &OriginalCatalogInput<'_, '_>,
    before: &SourceInventory,
) -> TestResult {
    let foreign_fact = other
        .facts()
        .iter()
        .find(|fact| fact.field().name() == "entityClassName")
        .ok_or("same-tree second-scope class")?;
    let refused = LoadoutClassNameReceipt::record(
        capture,
        other.guard_parent(foreign_fact)?,
        foreign_fact,
        &other.ancestors(foreign_fact)?,
    );
    assert!(
        matches!(
            refused,
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "the same Record and same Tree cannot replace the actual borrowed scope"
    );
    assert_eq!(
        *before,
        input.origins().uses.freeze()?,
        "second-scope refusal is before source mutation"
    );
    Ok(())
}

#[test]
fn repeated_class_callback_is_idempotent_but_equal_later_physical_record_is_rejected_before_uses()
-> TestResult {
    ship_pool_test_input::with_input(
        &fixture::database(false),
        Vec::new(),
        |_, _, forge, input| {
            fixture::original(forge)?;
            fixture::original_class_locations(forge, input)?;
            let root = forge.record(0).ok_or("the original first definition")?;
            let later = forge
                .record(3)
                .ok_or("the genuine equal later definition")?;
            let probe = Arc::new(probe::Probe::new(input));
            let mut resolver = LoadoutResolver::with_observed_entities(
                Arc::new(Entities::new(forge)),
                probe.clone(),
            );
            let _node = resolver.resolve(&root)?;
            let selected = probe
                .selections
                .lock()
                .map_err(|_| "class source log")?
                .first()
                .cloned()
                .ok_or("the actual opaque getter selection")?;
            check_callbacks(&probe, input, &selected, &root, &later)
        },
    )
}

fn check_callbacks(
    probe: &probe::Probe<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    selected: &LoadoutClassSelection,
    root: &Record<'_>,
    later: &Record<'_>,
) -> TestResult {
    let before = input.origins().uses.freeze()?;
    assert!(
        probe.capture.class_record(selected, root).is_continue(),
        "the same actual chosen Record may be observed again"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "a compatible repeated callback is idempotent"
    );
    assert!(
        probe.capture.class_record(selected, later).is_break(),
        "equal GUID/name cannot authorize another physical definition"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "wrong physical selection is refused before a new Header contribution"
    );
    assert!(
        probe.capture.class_record(selected, root).is_break(),
        "a failed receiver cannot continue with a complete prefix"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "later stopped callbacks register no new source uses"
    );
    assert!(
        matches!(
            probe.capture.finish(),
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "the actual first selection failure survives the receiver's terminal route"
    );
    Ok(())
}

#[test]
fn class_target_join_rejects_an_independent_forge_even_with_shared_inventory_before_header_capture()
-> TestResult {
    test_support::with_shared_inventory_inputs(
        &fixture::database(false),
        |forge, input, foreign_forge, foreign| {
            fixture::original(forge)?;
            fixture::original(foreign_forge)?;
            assert!(
                input
                    .origins()
                    .uses
                    .is_same_inventory(&foreign.origins().uses),
                "the adversarial reports share one genuine inventory"
            );
            let record = forge.record(1).ok_or("original class referrer")?;
            let tree = record.tree()?;
            let scope = input.record_scope(&record, &tree)?;
            let fields = OriginalFlowFields::read(&scope, None)?;
            let fact = fields
                .facts()
                .iter()
                .find(|fact| fact.field().name() == "entityClassName")
                .ok_or("original getter")?;
            let admission = RecordObservation::capture(input, &record)?;
            let paths = EntryPathCapture::new(input, &admission, &scope)?;
            let name = LoadoutClassNameReceipt::record(
                &paths,
                fields.guard_parent(fact)?,
                fact,
                &fields.ancestors(fact)?,
            )?;
            let own = forge.record(0).ok_or("genuine own target")?;
            let other = foreign_forge.record(0).ok_or("genuine foreign target")?;
            assert_eq!(
                (own.guid(), own.index(), own.name()?),
                (other.guid(), other.index(), other.name()?),
                "all copied nominal Header data coincides"
            );
            reject_foreign_target(input, foreign, name, &own, &other)
        },
    )
}

fn reject_foreign_target(
    input: &OriginalCatalogInput<'_, '_>,
    foreign: &OriginalCatalogInput<'_, '_>,
    name: LoadoutClassNameReceipt,
    own: &Record<'_>,
    other: &Record<'_>,
) -> TestResult {
    let before = input.origins().uses.freeze()?;
    let result = LoadoutTargetReceipt::selected_class(input, name.clone(), other);
    assert!(
        matches!(
            result,
            Err(CatalogError::SourceEvidence(
                SourceEvidenceError::ForeignEvidence
            ))
        ),
        "actual Forge membership, not equal Header data or inventory, authorizes the chosen Record"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "foreign Record refusal adds no Header input"
    );
    let target = LoadoutTargetReceipt::selected_class(input, name, own)?;
    let before = input.origins().uses.freeze()?;
    let result = target.check_input(foreign);
    assert!(
        matches!(
            result,
            Err(ExportAccountingError::ConflictingSourceDatum
                | ExportAccountingError::UnboundHealthSubject)
        ),
        "a retained class recipe remains bound to its actual original input"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "foreign report refusal adds no payload"
    );
    Ok(())
}

#[test]
fn class_body_cannot_be_retained_by_another_report_over_the_identical_actual_forge() -> TestResult {
    test_support::with_same_forge_reports(&fixture::database(false), |forge, input, other| {
        let guid = fixture::original(forge)?;
        fixture::original_class_locations(forge, input)?;
        let body = fixture::body(forge, input, None, guid, 2)?;
        let own = fixture::body(forge, other, None, guid, 2)?;
        assert_eq!(
            body.description()?,
            own.description()?,
            "matching values over the identical Forge do not pair report authority"
        );
        let before = other.origins().uses.freeze()?;
        let result = RetainedExampleBody::read(body, other);
        assert!(
            matches!(
                result,
                Err(CatalogError::Accounting(
                    ExportAccountingError::UnboundHealthSubject
                        | ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "the actual Class body cannot cross independent report inventories"
        );
        assert_eq!(
            before,
            other.origins().uses.freeze()?,
            "foreign retained-body refusal adds no payload"
        );
        Ok(())
    })
}

#[path = "health_loadout_class_scope_tests.rs"]
mod scope;
