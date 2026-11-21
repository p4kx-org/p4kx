// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceValueState, Value};

use super::fixture::{self, Fault, Origin, TestResult};
use crate::record_observation::RecordObservation;

#[test]
fn earlier_unknown_components_and_static_entries_propagate_only_to_selected_dependents()
-> TestResult {
    for fault in [
        Fault::EarlierComponent,
        Fault::EarlierStatic,
        Fault::LaterStatic,
    ] {
        fixture::with_projection(
            &fixture::database(fault),
            Origin::Plain,
            |forge, projection| {
                let record = forge.record(0).ok_or("original ship")?;
                let tree = record.tree()?;
                check_original_error(&tree, fault)?;
                let admission = RecordObservation::capture(projection.original(), &record)?;
                let read = super::super::read(&tree, projection, &admission, None)?;
                let vehicle_complete = !matches!(fault, Fault::EarlierComponent);
                let insurance_complete = !matches!(fault, Fault::EarlierStatic);
                assert_eq!(
                    read.crew_size.origin().source_set_complete(),
                    vehicle_complete,
                    "vehicle prefix uncertainty is scoped"
                );
                assert_eq!(
                    read.vehicle_definition.origin().source_set_complete(),
                    vehicle_complete,
                    "raw vehicle selector shares the actual prefix"
                );
                assert_eq!(
                    read.insurance.origin().source_set_complete(),
                    insurance_complete,
                    "only preceding static uncertainty changes the chosen declaration proof"
                );
                let insurance = read
                    .insurance
                    .value()
                    .ok_or("first readable actual insurance")?;
                let params = insurance.params.value().ok_or("readable actual params")?;
                for origin in [
                    insurance.restore_on_claim.origin(),
                    insurance.params.origin(),
                    params.ship_entity_class_name.origin(),
                    params.base_wait_time_minutes.origin(),
                    params.mandatory_wait_time_minutes.origin(),
                    params.expedite_fee.origin(),
                ] {
                    assert_eq!(
                        origin.source_set_complete(),
                        insurance_complete,
                        "each independent nested datum carries the selected parent uncertainty"
                    );
                }
                assert!(
                    read.header.guid.origin().source_set_complete(),
                    "intrinsic header proof is independent"
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}

fn check_original_error(tree: &RecordTree<'_>, fault: Fault) -> TestResult {
    let array = if matches!(fault, Fault::EarlierComponent) {
        "Components"
    } else {
        "StaticEntityClassData"
    };
    let Value::Array(original) = tree.field(array).ok_or("original array")? else {
        return Err("actual original array storage".into());
    };
    assert!(
        original
            .iter()
            .any(|value| matches!(value, Value::Error(_))),
        "the control contains an actual original decoder error"
    );
    Ok(())
}

#[test]
fn null_insurance_params_do_not_fabricate_independent_field_absence() -> TestResult {
    fixture::with_projection(
        &fixture::database(Fault::NullParams),
        Origin::Plain,
        |forge, projection| {
            let record = forge.record(0).ok_or("original ship")?;
            let tree = record.tree()?;
            let admission = RecordObservation::capture(projection.original(), &record)?;
            let read = super::super::read(&tree, projection, &admission, None)?;
            let insurance = read
                .insurance
                .value()
                .ok_or("present insurance declaration")?;
            assert_eq!(
                insurance.restore_on_claim.value(),
                Some(&false),
                "independent restore flag remains present"
            );
            assert_eq!(
                insurance.params.state(),
                SourceValueState::Null,
                "actual null is explicit"
            );
            assert!(
                insurance.params.origin().source_set_complete(),
                "a known null is a complete source observation"
            );
            Ok(())
        },
    )
}

#[test]
fn unreadable_first_insurance_params_never_select_a_later_successful_declaration() -> TestResult {
    fixture::with_projection(
        &fixture::database(Fault::UnreadableParams),
        Origin::Plain,
        |forge, projection| {
            let record = forge.record(0).ok_or("original ship")?;
            let tree = record.tree()?;
            let Value::Array(entries) = tree
                .field("StaticEntityClassData")
                .ok_or("original static array")?
            else {
                return Err("original static array storage".into());
            };
            let Some(Value::Struct(first)) = entries.get(1) else {
                return Err("first original insurance declaration".into());
            };
            assert!(
                matches!(first.field("shipInsuranceParams"), Some(Value::Error(_))),
                "the parameter failure is an actual original decoder error"
            );
            let admission = RecordObservation::capture(projection.original(), &record)?;
            let read = super::super::read(&tree, projection, &admission, None)?;
            let insurance = read.insurance.value().ok_or("selected first declaration")?;
            assert_eq!(
                insurance.restore_on_claim.value(),
                Some(&false),
                "the later true declaration does not replace the first"
            );
            assert_eq!(
                insurance.params.state(),
                SourceValueState::Unreadable,
                "unreadable parameter state stays explicit"
            );
            assert!(
                !insurance.params.origin().source_set_complete(),
                "failed parameter payload cannot claim complete contributors"
            );
            assert!(
                !read.notes.is_empty(),
                "the actual source-field failure is reported"
            );
            Ok(())
        },
    )
}

#[test]
fn missing_vehicle_sources_distinguish_known_absence_from_unreadable_possible_candidates()
-> TestResult {
    for failed in [false, true] {
        fixture::with_projection(
            &fixture::without_vehicle(failed),
            Origin::Plain,
            |forge, projection| {
                let record = forge.record(0).ok_or("original ship")?;
                let tree = record.tree()?;
                let Value::Array(values) =
                    tree.field("Components").ok_or("original component array")?
                else {
                    return Err("original component array storage".into());
                };
                assert_eq!(
                    values.iter().any(|value| matches!(value, Value::Error(_))),
                    failed,
                    "the actual source precondition is known empty or failed"
                );
                let admission = RecordObservation::capture(projection.original(), &record)?;
                let read = super::super::read(&tree, projection, &admission, None)?;
                let expected = if failed {
                    SourceValueState::Unreadable
                } else {
                    SourceValueState::Absent
                };
                assert_eq!(
                    read.crew_size.state(),
                    expected,
                    "crew size keeps selection uncertainty"
                );
                assert_eq!(
                    read.vehicle_definition.state(),
                    expected,
                    "raw vehicle selector keeps selection uncertainty"
                );
                assert_eq!(
                    read.playability_inputs.crew_manifest.state(),
                    expected,
                    "crew link has its own unknown selection state"
                );
                assert_eq!(
                    read.insurance.state(),
                    SourceValueState::Absent,
                    "the independent known-empty static collection stays absent"
                );
                Ok(())
            },
        )?;
    }
    Ok(())
}
