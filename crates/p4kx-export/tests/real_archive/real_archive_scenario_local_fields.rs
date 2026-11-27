// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, StructValue, Value as Original};
use serde_json::Value;

use super::{Locations, Origins, TestResult, field, record_index, value_index};
use crate::authored_definition_witness::{verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;

const PARAMS: &str = "localGeneratorParams";
const PARAMS_TYPE: &str = "LocalContractGeneratorParams";
const DEFAULTS: &str = "defaultAvailability";
const BOARD: &str = "missionBoardAvailability";
const TAGS: &str = "parentLocationGeneralTags";
const FACTIONS: &str = "factionIdentities";

pub(super) fn handlers<'read, 'source>(
    tree: &'read RecordTree<'source>,
) -> impl Iterator<Item = &'read Original<'source>> {
    let slots: &[Original<'source>] = match tree.field("generators") {
        Some(Original::Array(slots)) => slots,
        _ => &[],
    };
    slots.iter().filter(|slot| {
        matches!(slot, Original::Struct(handler) if handler.field(PARAMS).is_some()
            || matches!(handler.field(DEFAULTS), Some(Original::Struct(defaults)) if defaults.field(BOARD).is_some()))
    })
}

pub(super) fn populated(slot: &Original<'_>) -> bool {
    matches!(slot, Original::Struct(handler)
        if matches!(handler.field(PARAMS), Some(Original::Struct(params)) if params.type_name() == PARAMS_TYPE))
}

pub(super) fn compare(
    forge: &DataForge<'_>,
    check: &Check<'_, '_, '_>,
    tree: &RecordTree<'_>,
    rows: &[&Value],
) -> TestResult {
    let mut expected = handlers(tree);
    for row in rows {
        let raw = expected.next().ok_or("extra local Handler configuration")?;
        let Original::Struct(handler) = raw else {
            return Err("original Handler structure".into());
        };
        let source = field(field(row, "source")?, "handler")?;
        assert_eq!(
            value_index(source)?,
            check.locations.ordinal(raw)?,
            "the original Handler occurrence"
        );
        check.owned(
            handler.field(PARAMS),
            field(row, PARAMS)?,
            PARAMS_TYPE,
            |params, row| {
                check.enumeration(params.field("scope"), field(row, "scope")?)?;
                check.links(
                    forge,
                    "FactionIdentity",
                    params.field(FACTIONS),
                    field(row, FACTIONS)?,
                )?;
                check.owned(
                    params.field(TAGS),
                    field(row, TAGS)?,
                    "TagsDNF",
                    |tags, row| check.terms(forge, tags.field("terms"), field(row, "terms")?),
                )
            },
        )?;
        check.owned(
            handler.field(DEFAULTS),
            field(row, DEFAULTS)?,
            "ContractAvailability",
            |availability, row| check.enumeration(availability.field(BOARD), field(row, BOARD)?),
        )?;
    }
    assert!(
        expected.next().is_none(),
        "all original selected Handler configurations"
    );
    Ok(())
}

pub(super) struct Check<'read, 'catalog, 'source> {
    origins: &'read Origins<'catalog>,
    locations: Locations<'source>,
    owner: u32,
}

impl<'read, 'catalog, 'source> Check<'read, 'catalog, 'source> {
    pub(super) fn new(origins: &'read Origins<'catalog>, tree: &RecordTree<'source>) -> Self {
        Self {
            origins,
            locations: Locations::read(tree.root()),
            owner: tree.record_index(),
        }
    }

    fn origin(&self, row: &Value, source: &Original<'source>, part: &str) -> TestResult {
        self.origins
            .occurrence(row, self.owner, self.locations.ordinal(source)?, part)
    }

    fn owned(
        &self,
        source: Option<&Original<'source>>,
        row: &Value,
        kind: &str,
        compare: impl FnOnce(&StructValue<'source>, &Value) -> TestResult,
    ) -> TestResult {
        match source {
            Some(raw @ Original::Struct(data)) if data.type_name() == kind => {
                self.origin(row, raw, "$type")?;
                compare(data, consumer::present(row)?)
            }
            Some(raw @ Original::Null) => {
                verify_state(source, row)?;
                self.origin(row, raw, "$value")
            }
            _ => verify_state(source, row),
        }
    }

    fn enumeration(&self, source: Option<&Original<'source>>, row: &Value) -> TestResult {
        if let Some(raw @ Original::Enum(expected)) = source {
            assert!(
                consumer::present(row)?.as_str() == Some(expected),
                "the original enum label is retained"
            );
            self.origin(row, raw, "$value")
        } else {
            verify_state(source, row)
        }
    }

    fn terms(
        &self,
        forge: &DataForge<'_>,
        source: Option<&Original<'source>>,
        row: &Value,
    ) -> TestResult {
        if let Some(raw @ Original::Array(_)) = source {
            self.origin(row, raw, "$value")?;
        }
        verify_array(source, row, |value, row| {
            self.owned(value, row, "TagsDNFTerm", |term, row| {
                for name in ["positiveTags", "negativeTags"] {
                    self.links(forge, "Tag", term.field(name), field(row, name)?)?;
                }
                Ok(())
            })
        })
    }

    pub(super) fn links(
        &self,
        forge: &DataForge<'_>,
        target: &str,
        source: Option<&Original<'source>>,
        row: &Value,
    ) -> TestResult {
        if let Some(raw @ Original::Array(_)) = source {
            self.origin(row, raw, "$value")?;
        }
        verify_array(source, row, |value, row| {
            verify_link(forge, target, value, row)?;
            if let Some(raw @ Original::Reference(link)) = value {
                self.origin(row, raw, "$value")?;
                let index = field(consumer::present(row)?, "targetRecordIndex")?;
                assert!(
                    record_index(consumer::present(index)?)? == link.record_index(),
                    "exact physical Reference target"
                );
                self.origins.reference_field(
                    index,
                    self.owner,
                    self.locations.ordinal(raw)?,
                    link.record_index(),
                    Some("recordIndex"),
                )?;
            }
            Ok(())
        })
    }
}
