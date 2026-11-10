// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

/// Relative tolerance between an SCMDB number and the `f32` the DataForge stores.
const RELATIVE_TOLERANCE: f64 = 1e-5;

/// Absolute tolerance for numbers near zero.
const ABSOLUTE_TOLERANCE: f64 = 1e-6;

/// What a field missing on one side counts as.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OneSided {
    /// A disagreement.
    Disagrees,
    /// Only tallied in the summary.
    Tallied,
}

/// How often one field was compared, or missing on both or on one side.
#[derive(Debug, Default, Clone, Copy)]
struct FieldTally {
    compared: usize,
    absent_both: usize,
    absent_one_side: usize,
}

impl FieldTally {
    const fn total(self) -> usize {
        self.compared + self.absent_both + self.absent_one_side
    }
}

/// Returns whether two numbers are equal within the tolerance.
fn numbers_agree(scmdb: f64, p4kx: f64) -> bool {
    (scmdb - p4kx).abs() <= ABSOLUTE_TOLERANCE.max(RELATIVE_TOLERANCE * scmdb.abs().max(p4kx.abs()))
}

fn exact_integer(value: &serde_json::Value) -> Option<&serde_json::Number> {
    value
        .as_number()
        .filter(|number| number.is_i64() || number.is_u64())
}
/// The checks made per category, the per-field tallies and the disagreements found.
#[derive(Debug, Default)]
pub(crate) struct Findings {
    checks: BTreeMap<&'static str, usize>,
    fields: BTreeMap<(&'static str, String), FieldTally>,
    mismatches: BTreeMap<&'static str, Vec<String>>,
    uncompared: BTreeMap<(&'static str, &'static str, &'static str), usize>,
}

impl Findings {
    /// Counts one check of `category` and records `describe()` when the values disagree.
    pub(crate) fn check(
        &mut self,
        category: &'static str,
        agrees: bool,
        describe: impl FnOnce() -> String,
    ) {
        *self.checks.entry(category).or_default() += 1;
        if !agrees {
            self.mismatches
                .entry(category)
                .or_default()
                .push(describe());
        }
    }

    /// Compares one field within the tolerance. A field missing on both sides is tallied, not
    /// compared; a field missing on one side disagrees.
    pub(crate) fn number(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        scmdb: Option<f64>,
        p4kx: Option<f64>,
    ) {
        self.field(
            category,
            ident,
            field,
            (scmdb, p4kx),
            OneSided::Disagrees,
            numbers_agree,
        );
    }

    /// Like [`Self::number`], but a field missing on one side is only tallied, for fields one
    /// source omits at their default.
    pub(crate) fn number_if_both(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        scmdb: Option<f64>,
        p4kx: Option<f64>,
    ) {
        self.field(
            category,
            ident,
            field,
            (scmdb, p4kx),
            OneSided::Tallied,
            numbers_agree,
        );
    }

    /// Like [`Self::number_if_both`] for texts compared for equality.
    pub(crate) fn text_if_both(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        scmdb: Option<&str>,
        p4kx: Option<&str>,
    ) {
        self.field(
            category,
            ident,
            field,
            (scmdb, p4kx),
            OneSided::Tallied,
            |scmdb, p4kx| scmdb == p4kx,
        );
    }

    fn field<T: Copy + std::fmt::Debug>(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        (scmdb, p4kx): (Option<T>, Option<T>),
        one_sided: OneSided,
        agree: impl FnOnce(T, T) -> bool,
    ) {
        self.checks.entry(category).or_default();
        let tally = self.fields.entry((category, field.to_owned())).or_default();
        let agrees = match (scmdb, p4kx) {
            (Some(scmdb), Some(p4kx)) => {
                tally.compared += 1;
                Some(agree(scmdb, p4kx))
            }
            (None, None) => {
                tally.absent_both += 1;
                None
            }
            _ => {
                tally.absent_one_side += 1;
                (one_sided == OneSided::Disagrees).then_some(false)
            }
        };
        if let Some(agrees) = agrees {
            self.check(category, agrees, || {
                format!("{ident}: {field} SCMDB={scmdb:?} p4kx={p4kx:?}")
            });
        }
    }

    /// Keeps an empty comparison family visible in the summary.
    pub(crate) fn category(&mut self, category: &'static str) {
        self.checks.entry(category).or_default();
    }

    /// Counts a reference occurrence whose source mapping is unproven.
    pub(crate) fn uncomparable(
        &mut self,
        category: &'static str,
        field: &'static str,
        reason: &'static str,
    ) {
        self.category(category);
        *self
            .uncompared
            .entry((category, field, reason))
            .or_default() += 1;
    }

    /// Compares exact JSON integers, treating one-sided absence as disagreement.
    pub(crate) fn integer(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        values: (Option<&serde_json::Value>, Option<&serde_json::Value>),
    ) {
        self.integer_field(category, ident, field, values, OneSided::Disagrees);
    }

    /// Compares exact JSON integers and tallies one-sided absence.
    pub(crate) fn integer_if_both(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        values: (Option<&serde_json::Value>, Option<&serde_json::Value>),
    ) {
        self.integer_field(category, ident, field, values, OneSided::Tallied);
    }

    fn integer_field(
        &mut self,
        category: &'static str,
        ident: &str,
        field: &str,
        values: (Option<&serde_json::Value>, Option<&serde_json::Value>),
        one_sided: OneSided,
    ) {
        for value in <[_; 2]>::from(values).into_iter().flatten() {
            if !value.is_null() && exact_integer(value).is_none() {
                self.check(category, false, || {
                    format!("{ident}: {field} is not an exact JSON integer: {value}")
                });
            }
        }
        self.field(
            category,
            ident,
            field,
            (
                values.0.and_then(exact_integer),
                values.1.and_then(exact_integer),
            ),
            one_sided,
            |left, right| left == right,
        );
    }

    /// Returns whether any compared value or required input disagreed.
    pub(crate) fn has_mismatches(&self) -> bool {
        !self.mismatches.is_empty()
    }

    /// Returns whether no disagreement or uncomparable occurrence was recorded.
    pub(crate) fn is_empty(&self) -> bool {
        !self.has_mismatches() && self.uncompared.is_empty()
    }

    /// Returns the number of disagreements of `category`.
    pub(crate) fn mismatches_of(&self, category: &str) -> usize {
        self.mismatches.get(category).map_or(0, Vec::len)
    }

    /// Returns the tally of `field` in `category`.
    fn tally(&self, category: &str, field: &str) -> FieldTally {
        self.fields
            .iter()
            .find(|((c, f), _)| *c == category && f == field)
            .map_or_else(FieldTally::default, |(_, tally)| *tally)
    }

    /// Returns how often `field` of `category` was compared.
    pub(crate) fn compared(&self, category: &str, field: &str) -> usize {
        self.tally(category, field).compared
    }

    /// Returns how often `field` of `category` was missing on both sides.
    pub(crate) fn absent_on_both(&self, category: &str, field: &str) -> usize {
        self.tally(category, field).absent_both
    }

    /// Returns how often `field` of `category` was missing on one side only.
    pub(crate) fn absent_on_one_side(&self, category: &str, field: &str) -> usize {
        self.tally(category, field).absent_one_side
    }

    /// Returns one line per category with its checks and disagreements, each followed by one
    /// line per field with how many of its occurrences were compared.
    pub(crate) fn summary(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for (category, checks) in &self.checks {
            lines.push(format!(
                "{category}: {checks} checks, {} disagree",
                self.mismatches_of(category)
            ));
            for ((_, field), tally) in self.fields.iter().filter(|((c, _), _)| c == category) {
                lines.push(format!(
                    "  {field}: compared {} of {} ({} absent on both sides, {} absent on one side)",
                    tally.compared,
                    tally.total(),
                    tally.absent_both,
                    tally.absent_one_side
                ));
            }
        }
        for ((category, field, reason), count) in &self.uncompared {
            lines.push(format!(
                "{category} / {field}: not compared {count} ({reason})"
            ));
        }
        lines
    }

    /// Returns up to `per_category` disagreements of every category.
    pub(crate) fn examples(&self, per_category: usize) -> Vec<&str> {
        self.mismatches
            .values()
            .flat_map(|lines| lines.iter().take(per_category).map(String::as_str))
            .collect()
    }
}
