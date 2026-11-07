// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::mission_property_fields as fields;
use crate::mission_property_set::MissionPropertyLevel;
use crate::record_read::ReadError;

/// Visits every authored Generator scope independently of derived contract-row selection.
pub(crate) fn visit<'t, 'f>(
    root: &'t StructValue<'f>,
    mut visit: impl FnMut(MissionPropertyLevel, Option<&'t StructValue<'f>>) -> Result<(), ReadError>,
) -> Result<(), ReadError> {
    for value in array(root, "generators")? {
        let Some(handler) = fields::structure(value, "generators[]")? else {
            continue;
        };
        visit(
            MissionPropertyLevel::Handler,
            fields::parameter(handler, "contractParams")?,
        )?;
        for container in [
            "contracts",
            "introContracts",
            "legacyContracts",
            "PVPBountyContract",
        ] {
            for value in array(handler, container)? {
                let Some(contract) = fields::structure(value, "contracts[]")? else {
                    continue;
                };
                visit(
                    MissionPropertyLevel::Contract,
                    fields::parameter(contract, "paramOverrides")?,
                )?;
                for value in array(contract, "subContracts")? {
                    let Some(sub) = fields::structure(value, "subContracts[]")? else {
                        continue;
                    };
                    visit(
                        MissionPropertyLevel::SubContract,
                        fields::parameter(sub, "paramOverrides")?.or(Some(sub)),
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn array<'t, 'f>(
    parent: &'t StructValue<'f>,
    field: &'static str,
) -> Result<&'t [Value<'f>], ReadError> {
    match parent.field(field) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(value) => Err(fields::malformed(field, value, "a nullable source array")),
    }
}
