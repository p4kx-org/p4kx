// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use serde_json::Value;

use super::OracleResult;
use super::error::{Failure, OutputFault};

/// Published rows indexed without interpreting their producer's failure claims.
pub(super) struct Published<'a> {
    pub(super) sets: &'a [Value],
    pub(super) contracts: &'a [Value],
    pub(super) by_guid: HashMap<&'a str, &'a Value>,
}

impl<'a> Published<'a> {
    pub(super) fn read(document: &'a Value) -> OracleResult<Self> {
        let sets = document
            .get("missionPropertySets")
            .and_then(Value::as_array)
            .ok_or(Failure::OutputParse(OutputFault::MissingSets))?;
        let contracts = document
            .get("contracts")
            .and_then(Value::as_array)
            .ok_or(Failure::OutputParse(OutputFault::MissingContracts))?;
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
            by_guid,
        })
    }
}
