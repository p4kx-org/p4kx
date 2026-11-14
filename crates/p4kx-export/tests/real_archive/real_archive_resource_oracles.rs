// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded build-bound commodity and canonical resource source oracles.

use std::error::Error;
use std::fmt;

#[path = "resource_oracle/build.rs"]
mod build;
#[path = "resource_oracle/cardinality.rs"]
mod cardinality;
#[path = "resource_oracle/carriers.rs"]
mod carriers;
#[path = "resource_oracle/compare.rs"]
mod compare;
#[path = "resource_oracle/fixture.rs"]
mod fixture;
#[path = "resource_oracle/groups.rs"]
mod groups;
#[path = "resource_oracle/record_header.rs"]
mod record_header;
#[path = "resource_oracle/values.rs"]
mod values;

pub(super) type Result<T> = std::result::Result<T, Failure>;

pub(super) struct Failure {
    stage: &'static str,
    cause: Option<Box<dyn Error + Send + Sync>>,
}

impl Failure {
    pub(super) const fn check(stage: &'static str) -> Self {
        Self { stage, cause: None }
    }

    pub(super) fn source<E: Error + Send + Sync + 'static>(stage: &'static str, error: E) -> Self {
        Self {
            stage,
            cause: Some(Box::new(error)),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "resource oracle failed at {}", self.stage)
    }
}

impl fmt::Debug for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ResourceOracleFailure")
            .field("stage", &self.stage)
            .finish_non_exhaustive()
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|error| error as &(dyn Error + 'static))
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn commodity_identity_and_resource_owned_values_match_the_observed_build() -> Result<()> {
    let primary = p4kx_testkit::real_archive_path()
        .map_err(|error| Failure::source("primary-configuration", error))?;
    fixture::verify(&primary, "primary")?;
    if let Some(secondary) = p4kx_testkit::real_archive_b_path() {
        fixture::verify(&secondary, "secondary")?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::Failure;

    #[test]
    fn harness_errors_preserve_causes_without_printing_private_source_values() {
        let failure = Failure::source(
            "synthetic-private-source",
            std::io::Error::other("private-record-value"),
        );
        assert!(
            !format!("{failure:?}").contains("private-record-value"),
            "the test harness receives only the stage"
        );
        assert!(
            !failure.to_string().contains("private-record-value"),
            "the public display is sanitized"
        );
        assert!(
            failure.source().is_some(),
            "the underlying error remains available for private diagnosis"
        );
    }
}
