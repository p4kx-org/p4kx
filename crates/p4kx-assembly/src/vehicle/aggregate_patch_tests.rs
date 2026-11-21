// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::SourceEncryption;

use super::support::{self, TestResult};
use crate::{VehicleAggregateIssue, VehicleTotalMetric};

#[test]
fn exact_modification_beats_case_fallback_and_retains_only_the_evaluated_prefix() -> TestResult {
    let text = br#"<Vehicle><Parts><Part id="target" mass="2"/></Parts><Modifications>
        <Modification name="observed"><Elems><Elem idRef="target" name="mass" value="99"/></Elems></Modification>
        <Modification name="Observed"><Elems><Elem name="never_read" value="unused"/>
        <Elem idRef="target" name="mass" value="7"/></Elems></Modification>
        <Modification name="late"><Elems><Elem idRef="target" name="mass" value="88"/></Elems></Modification>
    </Modifications></Vehicle>"#;
    support::with_vehicle(text, None, "Observed", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        assert_eq!(vehicle.totals().mass(), 7.0);
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        let fragments = evidence
            .selectors()
            .filter_map(|site| support::base_fragment(site, text))
            .collect::<Vec<_>>();
        assert!(
            fragments
                .iter()
                .any(|value| value.contains("name=\"observed\""))
        );
        assert!(
            fragments
                .iter()
                .any(|value| value.contains("name=\"Observed\""))
        );
        assert!(
            !fragments
                .iter()
                .any(|value| value.contains("name=\"late\""))
        );
        assert!(
            !fragments
                .iter()
                .any(|value| value.contains("name=\"never_read\""))
        );
        assert_eq!(
            evidence.issues(),
            &[],
            "the selected modification is observed"
        );
        Ok(())
    })
}

#[test]
fn case_fallback_uses_the_first_match_after_ruling_out_an_exact_name() -> TestResult {
    let text = br#"<Vehicle><Parts><Part id="target" mass="2"/></Parts><Modifications>
        <Modification name="observed"><Elems><Elem idRef="target" name="mass" value="5"/></Elems></Modification>
        <Modification name="Observed"><Elems><Elem idRef="target" name="mass" value="7"/></Elems></Modification>
    </Modifications></Vehicle>"#;
    support::with_vehicle(text, None, "OBSERVED", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        assert_eq!(vehicle.totals().mass(), 5.0);
        assert_eq!(
            vehicle
                .aggregate_evidence()
                .ok_or("aggregate scope")?
                .issues(),
            &[],
            "the fallback modification is observed"
        );
        Ok(())
    })
}

#[test]
fn missing_modification_is_known_selection_absence_with_an_unavailable_stage() -> TestResult {
    support::with_vehicle(
        b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>",
        None,
        "Missing",
        true,
        |vehicle, observer| {
            support::assert_original_sites(vehicle, observer)?;
            let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
            assert_eq!(
                evidence.issues(),
                [VehicleAggregateIssue::ModificationUnavailable]
            );
            assert_eq!(vehicle.totals().mass(), 2.0);
            Ok(())
        },
    )
}

#[test]
fn wholesale_patch_retains_later_base_parts_and_every_repeated_patch_wrapper() -> TestResult {
    let base =
        br#"<Vehicle><Parts><Part mass="100"/></Parts><Parts><Part mass="3"/></Parts></Vehicle>"#;
    let patch =
        br#"<Vehicle><Parts><Part mass="5"/></Parts><Parts><Part mass="7"/></Parts></Vehicle>"#;
    support::with_vehicle(base, Some(patch), "Observed", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        assert_eq!(vehicle.totals().mass(), 15.0);
        assert_eq!(vehicle.totals().mass_parts(), 3);
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert!(evidence.selectors().any(|site| {
            site.file()
                .source()
                .entry_origin()
                .is_some_and(|origin| origin.encryption() == SourceEncryption::Encrypted)
        }));
        let mut values = Vec::new();
        let _ = vehicle.totals_with_field_uses(|field| {
            if field.metric() == VehicleTotalMetric::Mass
                && let Some(evidence) = field.evidence()
                && let Some(value) = support::base_fragment(evidence.original(), base)
            {
                values.push(value.to_owned());
            }
        });
        assert!(!values.iter().any(|value| value.contains("mass=\"100\"")));
        Ok(())
    })
}

#[test]
fn id_overlay_pairs_by_original_name_and_drops_unmatched_old_child_payloads() -> TestResult {
    let base = br#"<Vehicle><Parts><Part id="parent" mass="1">
        <Part name="paired" mass="2"/><Part name="dropped" mass="100"/>
    </Part></Parts></Vehicle>"#;
    let patch = br#"<Patch><Part id="parent"><Part name="paired"/></Part></Patch>"#;
    support::with_vehicle(base, Some(patch), "Observed", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        assert_eq!(vehicle.totals().mass(), 3.0);
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert!(
            evidence
                .selectors()
                .filter_map(|site| support::base_fragment(site, base))
                .any(|value| value.contains("name=\"dropped\""))
        );
        let mut fields = Vec::new();
        let _ = vehicle.totals_with_field_uses(|field| {
            if let Some(evidence) = field.evidence() {
                fields.extend(support::base_fragment(evidence.original(), base).map(str::to_owned));
            }
        });
        assert!(!fields.iter().any(|value| value.contains("mass=\"100\"")));
        Ok(())
    })
}

#[test]
fn every_same_id_target_keeps_its_distinct_original_selector() -> TestResult {
    let base = br#"<Vehicle><Parts><Part id="shared" mass="1"/>
        <Part id="shared" mass="2"/></Parts></Vehicle>"#;
    let patch = br#"<Patch><Part id="shared" mass="5"/></Patch>"#;
    support::with_vehicle(base, Some(patch), "Observed", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        assert_eq!(vehicle.totals().mass(), 10.0);
        assert_eq!(vehicle.totals().mass_parts(), 2);
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert_eq!(
            evidence
                .selectors()
                .filter_map(|site| support::base_fragment(site, base))
                .filter(|value| value.contains("id=\"shared\""))
                .count(),
            2
        );
        Ok(())
    })
}

#[test]
fn unreadable_patch_keeps_base_result_and_separate_read_failure() -> TestResult {
    let base = b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>";
    let (directory, _opened) = support::archive(base, Some(b"<Patch/>"))?;
    let archive = p4kx_archive::Archive::open(directory.path().join("Data.p4k"))?;
    support::with_archive(&archive, "Observed", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        assert_eq!(vehicle.totals().mass(), 2.0);
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert!(
            evidence
                .issues()
                .contains(&VehicleAggregateIssue::PatchReadFailed)
        );
        assert!(evidence.selectors().all(|site| {
            site.file()
                .source()
                .entry_origin()
                .is_some_and(|origin| origin.encryption() == SourceEncryption::Unencrypted)
        }));
        Ok(())
    })
}

#[test]
fn failed_patch_payload_is_an_unavailable_stage_and_not_a_retained_selector() -> TestResult {
    support::with_vehicle(
        b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>",
        Some(b"<Patch><Part"),
        "Observed",
        true,
        |vehicle, observer| {
            support::assert_original_sites(vehicle, observer)?;
            assert_eq!(vehicle.totals().mass(), 2.0);
            let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
            assert!(
                evidence
                    .issues()
                    .contains(&VehicleAggregateIssue::PatchApplicationFailed)
            );
            assert!(evidence.selectors().all(|site| {
                site.file()
                    .source()
                    .entry_origin()
                    .is_some_and(|origin| origin.encryption() == SourceEncryption::Unencrypted)
            }));
            Ok(())
        },
    )
}

#[test]
fn explicit_missing_patch_keeps_the_authored_selector_and_its_distinct_status() -> TestResult {
    let text = br#"<Vehicle><Parts><Part mass="2"/></Parts><Modifications>
        <Modification name="Observed" patchFile="Modifications/missing.xml"/>
    </Modifications></Vehicle>"#;
    support::with_vehicle(text, None, "Observed", true, |vehicle, observer| {
        support::assert_original_sites(vehicle, observer)?;
        let evidence = vehicle.aggregate_evidence().ok_or("aggregate scope")?;
        assert!(
            evidence
                .issues()
                .contains(&VehicleAggregateIssue::ExplicitPatchMissing)
        );
        assert!(
            evidence
                .selectors()
                .filter_map(|site| support::base_fragment(site, text))
                .any(|value| value.contains("patchFile="))
        );
        assert_eq!(vehicle.totals().mass(), 2.0);
        Ok(())
    })
}
