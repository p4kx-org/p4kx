// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::Kind;
use p4kx_extraction::{DataForge, RecordTree, StructValue, TreeBuilder, Value as Original};
use serde_json::{Value, json};

use super::TestResult;
use crate::authored_definition_witness::{field, verify_array, verify_link, verify_state};
use crate::catalog_v3_consumer as consumer;
use crate::real_archive_auto_spawns::{original::Locations, provenance::Origins};
use crate::real_archive_canonical_texts::{TextDomain, TextIndex, TextReferenceShape};
use crate::real_archive_law_definitions::{verify_class, verify_scalar};

const SCOPE: &str = "SReputationScopeParams";
const STANDING: &str = "SReputationStandingParams";
const CONTEXT: &str = "SReputationContextUI";

pub(super) fn compare_definitions(
    forge: &DataForge<'_>,
    catalog: &Value,
    origins: &Origins<'_>,
    texts: &TextIndex<'_>,
) -> TestResult<(usize, usize)> {
    let scopes = compare_family(
        forge,
        catalog,
        Kind::ReputationScopes,
        SCOPE,
        origins,
        texts,
    )?;
    let contexts = compare_family(
        forge,
        catalog,
        Kind::ReputationContexts,
        CONTEXT,
        origins,
        texts,
    )?;
    Ok((scopes, contexts))
}

fn compare_family(
    forge: &DataForge<'_>,
    catalog: &Value,
    kind: Kind,
    expected_type: &str,
    origins: &Origins<'_>,
    texts: &TextIndex<'_>,
) -> TestResult<usize> {
    let mut expected: BTreeSet<_> = forge
        .records()
        .filter(|record| record.type_name().ok() == Some(expected_type))
        .map(|record| record.index())
        .collect();
    let mut trees = TreeBuilder::new();
    let rows = consumer::rows(catalog, kind)?;
    for row in rows {
        let index = u32::try_from(
            consumer::field(row, kind, "recordIndex")?
                .as_u64()
                .ok_or("physical reputation scope or context index")?,
        )?;
        assert!(
            expected.remove(&index),
            "one row per original reputation scope or context"
        );
        let source = forge
            .record(index)
            .ok_or("original reputation scope or context")?;
        assert!(
            consumer::identity(row, kind)? == source.guid().to_string(),
            "original reputation definition GUID"
        );
        assert!(
            consumer::present(field(row, "recordPath")?)?.as_str() == Some(source.record_path()?),
            "original reputation definition path"
        );
        let tree = trees
            .build(&source)
            .map_err(|_| "read original reputation scope or context")?;
        let witness = Witness::new(forge, &tree, origins, texts);
        match kind {
            Kind::ReputationScopes => witness.scope(tree.root(), row)?,
            Kind::ReputationContexts => witness.context(tree.root(), row)?,
            _ => return Err("reputation scope or context kind required".into()),
        }
    }
    assert!(
        expected.is_empty(),
        "every physical reputation scope or context retained"
    );
    Ok(rows.len())
}

pub(crate) struct Witness<'scope, 'catalog, 'source> {
    forge: &'source DataForge<'source>,
    locations: Locations<'source>,
    origins: &'scope Origins<'catalog>,
    texts: &'scope TextIndex<'catalog>,
    owner: u32,
}

impl<'scope, 'catalog, 'source> Witness<'scope, 'catalog, 'source> {
    pub(crate) fn new(
        forge: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        origins: &'scope Origins<'catalog>,
        texts: &'scope TextIndex<'catalog>,
    ) -> Self {
        Self {
            forge,
            locations: Locations::read(tree.root()),
            origins,
            texts,
            owner: tree.record_index(),
        }
    }

    pub(super) fn faction(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        for name in [
            "name",
            "background",
            "logo",
            "missionBoardLogo",
            "guildBanner",
            "hideInDelpihApp",
        ] {
            self.scalar(source.field(name), field(row, name)?)?;
        }
        self.scalar(source.field("isNPC"), field(row, "isNpc")?)?;
        self.occurrence(source.field("GEID"), field(row, "geid")?)?;
        self.locale(source.field("displayName"), field(row, "displayName")?)?;
        self.description(source.field("description"), field(row, "description")?)?;
        for name in ["hostilityParams", "alliedParams"] {
            self.owned(source.field(name), field(row, name)?, |source, row| {
                self.relation(source, row)
            })?;
        }
        self.properties(source, row)
    }

    fn scope(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        for name in ["scopeName", "icon"] {
            self.scalar(source.field(name), field(row, name)?)?;
        }
        for name in ["displayName", "description"] {
            self.locale(source.field(name), field(row, name)?)?;
        }
        self.owned(
            source.field("standingMap"),
            field(row, "standingMap")?,
            |source, row| {
                for name in ["reputationCeiling", "initialReputation"] {
                    let value = source.field(name);
                    let cell = field(row, name)?;
                    self.occurrence(value, cell)?;
                    verify_number(value, cell)?;
                }
                self.array(
                    source.field("standings"),
                    field(row, "standings")?,
                    |source, cell| self.link(source, cell, STANDING),
                )
            },
        )
    }

    fn context(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.scalar(
            source.field("sortOrderScope"),
            field(row, "sortOrderScope")?,
        )?;
        self.owned(
            source.field("primaryScopeContext"),
            field(row, "primaryScopeContext")?,
            |source, row| self.scope_context(source, row),
        )?;
        self.array(
            source.field("scopeContextList"),
            field(row, "scopeContextList")?,
            |source, cell| self.owned(source, cell, |source, row| self.scope_context(source, row)),
        )
    }

    fn scope_context(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.link(source.field("scope"), field(row, "scope")?, SCOPE)?;
        self.properties(source, row)
    }

    fn relation(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        for (name, target) in [("scope", SCOPE), ("standing", STANDING)] {
            self.link(source.field(name), field(row, name)?, target)?;
        }
        self.owned(
            source.field("markerParams"),
            field(row, "markerParams")?,
            |source, row| {
                self.scalar(source.field("icon"), field(row, "icon")?)?;
                self.description(source.field("description"), field(row, "description")?)
            },
        )
    }

    fn properties(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        self.array(
            source.field("propertiesBB"),
            field(row, "propertiesBb")?,
            |source, cell| {
                self.owned(source, cell, |source, row| {
                    self.scalar(source.field("name"), field(row, "name")?)?;
                    self.owned(
                        source.field("dynamicProperty"),
                        field(row, "dynamicProperty")?,
                        |source, row| self.property(source, row),
                    )
                })
            },
        )
    }

    fn property(&self, source: &StructValue<'source>, row: &Value) -> TestResult {
        match source.type_name() {
            "SBBDynamicPropertyLocString" => {
                self.locale(source.field("value"), field(row, "value")?)
            }
            "SBBDynamicPropertyBool"
            | "SBBDynamicPropertyFloat"
            | "SBBDynamicPropertyInt"
            | "SBBDynamicPropertyString" => {
                self.scalar(source.field("value"), field(row, "value")?)
            }
            _ => {
                assert!(
                    row.get("kind").and_then(Value::as_str) == Some("authored"),
                    "unclassified original blackboard body remains authored"
                );
                Ok(())
            }
        }
    }

    pub(crate) fn scalar(&self, source: Option<&Original<'source>>, cell: &Value) -> TestResult {
        self.occurrence(source, cell)?;
        match source {
            Some(Original::String(text)) => {
                assert!(
                    consumer::present(cell)?.as_str() == Some(*text),
                    "exact original string"
                );
                Ok(())
            }
            _ => verify_scalar(source, cell),
        }
    }

    pub(crate) fn link(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
        target: &str,
    ) -> TestResult {
        self.occurrence(source, cell)?;
        verify_link(self.forge, target, source, cell)
    }

    pub(crate) fn array(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
        verify: impl Fn(Option<&Original<'source>>, &Value) -> TestResult,
    ) -> TestResult {
        self.occurrence(source, cell)?;
        verify_array(source, cell, verify)
    }

    pub(crate) fn owned(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
        verify: impl FnOnce(&StructValue<'source>, &Value) -> TestResult,
    ) -> TestResult {
        self.occurrence(source, cell)?;
        verify_class(source, cell, verify)
    }

    pub(crate) fn locale(&self, source: Option<&Original<'source>>, cell: &Value) -> TestResult {
        self.occurrence(source, cell)?;
        let Some(Original::Locale(key) | Original::String(key)) = source else {
            return verify_state(source, cell);
        };
        let target = self
            .texts
            .reference(cell, TextDomain::GameText, TextReferenceShape::Required)?
            .ok_or("original protected reputation text target")?;
        verify_key(target.source(), key);
        Ok(())
    }

    fn description(&self, source: Option<&Original<'source>>, cell: &Value) -> TestResult {
        self.occurrence(source, cell)?;
        match source {
            Some(Original::String(text)) => {
                let row = consumer::present(cell)?;
                assert!(
                    row.get("type").and_then(Value::as_str) == Some("literal")
                        && row.get("text").and_then(Value::as_str) == Some(*text),
                    "original literal reputation description"
                );
            }
            Some(Original::Locale(key)) => {
                let row = consumer::present(cell)?;
                assert!(
                    row.get("type").and_then(Value::as_str) == Some("localized"),
                    "original localized reputation description"
                );
                let mut reference = cell.clone();
                *reference.get_mut("value").ok_or("sourced text value")? =
                    field(row, "reference")?.clone();
                let target = self
                    .texts
                    .reference(
                        &reference,
                        TextDomain::GameText,
                        TextReferenceShape::Required,
                    )?
                    .ok_or("original protected description text target")?;
                verify_key(target.source(), key);
            }
            _ => return verify_state(source, cell),
        }
        Ok(())
    }

    pub(crate) fn occurrence(
        &self,
        source: Option<&Original<'source>>,
        cell: &Value,
    ) -> TestResult {
        if let Some(value) = source.filter(|value| !matches!(value, Original::Error(_))) {
            let part = if matches!(value, Original::Struct(_)) {
                "$type"
            } else {
                "$value"
            };
            self.origins
                .occurrence(cell, self.owner, self.locations.ordinal(value)?, part)?;
        }
        Ok(())
    }
}

fn verify_key(target: &Value, expected: &str) {
    assert!(
        target.pointer("/key/value").and_then(Value::as_str) == Some(expected),
        "exact original reputation localization key"
    );
}

pub(crate) fn verify_number(source: Option<&Original<'_>>, cell: &Value) -> TestResult {
    let (kind, expected) = match source {
        Some(Original::Int8(value)) => ("int8", json!(value)),
        Some(Original::Int16(value)) => ("int16", json!(value)),
        Some(Original::Int32(value)) => ("int32", json!(value)),
        Some(Original::Int64(value)) => ("int64", json!(value.to_string())),
        Some(Original::UInt8(value)) => ("uint8", json!(value)),
        Some(Original::UInt16(value)) => ("uint16", json!(value)),
        Some(Original::UInt32(value)) => ("uint32", json!(value)),
        Some(Original::UInt64(value)) => ("uint64", json!(value.to_string())),
        _ => return verify_state(source, cell),
    };
    let projected = consumer::present(cell)?;
    assert!(
        projected.get("type").and_then(Value::as_str) == Some(kind)
            && projected.get("value") == Some(&expected),
        "exact original reputation integer width and value"
    );
    Ok(())
}
