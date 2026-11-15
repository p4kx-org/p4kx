// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

struct BoxSize {
    scu: &'static str,
    member: &'static str,
}

const BOX_SIZES: [BoxSize; 10] = [
    BoxSize {
        scu: "0.125",
        member: "one_eighthSCU",
    },
    BoxSize {
        scu: "0.25",
        member: "one_quarterSCU",
    },
    BoxSize {
        scu: "0.5",
        member: "one_halfSCU",
    },
    BoxSize {
        scu: "1",
        member: "oneSCU",
    },
    BoxSize {
        scu: "2",
        member: "twoSCU",
    },
    BoxSize {
        scu: "4",
        member: "fourSCU",
    },
    BoxSize {
        scu: "8",
        member: "eightSCU",
    },
    BoxSize {
        scu: "16",
        member: "sixteenSCU",
    },
    BoxSize {
        scu: "24",
        member: "twentyFourSCU",
    },
    BoxSize {
        scu: "32",
        member: "thirtyTwoSCU",
    },
];

/// A box-size Class with sourced values keyed by the exact decimal SCU size.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "CargoBoxSizesOf{T}", transform = every_field_required)]
pub(super) struct CargoBoxSizes<'source, T> {
    source_type: SourcedValue<&'source str>,
    by_scu: BTreeMap<&'static str, SourcedValue<T>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source, T> CargoBoxSizes<'source, T> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        read_cell: impl Fn(
            &Observed<'_, '_, '_, 'source>,
            &'static str,
            &mut Vec<FieldError>,
        ) -> Result<SourcedValue<T>, CatalogError>,
    ) -> Result<Self, CatalogError> {
        let mut by_scu = BTreeMap::new();
        let mut projected = Vec::with_capacity(BOX_SIZES.len());
        for size in BOX_SIZES {
            let cell = read_cell(&source, size.member, notes)?;
            projected.push((size.member, cell.state()));
            by_scu.insert(size.scu, cell);
        }
        Ok(Self {
            source_type: source.source_type_value()?,
            by_scu,
            additional_fields: source.authored_members_except(projected)?,
        })
    }
}
