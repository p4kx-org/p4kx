// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The text check of the verification, per kind: the text the catalog holds for a key equals the
//! text the language's `global.ini` holds for it, a key the file lacks leaves that language `null`,
//! and, on the baselined Archive, the keys a file lacks stay within a baseline per kind. The
//! translation completeness of the game is measured, never asserted as a global share.
//!
//! The expectation is built here from the file's lines with the game's rules (the key without its
//! `@`, ASCII case ignored, the last plain and suffixed declarations with a non-empty plain
//! value preferred, `\n` decoded and the text trimmed) and does not use the exporter's lookup.

use std::error::Error;
use std::fmt;

use p4kx_export::{Kind, Language, catalog_schema};
use p4kx_extraction::{Archive, LocalizationSet};
use serde_json::Value;

use crate::catalog_v3_consumer;
pub(crate) use crate::real_archive_canonical_texts::manufacturer_text;
use crate::real_archive_canonical_texts::{TextIndex, kind_coverage};
use crate::real_archive_text_oracle::Oracle;
use crate::real_archive_verification::{TestResult, print_line};

#[path = "real_archive_texts_tests.rs"]
mod tests;

#[path = "real_archive_canonical_texts_tests.rs"]
mod canonical_tests;

#[path = "real_archive_legacy_texts.rs"]
mod legacy;
#[path = "real_archive_live_texts.rs"]
mod live;

pub(crate) use legacy::keyed_texts;

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

/// The German texts a kind with a baseline of 1.0 must still resolve, as measured on LIVE and
/// rounded down: a baseline of 1.0 alone would not notice keys that stop resolving.
const GERMAN_FLOORS: [(Kind, usize); 1] = [(Kind::MissionBrokerEntries, 90)];

/// The start of a mission token (`~mission(Contractor|Key)`) in a contract text; the exporter
/// replaces it by a placeholder such as `[CONTRACTOR]`, so only the presence of such a text is
/// compared.
const MISSION_TOKEN: &str = "~mission(";

/// What the catalog holds for the keyed texts of one kind in one language.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Coverage {
    /// Texts the file holds and the catalog has, equal.
    resolved: usize,
    /// Texts without a non-empty source translation.
    fallback: usize,
    /// Texts where the catalog contradicts the file: the exporter is wrong.
    contradictions: usize,
    supplemental_contradictions: usize,
}

impl Coverage {
    pub(crate) fn count(&mut self, oracle: &Oracle, key: &str, shown: Option<&Value>) {
        match (oracle.text(key), shown) {
            (Some(expected), Some(Value::String(shown)))
                if expected == shown || (!shown.is_empty() && expected.contains(MISSION_TOKEN)) =>
            {
                self.resolved += 1;
            }
            (None, Some(Value::Null)) => {
                self.fallback += 1;
            }
            _ => {
                self.contradictions += 1;
            }
        }
    }

    pub(crate) fn count_canonical(&mut self, oracle: &Oracle, key: &str, shown: &Value) {
        let Some(expected) = oracle.canonical_text(key) else {
            self.count(oracle, key, Some(shown));
            return;
        };
        if shown.as_str() == Some(expected) {
            if expected.is_empty() {
                self.fallback += 1;
            } else {
                self.resolved += 1;
            }
        } else {
            self.contradictions += 1;
        }
    }

    pub(crate) fn count_forms(
        &mut self,
        oracle: &Oracle,
        key: &str,
        source: &Value,
        shown: &Value,
    ) {
        let expected = oracle.source_text(key);
        let matches = match (expected, source) {
            (Some(expected), Value::String(actual)) => actual == expected,
            (None, Value::Null) => true,
            _ => false,
        };
        if matches {
            self.count_canonical(oracle, key, shown);
        } else {
            self.contradictions += 1;
        }
    }

    pub(super) fn with_supplemental(mut self, extra: Self) -> Self {
        self.supplemental_contradictions = extra.contradictions;
        self
    }

    const fn total(&self) -> usize {
        self.resolved + self.fallback + self.contradictions
    }
}

impl fmt::Display for Coverage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "total={} resolved={} fallback={} contradictions={} supplementalContradictions={}",
            self.total(),
            self.resolved,
            self.fallback,
            self.contradictions,
            self.supplemental_contradictions,
        )
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
    let set = LocalizationSet::read(archive, &[Language::English, Language::German])
        .map_err(|_| "could not read Archive localization sources")?;
    let (english_oracle, german_oracle) = (
        Oracle::of(&set, Language::English)?,
        Oracle::of(&set, Language::German)?,
    );
    check_catalog_texts(catalog, &english_oracle, &german_oracle, baselined)
}

fn check_catalog_texts(
    catalog: &Value,
    english_oracle: &Oracle,
    german_oracle: &Oracle,
    baselined: bool,
) -> TestResult {
    let fields = text_fields();
    let index = TextIndex::new(catalog).map_err(|_| "invalid canonical text index")?;
    let mut failures: Vec<String> = Vec::new();
    for kind in Kind::ALL {
        let Ok(rows) = text_rows(catalog, kind) else {
            failures.push(format!("{kind}: invalid dataset rows or source identities"));
            continue;
        };
        let Ok((english, german)) = kind_coverage(
            catalog,
            &index,
            kind,
            rows,
            &fields,
            english_oracle,
            german_oracle,
        ) else {
            failures.push(format!("{kind}: invalid text carriers or references"));
            continue;
        };
        check_coverage(
            kind,
            &english,
            &german,
            baselined,
            rows.len(),
            &mut failures,
        );
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("\n").into())
    }
}

pub(super) fn text_rows(catalog: &Value, kind: Kind) -> Result<&[Value], Box<dyn Error>> {
    let rows = if kind == Kind::Commodities {
        catalog_v3_consumer::presentations(catalog)?
    } else {
        catalog_v3_consumer::rows(catalog, kind)?
    };
    Ok(rows.as_slice())
}

fn floor_verdict(kind: Kind, german: &Coverage, failures: &mut Vec<String>) {
    if let Some((_, floor)) = GERMAN_FLOORS.iter().find(|(known, _)| *known == kind)
        && german.resolved < *floor
    {
        failures.push(format!(
            "{kind}: only {} de texts resolve, below the floor {floor}",
            german.resolved
        ));
    }
}

fn verdict(
    kind: Kind,
    language: &str,
    coverage: &Coverage,
    ceiling: Option<f64>,
    failures: &mut Vec<String>,
) {
    let contradictions = coverage.contradictions + coverage.supplemental_contradictions;
    if contradictions > 0 {
        failures.push(format!(
            "{kind}: {contradictions} {language} texts contradict the global.ini"
        ));
    }
    let fallback = share(coverage.fallback, coverage.total());
    if let Some(ceiling) = ceiling
        && fallback > ceiling
    {
        failures.push(format!(
            "{kind}: {} of {} {language} texts fall back, above the baseline {ceiling:.2}",
            coverage.fallback,
            coverage.total()
        ));
    }
}

fn check_coverage(
    kind: Kind,
    english: &Coverage,
    german: &Coverage,
    baselined: bool,
    row_count: usize,
    failures: &mut Vec<String>,
) {
    print_line(&format!(
        "{kind}: rows={row_count} en [{english}] de [{german}]"
    ));
    if english.total() == 0 {
        if baselined
            && matches!(
                kind,
                Kind::Manufacturers
                    | Kind::Items
                    | Kind::ResourceTypes
                    | Kind::ResourceGroups
                    | Kind::GameplayProperties
                    | Kind::Commodities
                    | Kind::ContractTemplates
                    | Kind::MissionTypes
                    | Kind::MissionLocationTemplates
                    | Kind::MissionBrokerEntries
                    | Kind::FactionReputations
                    | Kind::ReputationScopes
                    | Kind::ReputationStandings
            )
            && row_count > 0
        {
            failures.push(format!(
                "{kind}: positive baseline rows have no verifiable canonical texts"
            ));
        }
        verdict(kind, "en", english, None, failures);
        verdict(kind, "de", german, None, failures);
        return;
    }
    let baseline = baseline(kind);
    verdict(
        kind,
        "en",
        english,
        baselined.then_some(baseline.english),
        failures,
    );
    verdict(
        kind,
        "de",
        german,
        baselined.then_some(baseline.german),
        failures,
    );
    if baselined {
        floor_verdict(kind, german, failures);
    }
}
