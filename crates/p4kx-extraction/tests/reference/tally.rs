// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

/// The most characters of one example a summary prints.
const MAX_EXAMPLE_CHARS: usize = 400;

/// The most examples a summary prints.
pub(crate) const MAX_EXAMPLES: usize = 25;

/// The result of comparing one item with its reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Compared {
    Equal,
    /// Every difference is on the allow-list; the reason of each allowed value, in order.
    Allowed(Vec<&'static str>),
    /// The first difference not on the allow-list, with its path.
    Different(String),
}

impl Tally {
    pub(crate) fn record(&mut self, key: &str, compared: Compared) {
        match compared {
            Compared::Equal => self.equal(),
            Compared::Allowed(reasons) => self.allow_values(&reasons),
            Compared::Different(difference) => self.different(format!("{key}: {difference}")),
        }
    }
}

/// The counts of one differential comparison.
///
/// A difference fails the comparison unless the caller recorded it with [`Tally::allow`] under a
/// documented reason. `allowed` counts the allowed values per reason, `allowed_items` the items
/// with at least one allowed value.
#[derive(Debug, Default, Clone)]
pub(crate) struct Tally {
    pub(crate) compared: u64,
    pub(crate) equal: u64,
    pub(crate) different: u64,
    pub(crate) allowed_items: u64,
    pub(crate) allowed: BTreeMap<&'static str, u64>,
    pub(crate) embedded_records: u64,
    pub(crate) missing_in_reference: u64,
    pub(crate) missing_in_archive: u64,
    examples: Vec<String>,
}

impl Tally {
    pub(crate) const fn equal(&mut self) {
        self.compared += 1;
        self.equal += 1;
    }

    pub(crate) fn different(&mut self, example: String) {
        self.compared += 1;
        self.different += 1;
        self.example(example);
    }

    pub(crate) fn allow(&mut self, reason: &'static str) {
        self.allow_values(&[reason]);
    }

    pub(crate) fn allow_values(&mut self, reasons: &[&'static str]) {
        self.compared += 1;
        self.allowed_items += 1;
        for reason in reasons {
            *self.allowed.entry(reason).or_default() += 1;
        }
    }

    pub(crate) const fn missing_in_reference(&mut self) {
        self.missing_in_reference += 1;
    }

    pub(crate) fn missing_in_archive(&mut self, example: String) {
        self.missing_in_archive += 1;
        self.example(example);
    }

    fn example(&mut self, mut example: String) {
        if self.examples.len() < MAX_EXAMPLES {
            if let Some((end, _)) = example.char_indices().nth(MAX_EXAMPLE_CHARS) {
                example.truncate(end);
                example.push('…');
            }
            self.examples.push(example);
        }
    }

    #[must_use]
    pub(crate) fn merge(mut self, other: Self) -> Self {
        self.compared += other.compared;
        self.equal += other.equal;
        self.different += other.different;
        self.allowed_items += other.allowed_items;
        self.embedded_records += other.embedded_records;
        self.missing_in_reference += other.missing_in_reference;
        self.missing_in_archive += other.missing_in_archive;
        for (reason, count) in other.allowed {
            *self.allowed.entry(reason).or_default() += count;
        }
        for example in other.examples {
            self.example(example);
        }
        self
    }

    #[must_use]
    pub(crate) const fn passes(&self) -> bool {
        self.compared > 0
            && self.different == 0
            && self.missing_in_archive == 0
            && self.missing_in_reference == 0
    }

    #[must_use]
    pub(crate) fn examples(&self) -> &[String] {
        &self.examples
    }

    #[must_use]
    pub(crate) fn summary(&self, name: &str) -> String {
        let head = format!(
            "{name}: {} compared, {} equal, {} different, {} allowed, {} missing in the reference, {} missing in the Archive",
            self.compared,
            self.equal,
            self.different,
            self.allowed_items,
            self.missing_in_reference,
            self.missing_in_archive
        );
        let reasons = self
            .allowed
            .iter()
            .map(|(reason, count)| format!("  allowed {count} values: {reason}"));
        let embedded = (self.embedded_records > 0).then(|| {
            format!(
                "  {} embedded Records compared by content (a lower bound when a Record differs)",
                self.embedded_records
            )
        });
        let examples = self.examples.iter().map(|example| format!("  {example}"));
        std::iter::once(head)
            .chain(reasons)
            .chain(embedded)
            .chain(examples)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Prints the summary and fails when a difference is not on the allow-list.
    pub(crate) fn finish(&self, name: &str) -> Result<(), String> {
        let summary = self.summary(name);
        super::report(&summary);
        if self.passes() { Ok(()) } else { Err(summary) }
    }
}

#[cfg(test)]
mod tests {
    use super::{Compared, MAX_EXAMPLES, Tally};

    #[test]
    fn zero_comparisons_never_establish_independent_verification() {
        assert!(
            !Tally::default().passes(),
            "an empty comparison is not evidence"
        );
    }
    #[test]
    fn at_most_the_example_limit_is_kept_and_a_difference_fails() {
        let mut tally = Tally::default();
        for index in 0..MAX_EXAMPLES * 2 {
            tally.different(format!("example {index}"));
        }
        assert_eq!(tally.examples().len(), MAX_EXAMPLES, "capped examples");
        assert!(!tally.passes(), "a difference fails");
    }

    #[test]
    fn an_allowed_difference_does_not_excuse_a_missing_selected_reference() {
        let mut tally = Tally::default();
        tally.equal();
        tally.allow("a documented reason");
        tally.missing_in_reference();
        assert!(
            !tally.passes(),
            "missing selected output fails: {}",
            tally.summary("t")
        );
        assert_eq!(tally.compared, 2, "equal and allowed are compared");
    }

    #[test]
    fn allowed_values_are_counted_per_reason_and_items_once() {
        let mut tally = Tally::default();
        tally.record("a", Compared::Allowed(vec!["x", "x", "y"]));
        tally.record("b", Compared::Allowed(vec!["x"]));
        assert_eq!(tally.compared, 2, "two items");
        assert_eq!(tally.allowed_items, 2, "two items with allowances");
        assert_eq!(tally.allowed.get("x"), Some(&3), "values of x");
        assert_eq!(tally.allowed.get("y"), Some(&1), "values of y");
        let merged = tally.clone().merge(tally);
        assert_eq!(merged.allowed_items, 4, "merge adds items");
        assert_eq!(merged.allowed.get("x"), Some(&6), "merge adds values");
    }

    #[test]
    fn a_file_missing_in_the_archive_fails() {
        let mut tally = Tally::default();
        tally.missing_in_archive("only in the reference".to_owned());
        assert!(!tally.passes(), "missing in the Archive fails");
    }

    #[test]
    fn merging_adds_counts_and_keeps_the_cap() {
        let mut left = Tally::default();
        let mut right = Tally::default();
        for index in 0..MAX_EXAMPLES {
            left.different(format!("l{index}"));
            right.different(format!("r{index}"));
        }
        let merged = left.merge(right);
        assert_eq!(merged.different, 2 * MAX_EXAMPLES as u64, "counts add");
        assert_eq!(merged.examples().len(), MAX_EXAMPLES, "cap holds");
    }
}
