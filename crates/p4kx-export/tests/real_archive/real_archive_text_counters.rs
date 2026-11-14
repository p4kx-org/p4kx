// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Coverage, Oracle, TestResult, TextTarget};

pub(super) struct CoverageCounters<'oracle> {
    english: Coverage,
    german: Coverage,
    extra_english: Coverage,
    extra_german: Coverage,
    english_oracle: &'oracle Oracle,
    german_oracle: &'oracle Oracle,
}

impl<'oracle> CoverageCounters<'oracle> {
    pub(super) fn new(english: &'oracle Oracle, german: &'oracle Oracle) -> Self {
        Self {
            english: Coverage::default(),
            german: Coverage::default(),
            extra_english: Coverage::default(),
            extra_german: Coverage::default(),
            english_oracle: english,
            german_oracle: german,
        }
    }

    pub(super) fn count(&mut self, target: Option<&TextTarget<'_>>) -> TestResult<()> {
        self.oracles()
            .count(target, &mut self.english, &mut self.german)
    }

    pub(super) fn count_supplemental(&mut self, target: Option<&TextTarget<'_>>) -> TestResult<()> {
        self.oracles()
            .count(target, &mut self.extra_english, &mut self.extra_german)
    }

    const fn oracles(&self) -> Oracles<'oracle> {
        Oracles {
            english: self.english_oracle,
            german: self.german_oracle,
        }
    }

    pub(super) fn finish(self) -> (Coverage, Coverage) {
        (
            self.english.with_supplemental(self.extra_english),
            self.german.with_supplemental(self.extra_german),
        )
    }
}

struct Oracles<'a> {
    english: &'a Oracle,
    german: &'a Oracle,
}

impl Oracles<'_> {
    pub(super) fn count(
        &self,
        target: Option<&TextTarget<'_>>,
        english: &mut Coverage,
        german: &mut Coverage,
    ) -> TestResult<()> {
        let Some(target) = target else {
            return Ok(());
        };
        if p4kx_extraction::is_placeholder_key(target.key) {
            return Ok(());
        }
        english.count_forms(
            self.english,
            target.key,
            target.raw("en")?,
            target.shown("en")?,
        );
        german.count_forms(
            self.german,
            target.key,
            target.raw("de")?,
            target.shown("de")?,
        );
        Ok(())
    }
}
