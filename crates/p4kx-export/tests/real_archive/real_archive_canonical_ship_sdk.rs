// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::env;
use std::slice;

use p4kx_export::catalog_queries::equipment::{
    ShipManufacturerValue, ShipSnapshot, ShipSource, with_ships, with_ships_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{Playability, VariantTarget, VehicleKind};
use p4kx_extraction::{Archive, Checksum, DataForgeEntry, SourceValueState, SourcedValue};
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::equipment_input::{dataforge, record_index, with_archive};
use super::variant_parity;
use super::{complete, literal_filter, ship_row};
use crate::real_archive_catalog_table_parity::{Origins, TestResult, wire_row};
use crate::real_archive_verification::print_line;

const RECORD: &str = "P4KX_SHIP_SDK_RECORD";
const RECORD_PATH: &str = "P4KX_SHIP_SDK_RECORD_PATH";
const NAME_KEY: &str = "P4KX_SHIP_SDK_PUBLIC_NAME_KEY";
const MANUFACTURER: &str = "P4KX_SHIP_SDK_ATTACHMENT_MANUFACTURER";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Evidence {
    Cohort,
    ExosuitLocalAbsence,
}

struct Selection {
    index: u32,
    path: String,
    evidence: Evidence,
    name_key: Option<String>,
    manufacturer: Option<String>,
}

impl Selection {
    fn configured() -> TestResult<Self> {
        let index = record_index(RECORD)?;
        let path = env::var(RECORD_PATH).map_err(|_| "configure the original Ship Record path")?;
        if path.is_empty() || path.contains(['*', '?']) {
            return Err("the nominated original Ship path must be literal".into());
        }
        let evidence = match env::var("P4KX_SHIP_SDK_VARIANT_EVIDENCE").as_deref() {
            Err(env::VarError::NotPresent) => Evidence::Cohort,
            Ok("localExosuitAbsent") => Evidence::ExosuitLocalAbsence,
            _ => return Err("unrecognized expected Ship variant evidence kind".into()),
        };
        Ok(Self {
            index,
            path,
            evidence,
            name_key: optional_expected(NAME_KEY)?,
            manufacturer: optional_expected(MANUFACTURER)?,
        })
    }

    const fn record(&self) -> CatalogRecordSelection {
        CatalogRecordSelection::RecordIndex(self.index)
    }
}

fn optional_expected(name: &str) -> TestResult<Option<String>> {
    match env::var(name) {
        Ok(value) if value.is_empty() => Err("an expected Ship source value is empty".into()),
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

struct SelectedShip {
    row: Value,
    set: Option<Value>,
    origins: Origins,
    state: &'static str,
}

#[test]
#[ignore = "real Archive: run one physical Ship cold in its own measured process"]
fn installed_canonical_ship_cold_query() -> TestResult {
    with_archive(|archive| {
        let selection = Selection::configured()?;
        let result = with_ships(
            archive,
            selection.record(),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| take_snapshot(archive, &selection, &snapshot),
        )
        .map_err(|_| "cold canonical Ship query did not complete")??;
        report("cold", &result)
    })
}

#[test]
#[ignore = "real Archive: compare one physical Ship with a separate exact Normal scope"]
fn installed_canonical_ship_matches_exact_scoped_normal_producer() -> TestResult {
    with_archive(|archive| {
        let selection = Selection::configured()?;
        let input = dataforge(archive)?;
        let expected = with_ships_from(
            archive,
            &input,
            selection.record(),
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| take_snapshot(archive, &selection, &snapshot),
        )
        .map_err(|_| "scoped canonical Ship query did not complete")??;
        if let Some(code) = &selection.manufacturer
            && code != "null"
        {
            verify_manufacturer_target(&input, &expected.row, code)?;
        }
        let filter = literal_filter(&input, selection.index)?;
        let normal =
            variant_parity::scoped_normal(archive, &input, &filter, "/ships", &expected.origins)?;
        if selection.evidence == Evidence::ExosuitLocalAbsence
            && normal.get("ships").and_then(Value::as_array).map(Vec::len) != Some(1)
        {
            return Err("the exosuit scope must publish exactly one physical Ship".into());
        }
        let row = ship_row(&normal, selection.index)?;
        if row != &expected.row
            || variant_parity::selected_set(&normal, row)? != expected.set.as_ref()
        {
            return Err("the exact Normal and SDK Ship row or variant evidence differs".into());
        }
        expected.origins.verify(&normal)?;
        if selection.name_key.is_some() && selection.manufacturer.is_some() {
            print_line("Canonical Ship authored presentation verified");
        }
        report("parity", &expected)
    })
}

fn take_snapshot(
    archive: &Archive,
    selection: &Selection,
    snapshot: &ShipSnapshot<'_>,
) -> TestResult<SelectedShip> {
    let sources = snapshot.sources();
    if sources.build() != archive.build()
        || !complete(sources.coverage())
        || !sources.dependency_coverage().is_complete()
        || !sources.requested_languages().is_empty()
        || !sources.available_languages().is_empty()
        || !sources.localization_notes().is_empty()
    {
        return Err(
            "canonical Ship requires complete same-Build source and dependency coverage".into(),
        );
    }
    let [ship] = snapshot.rows() else {
        return Err("exactly one nominated physical Ship is required".into());
    };
    if ship.source().record_index().value() != Some(&selection.index)
        || ship.source().record_path().value().copied() != Some(selection.path.as_str())
    {
        return Err("the selected Ship differs from the retained physical owner".into());
    }
    verify_presentation_sources(selection, ship, snapshot)?;
    let row = wire_row(ship.catalog_row().ok_or("canonical Ship row unavailable")?)?;
    let provenance = snapshot.provenance();
    let set = snapshot_set(snapshot, ship, selection.evidence)?;
    let decision = ship
        .variant_of()
        .ok_or("canonical Ship variant decision unavailable")?;
    let state = verify_decision(&row, set.as_ref(), decision, selection.index)?;
    if !ship.variant_failed_source_uses().is_empty() {
        return Err("a complete selected Ship has failed variant inputs".into());
    }
    let mut origins = Origins::default();
    origins.extend(provenance.inventory(), slice::from_ref(&row))?;
    if let Some(set) = &set {
        origins.extend(provenance.inventory(), slice::from_ref(set))?;
    }
    Ok(SelectedShip {
        row,
        set,
        origins,
        state,
    })
}

fn verify_presentation_sources(
    selection: &Selection,
    ship: &ShipSource<'_>,
    snapshot: &ShipSnapshot<'_>,
) -> TestResult {
    if let Some(expected) = selection.name_key.as_deref() {
        let name = ship
            .name()
            .value()
            .and_then(|name| snapshot.sources().public_name(name))
            .ok_or("the selected Ship lacks its authored name")?;
        if name.key().value().copied() != Some(expected)
            || !ship.name().origin().source_set_complete()
        {
            return Err("the selected Ship name or its source evidence differs".into());
        }
    }
    if let Some(expected) = selection.manufacturer.as_deref() {
        let cell = ship.attachment_manufacturer().manufacturer();
        let matches = match (expected, cell.value()) {
            ("null", None) => cell.state() == SourceValueState::Null,
            ("ARGO" | "BANU" | "UNKN", Some(ShipManufacturerValue::Record(link))) => {
                link.resolved_index().is_some() && link.guid().value().is_some()
            }
            _ => false,
        };
        if !matches || !cell.origin().source_set_complete() {
            return Err("the selected Ship manufacturer or its source evidence differs".into());
        }
    }
    Ok(())
}

fn verify_manufacturer_target(input: &DataForgeEntry, row: &Value, code: &str) -> TestResult {
    let link = row
        .pointer("/attachmentManufacturer/manufacturer/value")
        .ok_or("the expected manufacturer needs an original Record link")?;
    let index = link
        .pointer("/targetRecordIndex/value")
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or("the original manufacturer target index is missing")?;
    let forge = input.open()?;
    let target = forge
        .record(index)
        .ok_or("the original manufacturer target Record is missing")?;
    let guid = target.guid().to_string();
    if link.pointer("/guid/value").and_then(Value::as_str) != Some(guid.as_str())
        || !matches!(
            target.tree()?.field("Code"),
            Some(p4kx_extraction::Value::String(actual)) if *actual == code
        )
    {
        return Err("the original manufacturer target identity or code differs".into());
    }
    Ok(())
}

fn snapshot_set(
    snapshot: &ShipSnapshot<'_>,
    ship: &ShipSource<'_>,
    evidence: Evidence,
) -> TestResult<Option<Value>> {
    let set = match evidence {
        Evidence::Cohort => {
            let sets = snapshot.provenance().variant_source_use_sets();
            let id = ship
                .variant_source_use_set_ref()
                .ok_or("the selected Ship must run its physical cohort comparison")?;
            if sets.iter_sets().len() != 1 {
                return Err("exactly one physical Ship cohort is required".into());
            }
            Some(wire_row(
                sets.get(id)
                    .ok_or("the selected shared source-use set is missing")?,
            )?)
        }
        Evidence::ExosuitLocalAbsence => {
            if ship.vehicle_kind().value() != Some(&VehicleKind::Exosuit)
                || ship.playability().value() != Some(&Playability::Player)
                || ship.variant_source_use_set_ref().is_some()
                || !snapshot.provenance().variant_source_use_sets().is_empty()
            {
                return Err(
                    "the selected playable exosuit must use only local variant evidence".into(),
                );
            }
            None
        }
    };
    Ok(set)
}

fn verify_decision(
    row: &Value,
    set: Option<&Value>,
    decision: &SourcedValue<VariantTarget>,
    index: u32,
) -> TestResult<&'static str> {
    let wire = row.get("variantOf").ok_or("missing variant wire field")?;
    let state = match decision.state() {
        SourceValueState::Present => {
            let target = decision
                .value()
                .ok_or("present Ship variant lacks its physical base")?;
            if target
                .target_record_index()
                .value()
                .is_none_or(|base| *base == index)
                || target.guid().value().is_none()
            {
                return Err("the present Ship variant lacks its distinct sourced base".into());
            }
            "present"
        }
        SourceValueState::Absent => "absent",
        _ => return Err("the selected Ship's variant comparison is unresolved".into()),
    };
    let origin = decision.origin();
    if set.is_none()
        && (state != "absent"
            || wire.get("value") != Some(&Value::Null)
            || wire.get("sourceUseSetRef") != Some(&Value::Null)
            || origin.source_uses_refs().is_empty())
    {
        return Err("the physical exosuit absence must close over original local evidence".into());
    }
    let encryption = serde_json::to_value(origin.contains_encrypted_source())?;
    if !origin.source_set_complete()
        || wire.get("state").and_then(Value::as_str) != Some(state)
        || wire.get("sourceSetComplete").and_then(Value::as_bool) != Some(true)
        || wire.get("containsEncryptedSource") != Some(&encryption)
        || !wire
            .get("failedSourceUseRefs")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty)
    {
        return Err(
            "the canonical Ship decision's source health differs across API and wire".into(),
        );
    }
    variant_parity::verify_expanded_origin(wire, set, origin)?;
    Ok(state)
}

fn report(phase: &str, ship: &SelectedShip) -> TestResult {
    let wire = serde_json::to_vec(&(&ship.row, &ship.set))?;
    let digest = Checksum::Sha256(Sha256::digest(wire).into()).hex();
    print_line(&format!(
        "Canonical Ship {phase} rows=1 variant={} digest={digest}",
        ship.state
    ));
    Ok(())
}
