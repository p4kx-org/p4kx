// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::Value;

use super::{Dumps, Findings, elements};

const CATEGORY: &str = "contract reward references";

/// Accounts for published reward occurrences without assuming an unproven source mapping.
pub(super) fn report(dumps: &Dumps, findings: &mut Findings) {
    for list in ["contracts", "legacyContracts"] {
        for contract in elements(&dumps.merged[list]) {
            let ident = contract["id"].as_str().unwrap_or("<missing contract id>");
            if !contract["rewardUEC"].is_null() {
                findings.uncomparable(
                    CATEGORY,
                    "rewardUEC",
                    "the reference generator's currency and reward formula are unproven",
                );
            }
            success(contract, &dumps.merged, ident, findings);
            members(
                &contract["factionRewards_fail"],
                ident,
                "failureReputation",
                findings,
            );
        }
    }
}

fn success(contract: &Value, merged: &Value, ident: &str, findings: &mut Findings) {
    if !contract["factionRewards"].is_null() {
        members(
            &contract["factionRewards"],
            ident,
            "successReputation",
            findings,
        );
        return;
    }
    let index = &contract["factionRewardsIndex"];
    if index.is_null() {
        return;
    }
    let pool = index
        .as_u64()
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| merged["factionRewardsPools"].as_array()?.get(index));
    if let Some(pool) = pool {
        if pool.is_null() {
            findings.check("reference input", false, || {
                format!("{ident}: the selected reputation pool is null")
            });
        } else {
            members(pool, ident, "successReputation", findings);
        }
    } else {
        findings.check("reference input", false, || {
            format!("{ident}: invalid or out-of-range factionRewardsIndex")
        });
    }
}

fn members(value: &Value, ident: &str, field: &'static str, findings: &mut Findings) {
    if value.is_null() {
        return;
    }
    if let Some(members) = value.as_array() {
        for member in members {
            findings.check("reference input", member.is_object(), || {
                format!("{ident}: {field} contains a non-object member")
            });
            findings.uncomparable(
                CATEGORY,
                field,
                "the reference outcome-to-authored-result mapping is unproven",
            );
        }
    } else {
        findings.check("reference input", false, || {
            format!("{ident}: {field} is not an array")
        });
    }
}
