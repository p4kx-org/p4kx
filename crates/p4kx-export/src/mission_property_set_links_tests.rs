// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Guid, SourceEvidenceError, StructValue, Value};

use super::MissionPropertySetLinks;
use crate::CatalogError;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::guid_text::CatalogGuid;
use crate::mission_property_fields::PropertyFieldError;
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::template_field_capture::TemplateFieldCapture;

#[path = "mission_property_set_links_fixture.rs"]
mod fixture;

#[test]
fn source_links_keep_row_scopes_without_decoding_properties_or_siblings() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("original property-link Record")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let links = MissionPropertySetLinks::new(&tree).map_err(test_error)?;
        let handler = element(tree.root(), "generators", 0)?;
        let contract = element(handler, "contracts", 0)?;
        let first = element(contract, "subContracts", 0)?;
        let direct = element(contract, "subContracts", 1)?;
        let before = scope.sources().freeze()?;
        assert_eq!(
            links
                .row(handler, contract, None, &capture)
                .map_err(test_error)?,
            expected(tree.guid(), None)?,
            "the parent row includes no sub-contract scope"
        );
        for (sub, location) in [
            (
                first,
                "generators[0].contracts[0].subContracts[0].paramOverrides",
            ),
            (direct, "generators[0].contracts[0].subContracts[1]"),
        ] {
            assert_eq!(
                links
                    .row(handler, contract, Some(sub), &capture)
                    .map_err(test_error)?,
                expected(tree.guid(), Some(location))?,
                "the exact selected source wins over direct or sibling declarations"
            );
        }
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "source-only links neither interpret the Int32 property elements nor emit payload claims"
        );
        Ok(())
    })
}

#[test]
fn absent_parameters_produce_no_authored_set_links() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("original blank-scope Record")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let links = MissionPropertySetLinks::new(&tree).map_err(test_error)?;
        let handler = element(tree.root(), "generators", 1)?;
        let contract = element(handler, "contracts", 0)?;
        assert!(
            links
                .row(handler, contract, None, &capture)
                .map_err(test_error)?
                .is_empty(),
            "absent authored parameters cannot invent a property-set identifier"
        );
        Ok(())
    })
}

#[test]
fn invalid_selected_property_array_keeps_the_original_failure_receipt() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("original invalid-scope Record")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let links = MissionPropertySetLinks::new(&tree).map_err(test_error)?;
        let handler = element(tree.root(), "generators", 0)?;
        let contract = element(handler, "contracts", 0)?;
        let sub = element(contract, "subContracts", 2)?;
        let Err(error) = links.row(handler, contract, Some(sub), &capture) else {
            return Err("a non-array override cannot be admitted as a source set".into());
        };
        let error = error
            .downcast_ref::<PropertyFieldError>()
            .ok_or("property shape failure")?;
        assert_eq!(
            error.description().field,
            "propertyOverrides",
            "the actual rejected field"
        );
        assert!(
            error.body().is_some(),
            "the source shape error retains its original capture"
        );
        Ok(())
    })
}

#[test]
fn equal_foreign_parameters_do_not_enter_an_original_link_selection() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let record = forge.record(0).ok_or("original link owner")?;
        let other = forge.record(1).ok_or("foreign link owner")?;
        let tree = record.tree()?;
        let other_tree = other.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let admission = RecordObservation::capture(input, &record)?;
        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let links = MissionPropertySetLinks::new(&tree).map_err(test_error)?;
        let handler = element(other_tree.root(), "generators", 0)?;
        let contract = element(handler, "contracts", 0)?;
        let before = scope.sources().freeze()?;
        let Err(error) = links.row(handler, contract, None, &capture) else {
            return Err("equal payload cannot admit foreign source nodes".into());
        };
        assert!(
            matches!(
                error.downcast_ref::<CatalogError>(),
                Some(CatalogError::SourceEvidence(
                    SourceEvidenceError::ForeignEvidence
                ))
            ),
            "the source-frame rejection remains a fatal CatalogError"
        );
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "foreign nodes emit no original claim"
        );
        Ok(())
    })
}

fn expected(guid: Guid, sub: Option<&str>) -> TestResult<Vec<CatalogGuid>> {
    let mut locations = vec![
        "generators[0].contractParams.propertyOverrides".to_owned(),
        "generators[0].contracts[0].paramOverrides.propertyOverrides".to_owned(),
    ];
    if let Some(sub) = sub {
        locations.push(format!("{sub}.propertyOverrides"));
    }
    locations
        .into_iter()
        .map(|location| super::identity(guid, &location).map_err(Into::into))
        .collect()
}

fn element<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    field: &str,
    index: usize,
) -> TestResult<&'tree StructValue<'source>> {
    let Some(Value::Array(values)) = parent.field(field) else {
        return Err(format!("original array {field}").into());
    };
    let Some(Value::Struct(value)) = values.get(index) else {
        return Err(format!("original struct {field}[{index}]").into());
    };
    Ok(value)
}

fn test_error(error: ReadError) -> Box<dyn Error> {
    error
}
