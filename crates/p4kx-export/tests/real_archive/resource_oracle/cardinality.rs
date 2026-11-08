// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::Guid;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Failure, Result};

pub(super) const MAX_ROWS: usize = 16_384;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Counts {
    rows: usize,
    unique_guids: usize,
    distinct_subtypes_including_null: usize,
    null_subtypes: usize,
    shared_subtype_groups: usize,
    rows_in_shared_subtypes: usize,
}

pub(super) fn check(catalog: &Value) -> Result<Counts> {
    let rows = catalog
        .get("commodities")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("commodity-array"))?;
    if rows.is_empty() || rows.len() > MAX_ROWS {
        return Err(Failure::check("commodity-row-budget"));
    }
    let mut guids = BTreeSet::new();
    let mut subtypes = BTreeMap::<Option<Guid>, usize>::new();
    for row in rows {
        if !guids.insert(guid(row.get("guid"))?) {
            return Err(Failure::check("duplicate-commodity-guid"));
        }
        let subtype = match row.get("subtypeGuid") {
            Some(Value::Null) => None,
            value => Some(guid(value)?),
        };
        *subtypes.entry(subtype).or_default() += 1;
    }
    let shared: Vec<_> = subtypes
        .iter()
        .filter(|(guid, count)| guid.is_some() && **count > 1)
        .collect();
    Ok(Counts {
        rows: rows.len(),
        unique_guids: guids.len(),
        distinct_subtypes_including_null: subtypes.len(),
        null_subtypes: subtypes.get(&None).copied().unwrap_or_default(),
        shared_subtype_groups: shared.len(),
        rows_in_shared_subtypes: shared.iter().map(|(_, count)| **count).sum(),
    })
}

fn guid(value: Option<&Value>) -> Result<Guid> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| Failure::check("commodity-link-type"))?
        .parse()
        .map_err(|error| Failure::source("commodity-guid-format", error))
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::Guid;

    use super::{Counts, Result, check};
    use serde_json::json;

    fn id(byte: u8) -> String {
        Guid::from_stored([byte; 16]).to_string()
    }

    #[test]
    fn primary_identity_and_shared_nullable_links_have_separate_counts() -> Result<()> {
        let catalog = json!({"commodities": [
            {"guid":id(1),"subtypeGuid":id(9)}, {"guid":id(2),"subtypeGuid":id(9)},
            {"guid":id(3),"subtypeGuid":id(8)}, {"guid":id(4),"subtypeGuid":null}
        ]});
        assert_eq!(
            check(&catalog)?,
            Counts {
                rows: 4,
                unique_guids: 4,
                distinct_subtypes_including_null: 3,
                null_subtypes: 1,
                shared_subtype_groups: 1,
                rows_in_shared_subtypes: 2
            }
        );
        Ok(())
    }

    #[test]
    fn duplicate_primary_guids_and_missing_nullable_fields_fail() {
        for rows in [
            json!([{"guid":id(1),"subtypeGuid":null},{"guid":id(1),"subtypeGuid":null}]),
            json!([{"guid":id(1)}]),
        ] {
            assert!(
                check(&json!({"commodities":rows})).is_err(),
                "identity and field presence are mandatory"
            );
        }
    }
}
