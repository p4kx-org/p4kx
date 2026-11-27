// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, StructValue, TreeBuilder, Value as Original};
use serde_json::Value;

use crate::authored_definition_witness::field;
use crate::real_archive_auto_spawns::provenance::Origins;
use crate::real_archive_canonical_texts::TextIndex;
use crate::real_archive_faction_reputation_source::presentation::{Witness, verify_number};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) fn compare(
    forge: &DataForge<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
    texts: &TextIndex<'_>,
) -> TestResult<(usize, usize)> {
    let rows = crate::catalog_v3_consumer::rows(catalog, Kind::ReputationStandings)?;
    let mut positions = BTreeSet::new();
    let expected: BTreeSet<_> = forge
        .records()
        .filter(|record| record.type_name().ok() == Some("SReputationStandingParams"))
        .map(|record| record.index())
        .collect();
    let mut trees = TreeBuilder::new();
    let mut source_singles = 0;
    for row in rows {
        let position = row
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("physical standing Record index")?;
        if !positions.insert(position) {
            return Err("repeated physical standing Record index".into());
        }
        let record = forge.record(position).ok_or("original standing Record")?;
        let tree = trees
            .build(&record)
            .map_err(|_| "read original standing Record")?;
        let guid = tree.guid().to_string();
        if tree.type_name() != "SReputationStandingParams"
            || row.pointer("/guid/value").and_then(Value::as_str) != Some(guid.as_str())
        {
            return Err("original standing identity differs".into());
        }
        let witness = Witness::new(forge, &tree, origins, texts);
        compare_fields(&witness, tree.root(), row)?;
        let drift = field(row, "driftTimeHours")?;
        witness.occurrence(tree.field("driftTimeHours"), drift)?;
        source_singles += usize::from(check_drift(drift, tree.field("driftTimeHours"))?);
    }
    assert!(
        positions == expected,
        "every physical reputation standing retained"
    );
    if source_singles == 0 {
        return Err("configured Build contains no traced standing drift Single".into());
    }
    Ok((positions.len(), source_singles))
}

fn compare_fields<'source>(
    witness: &Witness<'_, '_, 'source>,
    source: &StructValue<'source>,
    row: &Value,
) -> TestResult {
    for name in ["name", "description", "icon", "missionBoardIcon", "gated"] {
        witness.scalar(source.field(name), field(row, name)?)?;
    }
    for name in ["displayName", "perkDescription"] {
        witness.locale(source.field(name), field(row, name)?)?;
    }
    for name in ["minReputation", "driftReputation"] {
        let original = source.field(name);
        let cell = field(row, name)?;
        witness.occurrence(original, cell)?;
        verify_number(original, cell)?;
    }
    Ok(())
}

fn check_drift(drift: &Value, original: Option<&Original<'_>>) -> TestResult<bool> {
    match (original, drift.get("state").and_then(Value::as_str)) {
        (Some(Original::Single(original)), state) => {
            if state != Some("present") {
                return Err("original standing drift Single is not present".into());
            }
            check_single(drift, *original)?;
            Ok(true)
        }
        (None, Some("absent"))
        | (Some(Original::Null), Some("null"))
        | (Some(Original::Error(_)), Some("unreadable"))
        | (Some(_), Some("invalid" | "unsupported")) => Ok(false),
        _ => Err("original standing drift state differs".into()),
    }
}

fn check_single(drift: &Value, original: f32) -> TestResult {
    let bits = format!("{:08x}", original.to_bits());
    if drift.pointer("/value/rawBits").and_then(Value::as_str) != Some(&bits) {
        return Err("original standing drift word differs".into());
    }
    let finite = drift
        .pointer("/value/finite")
        .ok_or("finite drift projection")?;
    if original.is_finite() {
        let value = finite.to_string().parse::<f32>()?;
        if value.to_bits() != original.to_bits() {
            return Err("finite standing drift hours differ".into());
        }
    } else if !finite.is_null() {
        return Err("nonfinite standing drift has a decimal projection".into());
    }
    Ok(())
}
