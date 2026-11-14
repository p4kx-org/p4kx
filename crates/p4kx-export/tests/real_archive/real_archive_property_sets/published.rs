// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::Value;

use super::OracleResult;
use super::error::{Failure, OutputFault};
use crate::real_archive_contracts::v3;

type OwnerRows<'a> = HashMap<&'a str, Vec<&'a Value>>;

/// Published rows indexed without interpreting their producer's failure claims.
pub(super) struct Published<'a> {
    pub(super) sets: &'a [Value],
    pub(super) contracts: &'a [Value],
    by_owner: HashMap<u64, OwnerRows<'a>>,
    pub(super) by_guid: HashMap<&'a str, &'a Value>,
}

impl<'a> Published<'a> {
    pub(super) fn read(document: &'a Value) -> OracleResult<Self> {
        let sets = document
            .get("missionPropertySets")
            .and_then(Value::as_array)
            .ok_or(Failure::OutputParse(OutputFault::MissingSets))?;
        let malformed_contract = |_| Failure::OutputParse(OutputFault::MissingContracts);
        let contracts = v3::rows(document).map_err(malformed_contract)?;
        let projections = v3::Projections::read(document).map_err(malformed_contract)?;
        let mut by_owner: HashMap<u64, OwnerRows<'a>> = HashMap::new();
        for row in contracts {
            let (record, guid) = contract_owner(&projections, row).map_err(malformed_contract)?;
            by_owner
                .entry(record)
                .or_default()
                .entry(guid)
                .or_default()
                .push(row);
        }
        let by_guid: HashMap<_, _> = sets
            .iter()
            .map(|set| {
                Ok((
                    set["guid"]
                        .as_str()
                        .ok_or(Failure::OutputParse(OutputFault::SetIdentity))?,
                    set,
                ))
            })
            .collect::<OracleResult<_>>()?;
        if by_guid.len() != sets.len() {
            return Err(Failure::OutputParse(OutputFault::DuplicateSetIdentity));
        }
        Ok(Self {
            sets,
            contracts,
            by_owner,
            by_guid,
        })
    }

    pub(super) fn contracts_of(&self, record: u32, guid: &str) -> &[&'a Value] {
        self.by_owner
            .get(&u64::from(record))
            .and_then(|owners| owners.get(guid))
            .map_or(&[], Vec::as_slice)
    }
}

pub(super) fn contract_owner<'a>(
    projections: &v3::Projections<'a>,
    row: &Value,
) -> Result<(u64, &'a str), Box<dyn std::error::Error>> {
    let index = row
        .pointer("/source/recordIndex")
        .and_then(Value::as_u64)
        .ok_or("physical Generator record index")?;
    Ok((index, projections.header(row, "guid")?))
}
