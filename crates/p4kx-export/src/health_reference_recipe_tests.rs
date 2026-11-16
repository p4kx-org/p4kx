// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourceLocation, SourceUse, SourceUseRole, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::health_reference_observation::{
    ReferenceBody, ReferenceEvidence,
};
use crate::catalog_read::original::health_reference_path::ReferencePathUses;
use crate::catalog_read::original::health_reference_receipt::{ReferenceFrame, ReferenceUse};
use crate::catalog_read::original::health_reference_recipe::ReferenceRecipe;
use crate::catalog_read::original::{
    self, CatalogReferenceProof, ExampleBodyReceipt, OriginalValueEvent, OriginalValueSite,
    ReferenceCapture, ReferenceObservation,
};
use crate::export_diagnostic::{
    ExportDiagnosticCode, ExportDiagnosticSeverity, ExportDiagnosticStage, ExportDiagnosticUnit,
    ExportSourceRole,
};
use crate::health_diagnostic_observation::{DiagnosticObservation, ExampleObservation};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError, ExportReferenceStatus, Kind};

#[derive(Clone, Copy)]
enum Retention {
    Immediate,
    Deferred,
}

#[test]
fn exact_target_report_and_sample_bytes_match_the_eager_representation() -> TestResult {
    assert!(
        size_of::<ReferenceRecipe>() < size_of::<ReferenceBody>(),
        "the deferred payload stores contributions separately from already described values",
    );
    assert_eq!(
        size_of::<ReferencePathUses>(),
        size_of::<Vec<ReferenceUse>>(),
        "sharing a prefix does not enlarge the retained recipe"
    );
    let baseline = snapshot(Retention::Immediate)?;
    let deferred = snapshot(Retention::Deferred)?;
    assert_eq!(
        deferred, baseline,
        "reports, sampled examples, provenance IDs and row order are byte-identical",
    );
    Ok(())
}

fn snapshot(retention: Retention) -> TestResult<Vec<u8>> {
    let mut result = Vec::new();
    test_support::with_input(&database(), |forge, input| {
        let references = observations(forge, input, retention)?;
        let references = references
            .into_iter()
            .map(|reference| {
                let mut row = reference.resolved(ExportReferenceStatus::ExcludedByFilter);
                row.contextualize(Some(Kind::Items), ExportSourceRole::Dependency);
                row
            })
            .collect::<Vec<_>>();
        let diagnostic = diagnostic(&references)?;
        let report = original::finish_report(
            input,
            &[],
            Vec::new(),
            Vec::new(),
            vec![diagnostic],
            references,
            None,
        )?;
        result = serde_json::to_vec(&(
            report.references,
            report.diagnostics,
            report.metadata,
            input.origins().uses.freeze()?,
        ))?;
        Ok(())
    })?;
    Ok(result)
}

fn diagnostic(references: &[ReferenceObservation]) -> TestResult<DiagnosticObservation> {
    let examples = references
        .iter()
        .map(|reference| {
            let data = reference.data();
            ExampleObservation::captured(
                reference.subject().clone(),
                Some(data.field.as_str()),
                Some(data.source_type.as_str()),
                data.target_guid,
                ExampleBodyReceipt::reference(reference)?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DiagnosticObservation {
        kind: Some(Kind::Items),
        stage: ExportDiagnosticStage::SourceField,
        code: ExportDiagnosticCode::CatalogTargetUnavailable,
        severity: ExportDiagnosticSeverity::Information,
        source_role: Some(ExportSourceRole::Dependency),
        warning_code: None,
        count: references.len(),
        count_unit: ExportDiagnosticUnit::Values,
        examples,
    })
}

#[test]
fn deferred_recipe_keeps_physical_path_caller_and_target_operands_in_order() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let refs = candidates(forge, input)?;
        let [first, second, weak] = refs.as_slice() else {
            return Err("two array References and one actual root-target WeakPointer".into());
        };
        let left = first.recipe_for_test().ok_or("first original recipe")?;
        let right = second.recipe_for_test().ok_or("second original recipe")?;
        assert!(
            left.path.shares_prefix_with(&right.path),
            "both retained candidates share the same physical prefix allocation"
        );
        assert_eq!(
            first.data().field.as_str(),
            "body.links[0]",
            "first array site"
        );
        assert_eq!(
            second.data().field.as_str(),
            "body.links[1]",
            "second array site"
        );
        assert_eq!(weak.data().field.as_str(), "body.weak", "weak caller site");
        let uses = &input.origins().uses;
        let before = uses.freeze()?;
        let first = first.materialize(None)?;
        let second = second.materialize(None)?;
        let weak = weak.materialize(None)?;
        assert_eq!(
            before,
            uses.freeze()?,
            "materialization alone claims no source contribution",
        );
        check_operands(&first, 2, false)?;
        check_operands(&second, 3, false)?;
        check_operands(&weak, 4, true)
    })
}

fn check_operands(reference: &ReferenceObservation, caller: u64, weak: bool) -> TestResult {
    use SourceUseRole::{Conversion, Payload, Selector};

    let ReferenceEvidence::Captured { frame, body } = &reference.evidence else {
        return Err("one materialized original Reference".into());
    };
    let mut field = vec![(value(0, "$fieldName"), Conversion)];
    if weak {
        field.push((value(caller, "$fieldName"), Conversion));
    } else {
        field.push((value(1, "$fieldName"), Conversion));
        field.push((value(1, "$value"), Selector));
    }
    check_inputs(body.field.clone().finish(frame)?.contributions(), &field);
    check_inputs(
        body.source_type.clone().finish(frame)?.contributions(),
        &[(value(0, "$type"), Selector)],
    );
    let guid = if weak {
        vec![
            (header("guid"), Payload),
            (value(caller, "$value"), Selector),
            (value(caller, "$value"), Conversion),
        ]
    } else {
        vec![(value(caller, "$value"), Payload)]
    };
    check_inputs(
        body.target_guid.clone().finish(frame)?.contributions(),
        &guid,
    );
    check_target_operands(frame, body, caller)
}

fn check_target_operands(frame: &ReferenceFrame, body: &ReferenceBody, caller: u64) -> TestResult {
    use SourceUseRole::{Conversion, Selector};

    check_inputs(
        body.target_record_index
            .clone()
            .finish(frame)?
            .contributions(),
        &[
            (header("recordIndex"), Selector),
            (value(caller, "$value"), Selector),
        ],
    );
    check_inputs(
        body.target_record_path
            .clone()
            .finish(frame)?
            .contributions(),
        &[
            (header("fileName"), Conversion),
            (value(caller, "$value"), Selector),
        ],
    );
    check_inputs(
        body.target_type.clone().finish(frame)?.contributions(),
        &[
            (header("sourceType"), Selector),
            (value(caller, "$value"), Selector),
        ],
    );
    Ok(())
}

fn check_inputs(
    (uses, complete): (&[SourceUse], bool),
    expected: &[(SourceLocation, SourceUseRole)],
) {
    assert!(complete, "the original retained contributions are complete");
    assert!(
        uses.iter()
            .map(|used| (used.description().location(), used.role()))
            .eq(expected.iter().map(|(location, role)| (location, *role))),
        "all exact original coordinates and roles survive in sequence",
    );
}

fn value(ordinal: u64, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeValue".to_owned(),
        indices: vec![0, ordinal],
        field: field.to_owned(),
    }
}

fn header(field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordHeader".to_owned(),
        indices: vec![1],
        field: field.to_owned(),
    }
}

#[test]
fn immediate_and_deferred_reads_of_one_occurrence_are_equivalent() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let immediate = observations(forge, input, Retention::Immediate)?;
        let deferred = observations(forge, input, Retention::Deferred)?;
        assert_eq!(
            immediate.len(),
            deferred.len(),
            "all source occurrences match"
        );
        for (left, right) in immediate.iter().zip(&deferred) {
            left.check_duplicate(right)?;
            right.check_duplicate(left)?;
        }
        Ok(())
    })
}

#[test]
fn equal_deferred_recipes_cannot_import_another_archive_within_the_same_inventory() -> TestResult {
    test_support::with_shared_inventory_inputs(&database(), |forge, input, other, other_input| {
        let original = candidates(forge, input)?;
        let foreign = candidates(other, other_input)?;
        for (left, right) in original.iter().zip(&foreign) {
            assert_eq!(left, right, "description and coordinate equality coincide");
            assert!(
                matches!(
                    left.check_duplicate(right),
                    Err(ExportAccountingError::ConflictingSourceDatum)
                ),
                "equal deferred recipes cannot import another original frame",
            );
        }
        let before = input.origins().uses.freeze()?;
        let outcome = original::finish_report(
            other_input,
            &[],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            original
                .into_iter()
                .map(|row| row.materialize(Some(ExportReferenceStatus::TargetNotEmitted)))
                .collect::<Result<_, _>>()?,
            None,
        );
        assert!(
            matches!(
                outcome,
                Err(CatalogError::Accounting(
                    ExportAccountingError::UnboundHealthSubject
                ))
            ),
            "publication still requires an original frame in this Archive",
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "failed authority checks register no source contributions",
        );
        Ok(())
    })
}

fn observations(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    retention: Retention,
) -> TestResult<Vec<ReferenceObservation>> {
    read_occurrences(forge, input, |capture, site| match retention {
        Retention::Immediate => capture.reference(site),
        Retention::Deferred => Ok(capture.catalog_reference(site)?.materialize(None)?),
    })
}

fn candidates(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult<Vec<CatalogReferenceProof>> {
    read_occurrences(forge, input, |capture, site| {
        capture.catalog_reference(site)
    })
}

fn read_occurrences<T>(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    mut read: impl FnMut(
        &ReferenceCapture<'_, '_, '_>,
        &OriginalValueSite<'_, '_, '_, '_>,
    ) -> Result<T, CatalogError>,
) -> TestResult<Vec<T>> {
    let owner = forge.record(0).ok_or("original Owner record")?;
    let tree = owner.tree()?;
    assert_eq!(
        tree.error_count(),
        0,
        "all three original target links are readable"
    );
    let scope = input.record_scope(&owner, &tree)?;
    let admission = RecordObservation::capture(input, &owner)?;
    let mut capture = ReferenceCapture::new(input, &admission, &scope)?;
    let mut walk = scope.walk(None);
    let mut rows = Vec::new();
    while let Some(event) = walk.next_event()? {
        match event {
            OriginalValueEvent::Value(site) => match site.value() {
                Value::Struct(_) | Value::Array(_) => capture.enter(&site)?,
                Value::Reference(_) => rows.push(read(&capture, &site)?),
                _ => {}
            },
            OriginalValueEvent::EndContainer { ordinal, .. } => capture.leave(ordinal)?,
        }
    }
    assert_eq!(rows.len(), 3, "the original occurrence count");
    Ok(rows)
}

fn database() -> Vec<u8> {
    const TARGET_TYPE: &str = "SCItemManufacturer";
    const TARGET_GUID: u128 = 0x6812;
    const TARGET_STRUCT_INDEX: u32 = 2;
    let weak = TestValue::Raw([TARGET_STRUCT_INDEX.to_le_bytes(), 0_u32.to_le_bytes()].concat());
    let body = TestInstance::new("Inner")
        .value(
            "links",
            TestValue::Array(vec![TestValue::Reference(TARGET_GUID); 2]),
        )
        .value("weak", weak);
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner").field("body", TestType::Class("Inner".to_owned())))
        .structure(
            TestStruct::new("Inner")
                .array("links", TestType::ReferenceTo(TARGET_TYPE.to_owned()))
                .field("weak", TestType::WeakPointer(TARGET_TYPE.to_owned())),
        )
        .structure(TestStruct::new(TARGET_TYPE).field("marker", TestType::UInt32))
        .record(
            TestRecord::new(0x6811, "Owner", "Owner.One", "owner.xml")
                .value("body", TestValue::Struct(body)),
        )
        .record(TestRecord::new(
            TARGET_GUID,
            TARGET_TYPE,
            "SCItemManufacturer.Target",
            "libs/foundry/records/scitemmanufacturer/target.xml",
        ))
        .build()
}
