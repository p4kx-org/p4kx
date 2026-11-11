// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{Record, RecordTree, Value};
use serde_json::Value as Json;

use crate::Kind;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_number_capture::FloatToken;
use crate::catalog_number_health::{NumberCaptures, NumberRequirement};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_row::CatalogRows;
use crate::catalog_target_index::TargetIndex;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::{SourceDiagnosis, SourceDisposition, SourceObservation};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_flow_plan::FlowPlan;
use crate::template_issues::Unknowns;
use crate::template_number_capture::TemplateNumbers;
use crate::template_reference_index::TemplateReferences;
use crate::template_source_members::TemplateMembers;

use super::selection::{self, Action};

#[path = "template_number_dependency_fixture.rs"]
mod fixture;

#[test]
fn actual_template_dependency_uses_only_the_emitted_same_original_number_occurrences() -> TestResult
{
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual source Template")?;
        let tree = record.tree()?;
        controls(&record, &tree)?;
        assert!(
            matches!(selection::choose(&record)?, Some(Action::Template)),
            "the real mission dependency selector chooses its Template action"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let (numbers, requirements) = emission(&fields, input, &record)?;
        let admission = RecordObservation::capture(input, &record)?;
        let mut ledger = crate::export_field_walk::inspect_observed(
            Some(input),
            &tree,
            &admission,
            Some(Kind::ContractTemplates),
            ExportSourceRole::Selected,
            None,
        )?;
        super::observe(
            &mut ledger,
            Action::Template,
            &admission,
            SourceDisposition::Projected,
            &SourceDiagnosis::default(),
        )?;
        let dependency = ledger
            .sources
            .iter()
            .find(|source| {
                source.kind == Some(Kind::Contracts) && source.role == ExportSourceRole::Dependency
            })
            .ok_or("actual observed dependency admission")?;
        assert_coverage(&numbers, &requirements, dependency)?;
        let mut outside = requirements
            .first()
            .ok_or("actual original numeric requirement")?
            .clone();
        let fact = fields
            .field(None, "outside")?
            .ok_or("actual non-emitted outside field")?
            .1;
        outside.original_ordinal = fact.ordinal();
        outside.field = fields.logical_path(fact)?;
        outside.source_type = fact.owner_type().to_owned();
        with_coverage(&numbers, true, dependency, |coverage| {
            assert!(
                !coverage.take(&outside),
                "equal bits in an actually present outside occurrence remain blocking"
            );
            Ok(())
        })?;
        foreign_input(&numbers)?;
        Ok(())
    })
}

fn controls(record: &Record<'_>, tree: &RecordTree<'_>) -> TestResult {
    assert!(tree.is_from(record), "actual immutable Record/tree pair");
    assert_eq!(
        tree.error_count(),
        0,
        "all original numbers decode without hidden errors"
    );
    let Value::Struct(flow) = tree.field("missionFlow").ok_or("actual original flow")? else {
        return Err("actual original Flow Struct".into());
    };
    let Value::Array(equal) = flow.field("equal").ok_or("actual equal-number array")? else {
        return Err("actual numeric source array".into());
    };
    assert_eq!(equal.len(), 2, "two genuine equal source slots");
    for value in equal {
        assert_eq!(
            FloatToken::read(value),
            Some(FloatToken::Single(fixture::SINGLE)),
            "each original Single keeps its exact width and IEEE bits"
        );
    }
    assert_eq!(
        FloatToken::read(flow.field("double").ok_or("actual original Double")?),
        Some(FloatToken::Double(fixture::DOUBLE)),
        "exact original Double bits"
    );
    Ok(())
}

fn emission<'scope, 'source>(
    fields: &OriginalFlowFields<'scope, '_, 'source>,
    input: &'scope crate::build::original_input::OriginalCatalogInput<'_, '_>,
    record: &Record<'source>,
) -> TestResult<(TemplateNumbers, Vec<NumberRequirement>)> {
    emission_kind(fields, input, record, Kind::ContractTemplates)
}

fn emission_kind<'scope, 'source>(
    fields: &OriginalFlowFields<'scope, '_, 'source>,
    input: &'scope crate::build::original_input::OriginalCatalogInput<'_, '_>,
    record: &Record<'source>,
    kind: Kind,
) -> TestResult<(TemplateNumbers, Vec<NumberRequirement>)> {
    let plan = FlowPlan::read(fields)?;
    let roots = plan
        .declarations
        .iter()
        .filter_map(|declaration| declaration.declaration);
    let members = TemplateMembers::new(fields, roots, FlowOwnerKind::ContractTemplate)?;
    let admission = RecordObservation::capture(input, record)?;
    let capture = TemplateFieldCapture::new(fields, input, &admission)?;
    let mut references = TemplateReferences::new(fields);
    let (source, failure) = crate::template_meanings::read(
        &members,
        &plan,
        &mut references,
        Some(&Unknowns::default()),
        &capture,
    )?;
    assert!(
        failure.is_none(),
        "the real raw producer has no unsupported required meaning"
    );
    let wire: Json = serde_json::to_value(source)?;
    let values = wire
        .get("values")
        .and_then(Json::as_array)
        .ok_or("real produced source arena")?;
    let tokens: Vec<_> = values
        .iter()
        .filter_map(|node| {
            node.pointer("/value/value/value/rawBits")
                .and_then(Json::as_str)
        })
        .collect();
    assert_eq!(
        tokens,
        ["7fc01234", "7fc01234", "7fc01234", "7ff800000000abcd"],
        "the actual raw producer emits all four original nonfinite occurrences exactly"
    );
    let mut requirements = Vec::new();
    for fact in members.values() {
        if let Some(token) = FloatToken::read(fact.value()) {
            requirements.push(NumberRequirement {
                record_index: record.index(),
                original_ordinal: fact.ordinal(),
                field: fields.logical_path(fact)?,
                source_type: fact.owner_type().to_owned(),
                contract: None,
                token,
                body: None,
            });
        }
    }
    let numbers = match kind {
        Kind::ContractTemplates => TemplateNumbers::read(&members, input)?,
        Kind::MissionBrokerEntries => TemplateNumbers::broker(&members, input)?,
        _ => return Err("only actual canonical Flow-owner families are used in this test".into()),
    };
    Ok((numbers, requirements))
}

fn with_coverage(
    numbers: &TemplateNumbers,
    emitted: bool,
    source: &SourceObservation,
    check: impl FnOnce(&mut crate::catalog_number_health::Coverage<'_, '_>) -> TestResult,
) -> TestResult {
    with_family(
        numbers,
        emitted.then_some(Kind::ContractTemplates),
        source,
        check,
    )
}

fn with_family(
    numbers: &TemplateNumbers,
    emitted: Option<Kind>,
    source: &SourceObservation,
    check: impl FnOnce(&mut crate::catalog_number_health::Coverage<'_, '_>) -> TestResult,
) -> TestResult {
    let rows = CatalogRows {
        template_numbers: vec![numbers.clone()],
        ..CatalogRows::default()
    };
    let targets = TargetIndex {
        emitted: emitted.map_or_else(HashSet::default, |kind| [(kind, 0)].into()),
        emitted_by_guid: HashMap::default(),
        ambiguous: HashSet::default(),
        ambiguous_sources: Vec::new(),
        excluded: HashMap::default(),
    };
    let indexed = NumberCaptures::new(Some(&rows), Some(&targets));
    check(&mut indexed.for_source(source)?)
}

fn assert_coverage(
    numbers: &TemplateNumbers,
    requirements: &[NumberRequirement],
    source: &SourceObservation,
) -> TestResult {
    assert_eq!(
        requirements.len(),
        4,
        "four independent original nonfinite requirements"
    );
    with_coverage(numbers, true, source, |coverage| {
        for requirement in requirements {
            assert!(
                coverage.take(requirement),
                "each actual emitted original occurrence covers itself"
            );
        }
        assert!(
            !coverage.take(requirements.first().ok_or("first actual requirement")?),
            "one original occurrence receipt cannot cover a repeated requirement"
        );
        Ok(())
    })?;
    for requirement in requirements {
        with_coverage(numbers, false, source, |coverage| {
            assert!(
                !coverage.take(requirement),
                "source values without the actual D emission cannot waive loss"
            );
            Ok(())
        })?;
    }
    wrong_requirements(
        numbers,
        requirements.first().ok_or("actual requirement")?,
        source,
    )?;
    for (kind, role) in [
        (Kind::ResourceTypes, ExportSourceRole::Selected),
        (Kind::GameplayProperties, ExportSourceRole::Dependency),
        (Kind::Contracts, ExportSourceRole::Selected),
    ] {
        let mut foreign = source.clone();
        foreign.kind = Some(kind);
        foreign.role = role;
        with_coverage(numbers, true, &foreign, |coverage| {
            assert!(
                !coverage.take(requirements.first().ok_or("actual requirement")?),
                "a genuine original subject does not waive another family or role"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn wrong_requirements(
    numbers: &TemplateNumbers,
    original: &NumberRequirement,
    source: &SourceObservation,
) -> TestResult {
    let mut width = original.clone();
    width.token = FloatToken::Double(u64::from(fixture::SINGLE));
    let mut bits = original.clone();
    bits.token = FloatToken::Single(fixture::SINGLE ^ 1);
    let mut owner = original.clone();
    owner.record_index = owner
        .record_index
        .checked_add(1)
        .ok_or("next physical Record index")?;
    let mut path = original.clone();
    path.field.push_str(".different");
    let mut source_type = original.clone();
    source_type.source_type.push_str("Different");
    for wrong in [width, bits, owner, path, source_type] {
        with_coverage(numbers, true, source, |coverage| {
            assert!(
                !coverage.take(&wrong),
                "exact width/bits/physical owner/path/source type remain mandatory"
            );
            Ok(())
        })?;
    }
    Ok(())
}

fn foreign_input(numbers: &TemplateNumbers) -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge
            .record(0)
            .ok_or("actual foreign-input original Template")?;
        let admission = RecordObservation::capture(input, &record)?;
        let source = admission.source(
            Some(Kind::Contracts),
            ExportSourceRole::Dependency,
            SourceDisposition::Projected,
        )?;
        assert!(
            numbers.for_source(&source)?.is_none(),
            "equal GUID/index/path/bits in another genuine input cannot cover this original"
        );
        Ok(())
    })
}
#[path = "broker_number_tests.rs"]
mod broker_numbers;
