// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::OracleResult;
use super::error::{Failure, SetupFault};

/// The explicitly required positive coverage for a source-bound verification run.
pub(super) struct Requirements {
    pub(super) minimum_scoped_values: usize,
}

impl Requirements {
    pub(super) fn from_environment() -> OracleResult<Self> {
        let minimum_scoped_values = std::env::var_os("P4KX_MIN_SCOPED_LOCATION_VALUES")
            .map(|value| {
                value
                    .to_str()
                    .ok_or(Failure::Setup(SetupFault::CoverageConfiguration))?
                    .parse::<usize>()
                    .map_err(|_| Failure::Setup(SetupFault::CoverageConfiguration))
            })
            .transpose()?
            .unwrap_or_default();
        Ok(Self {
            minimum_scoped_values,
        })
    }

    pub(super) const fn check(&self, scoped_values: usize) -> OracleResult {
        if scoped_values < self.minimum_scoped_values {
            return Err(Failure::PositiveScopedCoverage);
        }
        Ok(())
    }
}
