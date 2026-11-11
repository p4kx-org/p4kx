// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::mission_property_fields as fields;
use crate::mission_property_set::MissionPropertyLevel;
use crate::record_read::ReadError;
use crate::source_fields::FieldCapture;

/// Visits every authored Generator scope independently of derived contract-row selection.
pub(crate) fn visit<'t, 'f>(
    root: &'t StructValue<'f>,
    capture: &dyn FieldCapture<'f>,
    mut visit: impl FnMut(MissionPropertyLevel, Option<&'t StructValue<'f>>) -> Result<(), ReadError>,
) -> Result<(), ReadError> {
    for value in array(root, "generators", capture)? {
        let Some(handler) = fields::structure(root, value, "generators[]", capture)? else {
            continue;
        };
        visit(
            MissionPropertyLevel::Handler,
            fields::parameter(handler, "contractParams", capture)?,
        )?;
        for container in [
            "contracts",
            "introContracts",
            "legacyContracts",
            "PVPBountyContract",
        ] {
            for value in array(handler, container, capture)? {
                let Some(contract) = fields::structure(handler, value, "contracts[]", capture)?
                else {
                    continue;
                };
                visit(
                    MissionPropertyLevel::Contract,
                    fields::parameter(contract, "paramOverrides", capture)?,
                )?;
                for value in array(contract, "subContracts", capture)? {
                    let Some(sub) = fields::structure(contract, value, "subContracts[]", capture)?
                    else {
                        continue;
                    };
                    visit(
                        MissionPropertyLevel::SubContract,
                        fields::parameter(sub, "paramOverrides", capture)?.or(Some(sub)),
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
    capture: &dyn FieldCapture<'f>,
) -> Result<&'t [Value<'f>], ReadError> {
    capture.check_parent(parent)?;
    match parent.field(field) {
        None | Some(Value::Null) => Ok(&[]),
        Some(Value::Array(values)) => Ok(values),
        Some(value) => Err(fields::malformed(
            parent,
            field,
            value,
            "a nullable source array",
            capture,
        )),
    }
}
