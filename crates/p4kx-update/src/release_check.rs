// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::Duration;

use serde::Deserialize;

use crate::distribution::Distribution;
use crate::{Release, UpdateError};

const GITHUB_LATEST: &str = "https://api.github.com/repos/p4kx-org/p4kx/releases/latest";
const TIMEOUT: Duration = Duration::from_secs(10);
const USER_AGENT: &str = concat!("P4KX/", env!("CARGO_PKG_VERSION"));

#[derive(Deserialize)]
struct Answer {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    assets: serde_json::Value,
}

/// The check against one releases endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseCheck {
    endpoint: String,
    distribution: Option<Distribution>,
}

impl ReleaseCheck {
    /// Creates the check against the latest Release of this repository on GitHub.
    /// Missing, malformed or mismatched package identity selects the Release page.
    #[must_use]
    pub fn github() -> Self {
        let mut check = Self::new(GITHUB_LATEST);
        check.distribution = Distribution::current();
        check
    }

    /// Creates the check against `endpoint`, which answers like the GitHub latest-Release API.
    #[must_use]
    pub fn new(endpoint: &str) -> Self {
        Self {
            endpoint: endpoint.to_owned(),
            distribution: None,
        }
    }

    /// Selects a package format for the compiled binary target.
    /// A format incompatible with that platform selects the Release page.
    #[must_use]
    pub fn with_package(mut self, package: crate::PackageKind) -> Self {
        self.distribution = crate::Platform::current()
            .filter(|platform| package.supports(platform.os))
            .map(|platform| Distribution { platform, package });
        self
    }

    /// Returns the endpoint asked.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Asks for the latest Release and returns it when it is newer than `current`.
    ///
    /// # Errors
    ///
    /// [`UpdateError`] when the request fails, the answer is no Release, or either version is
    /// not a semantic version.
    pub fn newer_than(&self, current: &str) -> Result<Option<Release>, UpdateError> {
        let current = parse(current)?;
        let answer = self.latest()?;
        let version = answer
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&answer.tag_name);
        let latest = parse(version)?;
        if latest <= current {
            return Ok(None);
        }
        let asset = match &self.distribution {
            Some(distribution) => crate::release_asset::select_asset(
                version,
                distribution.platform,
                distribution.package,
                &answer.assets,
            )?,
            None => None,
        };
        Ok(Some(Release {
            version: version.to_owned(),
            page: answer.html_url,
            asset,
        }))
    }

    fn latest(&self) -> Result<Answer, UpdateError> {
        let request = |source| UpdateError::Request {
            endpoint: self.endpoint.clone(),
            source: Box::new(source),
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .user_agent(USER_AGENT)
            .build()
            .into();
        let body = agent
            .get(&self.endpoint)
            .header("Accept", "application/vnd.github+json")
            .call()
            .map_err(request)?
            .body_mut()
            .read_to_string()
            .map_err(request)?;
        serde_json::from_str(&body).map_err(|source| UpdateError::Malformed {
            endpoint: self.endpoint.clone(),
            source,
        })
    }
}

fn parse(version: &str) -> Result<semver::Version, UpdateError> {
    semver::Version::parse(version).map_err(|source| UpdateError::Version {
        version: version.to_owned(),
        source,
    })
}
