// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The text check of the verification, per kind: the text the catalog holds for a key equals the
//! text the language's `global.ini` holds for it, a key the file lacks leaves that language `null`,
//! and, on the baselined Archive, the keys a file lacks stay within a baseline per kind. The
//! translation completeness of the game is measured, never asserted as a global share.
//!
//! The expectation is built here from the file's lines with the game's rules (the key without its
//! `@`, ASCII case ignored, a `key,P=` line standing for `key`, the first non-empty text, `\n`
//! decoded and the text trimmed) and does not use the exporter's lookup.

use std::borrow::Cow;
use std::collections::HashMap;

use p4kx_export::{Kind, Language, catalog_schema};
use p4kx_extraction::{Archive, LocalizationSet, TextSet, display_text, is_placeholder_key};
use serde_json::Value;

use crate::real_archive_verification::{TestResult, list, print_line};

/// The baseline of a kind: the largest share of its keyed texts the English and the German
/// `global.ini` may lack, as measured on LIVE and rounded up to the hundredth. A kind with keyed
/// texts that is not listed has a baseline of zero, so a new kind with untranslated keys names
/// itself; a German baseline of 1.0 leaves only the comparison with the file for that kind.
#[derive(Clone, Copy)]
struct Baseline {
    kind: Kind,
    english: f64,
    german: f64,
}

const fn baseline_of(kind: Kind, english: f64, german: f64) -> Baseline {
    Baseline {
        kind,
        english,
        german,
    }
}

const BASELINES: [Baseline; 18] = [
    baseline_of(Kind::Manufacturers, 0.01, 0.06),
    baseline_of(Kind::Items, 0.01, 0.38),
    baseline_of(Kind::Ships, 0.01, 0.31),
    baseline_of(Kind::Commodities, 0.0, 0.16),
    baseline_of(Kind::Blueprints, 0.0, 1.0),
    baseline_of(Kind::ResourceGroups, 0.0, 0.0),
    baseline_of(Kind::GameplayProperties, 0.0, 0.97),
    baseline_of(Kind::StarmapObjects, 0.04, 0.67),
    baseline_of(Kind::Contracts, 0.02, 0.53),
    baseline_of(Kind::RefiningProcesses, 0.0, 1.0),
    baseline_of(Kind::MineableCompositions, 0.01, 0.16),
    baseline_of(Kind::FactionReputations, 0.0, 0.73),
    baseline_of(Kind::ReputationScopes, 0.24, 0.31),
    baseline_of(Kind::ReputationStandings, 0.08, 0.21),
    baseline_of(Kind::MissionTypes, 0.0, 0.57),
    baseline_of(Kind::MissionLocationTemplates, 0.09, 0.99),
    baseline_of(Kind::ContractTemplates, 0.01, 0.99),
    baseline_of(Kind::MissionBrokerEntries, 0.04, 1.0),
];

/// The start of a mission token (`~mission(Contractor|Key)`) in a contract text; the exporter
/// replaces it by a placeholder such as `[CONTRACTOR]`, so only the presence of such a text is
/// compared.
const MISSION_TOKEN: &str = "~mission(";

/// How many keys of a kind a failure message names.
const SAMPLE: usize = 3;

/// The texts one language's file holds, by lower-case key without `@`.
struct Oracle<'a>(HashMap<String, Cow<'a, str>>);

impl<'a> Oracle<'a> {
    fn of(texts: &TextSet<'a>, language: Language) -> Result<Self, String> {
        let ini = texts
            .ini(language)
            .ok_or_else(|| format!("the Archive holds {language}"))?;
        let map = ini
            .iter()
            .filter_map(|(key, text)| {
                let shown = display_text(text);
                (!shown.is_empty()).then(|| (key.to_ascii_lowercase(), shown))
            })
            .collect();
        Ok(Self(map))
    }

    fn text(&self, key: &str) -> Option<&str> {
        let key = key.strip_prefix('@').unwrap_or(key).to_ascii_lowercase();
        self.0.get(&key).map(AsRef::as_ref)
    }
}

/// What the catalog holds for the keyed texts of one kind in one language.
#[derive(Debug, Default)]
struct Coverage {
    /// Texts the file holds and the catalog has, equal.
    resolved: usize,
    /// Texts the file lacks and the catalog has as `null`.
    fallback: usize,
    /// Texts where the catalog contradicts the file: the exporter is wrong.
    contradictions: usize,
    /// The first keys of the contradictions and of the fallbacks.
    contradicted: Vec<String>,
    lacking: Vec<String>,
}

impl Coverage {
    fn count(&mut self, oracle: &Oracle<'_>, key: &str, shown: Option<&Value>) {
        match (oracle.text(key), shown.and_then(Value::as_str)) {
            (Some(expected), Some(shown))
                if expected == shown || expected.contains(MISSION_TOKEN) =>
            {
                self.resolved += 1;
            }
            (None, None) => {
                self.fallback += 1;
                if self.lacking.len() < SAMPLE {
                    self.lacking.push(key.to_owned());
                }
            }
            (expected, shown) => {
                self.contradictions += 1;
                if self.contradicted.len() < SAMPLE {
                    self.contradicted
                        .push(format!("{key}: file {expected:?}, catalog {shown:?}"));
                }
            }
        }
    }

    const fn total(&self) -> usize {
        self.resolved + self.fallback + self.contradictions
    }
}

/// Returns the names of the schema properties that carry `x-gameText`, and `name`.
fn text_fields() -> Vec<String> {
    fn collect(node: &Value, found: &mut Vec<String>) {
        match node {
            Value::Object(map) => {
                if let Some(properties) = map.get("properties").and_then(Value::as_object) {
                    for (name, value) in properties {
                        if value.get("x-gameText") == Some(&Value::Bool(true)) {
                            found.push(name.clone());
                        }
                    }
                }
                map.values().for_each(|value| collect(value, found));
            }
            Value::Array(items) => items.iter().for_each(|value| collect(value, found)),
            _ => {}
        }
    }
    let mut found = vec!["name".to_owned()];
    collect(&catalog_schema(), &mut found);
    found.sort();
    found.dedup();
    found
}

/// Collects the texts with a key below `node`: the value of every field named in `fields` that
/// is an object with a string `key`, placeholder keys excluded.
fn keyed_texts<'v>(node: &'v Value, fields: &[String], found: &mut Vec<(&'v str, &'v Value)>) {
    match node {
        Value::Object(map) => {
            for (name, value) in map {
                if fields.contains(name)
                    && let Some(key) = value.get("key").and_then(Value::as_str)
                    && !is_placeholder_key(key)
                {
                    found.push((key, value));
                }
                keyed_texts(value, fields, found);
            }
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| keyed_texts(item, fields, found)),
        _ => {}
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a share of counts far below 2^52"
)]
fn share(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

fn baseline(kind: Kind) -> Baseline {
    BASELINES
        .into_iter()
        .find(|known| known.kind == kind)
        .unwrap_or_else(|| baseline_of(kind, 0.0, 0.0))
}

/// Checks the keyed texts of every kind of `catalog` against the `global.ini` files of `archive`
/// and prints the numbers per kind; `baselined` also holds each kind to its [`Baseline`], which
/// holds for LIVE only.
pub(crate) fn check_texts(archive: &Archive, catalog: &Value, baselined: bool) -> TestResult {
    let set = LocalizationSet::read(archive, &[Language::English, Language::German])?;
    let texts = set.index()?;
    let (english_oracle, german_oracle) = (
        Oracle::of(&texts, Language::English)?,
        Oracle::of(&texts, Language::German)?,
    );
    let fields = text_fields();
    let mut failures: Vec<String> = Vec::new();
    for kind in Kind::ALL {
        let (mut english, mut german) = (Coverage::default(), Coverage::default());
        let mut found = Vec::new();
        for entity in list(catalog, kind)? {
            keyed_texts(entity, &fields, &mut found);
        }
        for (key, text) in found {
            english.count(&english_oracle, key, text.get("en"));
            german.count(&german_oracle, key, text.get("de"));
        }
        let keyed = english.total();
        if keyed == 0 {
            continue;
        }
        let baseline = baseline(kind);
        print_line(&format!(
            "{kind}: {keyed} keyed texts, en {} resolved / {} unresolved ({:.3}, baseline {:.2}, e.g. {:?}), de {} resolved / {} fall back to en ({:.3}, baseline {:.2})",
            english.resolved,
            english.fallback,
            share(english.fallback, keyed),
            baseline.english,
            english.lacking,
            german.resolved,
            german.fallback,
            share(german.fallback, keyed),
            baseline.german,
        ));
        verdict(
            kind,
            "en",
            &english,
            baselined.then_some(baseline.english),
            &mut failures,
        );
        verdict(
            kind,
            "de",
            &german,
            baselined.then_some(baseline.german),
            &mut failures,
        );
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

fn verdict(
    kind: Kind,
    language: &str,
    coverage: &Coverage,
    ceiling: Option<f64>,
    failures: &mut Vec<String>,
) {
    if coverage.contradictions > 0 {
        failures.push(format!(
            "{kind}: {} {language} texts contradict the global.ini, e.g. {:?}",
            coverage.contradictions, coverage.contradicted
        ));
    }
    let fallback = share(coverage.fallback, coverage.total());
    if let Some(ceiling) = ceiling
        && fallback > ceiling
    {
        failures.push(format!(
            "{kind}: {fallback:.3} of the {language} texts fall back, above the baseline {ceiling:.2}, e.g. {:?}",
            coverage.lacking
        ));
    }
}
