// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::Read;

use p4kx_extraction::{Build, Checksum};
use serde::{Deserialize, Serialize};

use super::cardinality::Counts;
use super::{Failure, Result};

const BASELINE_LIMIT: u64 = 16 * 1024;
const BASELINE_ENV: &str = "P4KX_COMMODITY_CARDINALITY_BASELINE";

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Identity {
    branch: String,
    version: String,
    change_number: String,
    build_id: String,
}

impl Identity {
    pub(super) fn of(build: Option<&Build>) -> Result<Self> {
        let build = build.ok_or_else(|| Failure::check("missing-complete-build-identity"))?;
        let value = Self {
            branch: build.branch().to_owned(),
            version: build.version().to_owned(),
            change_number: build.change_number().to_owned(),
            build_id: build.build_id().to_owned(),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        if [
            &self.branch,
            &self.version,
            &self.change_number,
            &self.build_id,
        ]
        .iter()
        .any(|part| part.trim().is_empty())
        {
            return Err(Failure::check("incomplete-build-identity"));
        }
        Ok(())
    }

    pub(super) fn check_catalog(&self, catalog: &serde_json::Value) -> Result<()> {
        let source = catalog
            .pointer("/source/build")
            .ok_or_else(|| Failure::check("catalog-build-missing"))?;
        for (field, expected) in [
            ("branch", &self.branch),
            ("version", &self.version),
            ("changeNumber", &self.change_number),
            ("buildId", &self.build_id),
        ] {
            if source[field].as_str() != Some(expected.as_str()) {
                return Err(Failure::check("catalog-build-identity-mismatch"));
            }
        }
        Ok(())
    }
    pub(super) fn tag(&self) -> Result<String> {
        use sha2::Digest as _;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| Failure::source("build-fingerprint", error))?;
        Ok(Checksum::Sha256(sha2::Sha256::digest(bytes).into()).hex())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Baseline {
    identity: Identity,
    counts: Counts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Pin {
    Unavailable,
    DifferentBuild,
    Matched,
}

pub(super) fn check(identity: &Identity, counts: &Counts) -> Result<Pin> {
    let Some(path) = std::env::var_os(BASELINE_ENV) else {
        return Ok(Pin::Unavailable);
    };
    let file = File::open(path).map_err(|error| Failure::source("baseline-open", error))?;
    let mut bytes = Vec::new();
    file.take(BASELINE_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| Failure::source("baseline-read", error))?;
    if u64::try_from(bytes.len()).map_err(|error| Failure::source("baseline-size", error))?
        > BASELINE_LIMIT
    {
        return Err(Failure::check("baseline-size"));
    }
    let baseline: Baseline = serde_json::from_slice(&bytes)
        .map_err(|error| Failure::source("baseline-format", error))?;
    compare(&baseline, identity, counts)
}

fn compare(baseline: &Baseline, identity: &Identity, counts: &Counts) -> Result<Pin> {
    baseline.identity.validate()?;
    identity.validate()?;
    if baseline.identity != *identity {
        return Ok(Pin::DifferentBuild);
    }
    if baseline.counts != *counts {
        return Err(Failure::check("exact-build-cardinality-mismatch"));
    }
    Ok(Pin::Matched)
}

#[cfg(test)]
mod tests {
    use super::{Baseline, Counts, Failure, Identity, Pin, Result, compare};

    fn identity() -> Identity {
        Identity {
            branch: "private-branch".into(),
            version: "1.2.3.456".into(),
            change_number: "123456".into(),
            build_id: "private-build-id".into(),
        }
    }

    #[test]
    fn cardinalities_are_pinned_only_for_an_exact_complete_build() -> Result<()> {
        let identity = identity();
        let baseline = Baseline {
            identity: identity.clone(),
            counts: Counts::default(),
        };
        assert_eq!(
            compare(&baseline, &identity, &Counts::default())?,
            Pin::Matched
        );
        for field in ["branch", "version", "changeNumber", "buildId"] {
            let mut value = serde_json::to_value(&identity)
                .map_err(|error| Failure::source("synthetic-identity", error))?;
            value[field] = serde_json::json!("different");
            let other = serde_json::from_value(value)
                .map_err(|error| Failure::source("synthetic-identity", error))?;
            assert_eq!(
                compare(&baseline, &other, &Counts::default())?,
                Pin::DifferentBuild,
                "every exact build component participates"
            );
        }
        let mut incomplete = identity;
        incomplete.build_id.clear();
        assert!(
            incomplete.validate().is_err(),
            "a version prefix is not complete build evidence"
        );
        Ok(())
    }

    #[test]
    fn a_version_prefix_and_wrong_exact_counts_never_match() -> Result<()> {
        let identity = identity();
        let mut prefix = identity.clone();
        prefix.version = "1.2.3".to_owned();
        let baseline = Baseline {
            identity: identity.clone(),
            counts: Counts::default(),
        };
        assert_eq!(
            compare(&baseline, &prefix, &Counts::default())?,
            Pin::DifferentBuild,
            "a public version prefix cannot activate an exact baseline"
        );
        let counts = serde_json::from_value(serde_json::json!({"rows": 1, "uniqueGuids": 1,
            "distinctSubtypesIncludingNull": 1, "nullSubtypes": 1,
            "sharedSubtypeGroups": 0, "rowsInSharedSubtypes": 0}))
        .map_err(|error| Failure::source("synthetic-counts", error))?;
        assert!(
            compare(&baseline, &identity, &counts).is_err(),
            "matching Builds must match all counts"
        );
        Ok(())
    }

    #[test]
    fn production_catalog_must_carry_the_same_complete_build() -> Result<()> {
        let identity = identity();
        let mut catalog = serde_json::json!({"source": {"build": serde_json::to_value(&identity)
            .map_err(|error| Failure::source("synthetic-build", error))?}});
        identity.check_catalog(&catalog)?;
        catalog["source"]["build"]["buildId"] = serde_json::json!("different-private-id");
        assert!(
            identity.check_catalog(&catalog).is_err(),
            "counts cannot drift to another Build"
        );
        Ok(())
    }
    #[test]
    fn report_tag_exposes_no_build_labels() -> Result<()> {
        let tag = identity().tag()?;
        assert_eq!(tag.len(), 64, "the report carries only a SHA-256 tag");
        assert!(
            tag.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "no labels enter the tag"
        );
        Ok(())
    }
}
