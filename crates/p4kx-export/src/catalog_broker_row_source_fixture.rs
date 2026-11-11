// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{DataType, Record, RecordTree, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::Kind;
use crate::broker_definition::BrokerDefinition;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_row::CatalogRow;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_field_inspection::FieldInspection;
use crate::export_ledger::{SourceDisposition, SourceObservation};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_issues::Unknowns;
use crate::template_meanings::PreparedFlow;
use crate::template_plan::TemplatePlan;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;

pub(super) struct Actual {
    pub(super) row: CatalogRow,
    pub(super) source: SourceObservation,
    pub(super) canonical: BTreeMap<u32, FieldInspection>,
}

pub(super) fn with_actual(check: impl FnOnce(Actual) -> TestResult) -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Broker")?;
        assert_eq!(
            forge.records().len(),
            1,
            "one actual original physical owner"
        );
        let tree = record.tree()?;
        original(&record, &tree)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let emitted = definition(input, &fields, &tree, &capture)?;
        let row = emitted_row(&emitted, &record, &tree);
        let source = admission.source(
            Some(Kind::MissionBrokerEntries),
            ExportSourceRole::Selected,
            SourceDisposition::Projected,
        )?;
        let mut inspected = crate::export_field_walk::inspect_observed(
            Some(input),
            &tree,
            &admission,
            Some(Kind::MissionBrokerEntries),
            ExportSourceRole::Selected,
            None,
        )?
        .field_inspections;
        assert_eq!(
            inspected.len(),
            1,
            "one genuine canonical inspection, not a copied descriptor"
        );
        let proof = inspected.pop().ok_or("actual inspected source proof")?;
        assert_eq!(
            proof.record_index,
            record.index(),
            "same actual physical inspection position"
        );
        check(Actual {
            row,
            source,
            canonical: [(record.index(), proof)].into(),
        })
    })
}

fn emitted_row(
    emitted: &BrokerDefinition<'_>,
    record: &Record<'_>,
    tree: &RecordTree<'_>,
) -> CatalogRow {
    assert_eq!(
        emitted.source.values.len(),
        2,
        "the real D source retains its Struct and genuine empty Array occurrences"
    );
    let row = CatalogRow {
        kind: Kind::MissionBrokerEntries,
        guid: emitted.physical_guid.0,
        record_path: emitted.physical_path.to_owned(),
    };
    assert_eq!(
        row.guid,
        record.guid(),
        "actual D GUID comes from the original Record"
    );
    assert_eq!(
        row.record_path,
        tree.record_path(),
        "actual D path comes from that exact read"
    );
    row
}

fn definition<'source>(
    input: &crate::build::original_input::OriginalCatalogInput<'_, '_>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    tree: &RecordTree<'source>,
    capture: &dyn crate::source_fields::FieldCapture<'source>,
) -> Result<BrokerDefinition<'source>, crate::CatalogError> {
    let kind = FlowOwnerKind::MissionBrokerEntry;
    let mut references = TemplateReferences::new(fields);
    let plan = TemplatePlan::read(fields, &mut references, kind)?;
    let members = TemplateMembers::new(fields, plan.roots(), kind)?;
    let (_, flows) = crate::template_flow_configuration_read::read(&members, &plan, kind)?;
    let (_, properties) =
        crate::template_property_configuration_read::read(&members, &plan.objectives, kind)?;
    let PreparedFlow { meanings, failure } = crate::template_meanings::prepare(
        &members,
        &plan.flow,
        &mut references,
        Some(&Unknowns::default()),
        capture,
    )?;
    assert!(
        failure.is_none(),
        "the real raw/meaning producer has no required Domain error"
    );
    BrokerDefinition::emit(input, tree, &mut references, meanings, (flows, properties))
}

fn original<'source>(record: &Record<'source>, tree: &RecordTree<'source>) -> TestResult {
    assert_eq!(
        record.type_name()?,
        "MissionBrokerEntry",
        "the real source family"
    );
    assert!(tree.is_from(record), "the genuine original immutable pair");
    assert_eq!(tree.error_count(), 0, "all original values are readable");
    assert_eq!(
        tree.fields().len(),
        1,
        "one actual original flow declaration"
    );
    let declaration = tree.fields().first().ok_or("actual root Field")?;
    assert_eq!(
        declaration.name(),
        "missionFlow",
        "the actual authored name"
    );
    assert_eq!(
        declaration.storage().data_type(),
        DataType::Class,
        "actual Class storage"
    );
    let Some(Value::Struct(flow)) = tree.field("missionFlow") else {
        return Err("actual original Flow Struct".into());
    };
    assert!(
        matches!(flow.field("triggers"), Some(Value::Array(values)) if values.is_empty()),
        "the actual original empty trigger array remains distinct from absence"
    );
    Ok(())
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("MissionBrokerEntry")
                .field("missionFlow", TestType::Class("MissionFlow".to_owned())),
        )
        .structure(
            TestStruct::new("MissionFlow")
                .array("triggers", TestType::Class("FlowTrigger".to_owned())),
        )
        .structure(TestStruct::new("FlowTrigger"))
        .record(
            TestRecord::new(
                0xb155_0000_0000_4000_8000_0000_0000_0001,
                "MissionBrokerEntry",
                "MissionBrokerEntry.RowSource",
                "libs/foundry/records/missionbroker/observed/row_source.xml",
            )
            .value(
                "missionFlow",
                TestValue::Struct(
                    TestInstance::new("MissionFlow")
                        .value("triggers", TestValue::Array(Vec::new())),
                ),
            ),
        )
        .build()
}
