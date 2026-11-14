// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "real_archive_blueprint_research.rs"]
mod research;

use std::collections::{HashMap, HashSet};
use std::error::Error;

use p4kx_export::Kind;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult<T> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
struct Owner<'catalog> {
    input: &'catalog str,
    index: u64,
}

impl<'catalog> Owner<'catalog> {
    fn read(row: &'catalog Value, index: &Value) -> TestResult<Self> {
        Ok(Self {
            input: row["inputRef"].as_str().ok_or("physical Blueprint input")?,
            index: index.as_u64().ok_or("physical Blueprint index")?,
        })
    }
}

/// Borrowed physical Blueprint rows indexed once for installed-Archive comparisons.
pub(crate) struct Blueprints<'catalog> {
    rows: Vec<Blueprint<'catalog>>,
}

impl<'catalog> Blueprints<'catalog> {
    pub(crate) fn read(catalog: &'catalog Value) -> TestResult<Self> {
        let definitions = catalog_v3_consumer::rows(catalog, Kind::Blueprints)?;
        let mut presentations = HashMap::new();
        for row in array(catalog, "/projections/blueprintPresentation/rows")? {
            let owner = &row["blueprint"];
            let owner = Owner::read(owner, &owner["recordIndex"])?;
            if presentations.insert(owner, row).is_some() {
                return Err("duplicate physical Blueprint presentation".into());
            }
        }
        let mut tiers = HashMap::<_, Vec<_>>::new();
        for row in array(catalog, "/configurations/blueprintTiers/rows")? {
            let owner = &row["blueprint"];
            let owner = Owner::read(owner, &owner["recordIndex"])?;
            tiers.entry(owner).or_default().push(row);
        }
        let mut rows = Vec::with_capacity(definitions.len());
        for definition in definitions {
            let owner = Owner::read(
                definition,
                definition
                    .pointer("/recordIndex/value")
                    .ok_or("physical Blueprint record index")?,
            )?;
            rows.push(Blueprint {
                definition,
                presentation: presentations
                    .remove(&owner)
                    .ok_or("missing physical Blueprint presentation")?,
                tiers: tiers.remove(&owner).unwrap_or_default(),
            });
        }
        assert!(
            presentations.is_empty() && tiers.is_empty(),
            "every contextual row has one actual definition"
        );
        Ok(Self { rows })
    }

    pub(crate) const fn rows(&self) -> &[Blueprint<'catalog>] {
        self.rows.as_slice()
    }

    pub(crate) fn named(&self, name: &str) -> TestResult<&Blueprint<'catalog>> {
        self.rows
            .iter()
            .find(|blueprint| {
                blueprint
                    .definition
                    .pointer("/className/value")
                    .and_then(Value::as_str)
                    == Some(name)
            })
            .ok_or_else(|| format!("original Blueprint {name}").into())
    }
}

pub(crate) struct Blueprint<'catalog> {
    pub(crate) definition: &'catalog Value,
    pub(crate) presentation: &'catalog Value,
    pub(crate) tiers: Vec<&'catalog Value>,
}

impl<'catalog> Blueprint<'catalog> {
    pub(crate) fn costs(&self) -> impl Iterator<Item = &'catalog Value> + '_ {
        self.tiers
            .iter()
            .flat_map(|tier| tier["costs"].as_array().into_iter().flatten())
    }

    pub(crate) fn first_cost(&self) -> TestResult<&'catalog Value> {
        let tier = self.tiers.first().ok_or("first Blueprint tier")?;
        first_cost(tier, &tier["mandatory"])
    }

    pub(crate) fn ingredients(&self) -> TestResult<Vec<&'catalog Value>> {
        let found = array(self.presentation, "/ingredients")?;
        let Some(tier) = self.tiers.first() else {
            assert_eq!(
                found.len(),
                0,
                "a Blueprint without a tier has no ingredients"
            );
            return Ok(Vec::new());
        };
        found
            .iter()
            .map(|ingredient| first_cost(tier, &ingredient["cost"]))
            .collect()
    }

    pub(crate) fn check_ingredients(&self) -> TestResult<()> {
        let tier = self.tiers.first().ok_or("first Blueprint tier")?;
        assert_eq!(
            self.presentation.get("firstTier"),
            tier.get("source"),
            "the projection links the first source tier"
        );
        let mut work = Vec::new();
        for reference in array(tier, "/optional")?.iter().rev() {
            work.push((reference, None));
        }
        if !tier["mandatory"].is_null() {
            work.push((&tier["mandatory"], None));
        }
        let mut expected = Vec::new();
        let mut visited = HashSet::new();
        while let Some((source, slot)) = work.pop() {
            assert!(
                visited.insert(occurrence(source)?),
                "one cost occurrence cannot recur in a tier"
            );
            let cost = first_cost(tier, source)?;
            let meaning = &cost["meaning"];
            if meaning["type"] == "select" {
                let own = self.has_name(meaning)?;
                let slot = if own { Some(source) } else { slot };
                for option in array(meaning, "/options")?.iter().rev() {
                    work.push((option, slot));
                }
            } else {
                expected.push((source, slot));
            }
        }
        let actual = array(self.presentation, "/ingredients")?;
        assert_eq!(
            actual.len(),
            expected.len(),
            "every derived ingredient has one source cost"
        );
        for (actual, (cost, slot)) in actual.iter().zip(expected) {
            assert_eq!(
                actual.get("cost"),
                Some(cost),
                "the ingredient links the actual source cost"
            );
            assert_eq!(
                actual.get("slot"),
                Some(slot.unwrap_or(&Value::Null)),
                "the ingredient links the nearest named slot"
            );
        }
        Ok(())
    }

    fn has_name(&self, meaning: &Value) -> TestResult<bool> {
        if meaning
            .pointer("/displayName/value")
            .is_some_and(Value::is_string)
        {
            return Ok(true);
        }
        if meaning["debugName"].is_null() {
            return Ok(false);
        }
        let ordinal = occurrence(&meaning["debugName"])?.parse::<usize>()?;
        let values = array(self.definition, "/source/values")?;
        let original = values.get(ordinal).ok_or("original debug-name slot")?;
        let value = original
            .pointer("/value/value/value")
            .ok_or("original debug-name payload")?;
        let text = value.as_str().or_else(|| value["sourceText"].as_str());
        Ok(text.is_some_and(|text| !text.is_empty()))
    }
}

fn first_cost<'catalog>(tier: &'catalog Value, source: &Value) -> TestResult<&'catalog Value> {
    let ordinal = occurrence(source)?;
    array(tier, "/costs")?
        .iter()
        .find(|cost| cost.pointer("/source/valueIndex").and_then(Value::as_str) == Some(ordinal))
        .ok_or_else(|| format!("actual Blueprint cost occurrence {ordinal}").into())
}

fn occurrence(reference: &Value) -> TestResult<&str> {
    reference["valueIndex"]
        .as_str()
        .ok_or_else(|| "original value occurrence".into())
}

fn array<'catalog>(value: &'catalog Value, path: &str) -> TestResult<&'catalog [Value]> {
    value
        .pointer(path)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| path.to_owned().into())
}
