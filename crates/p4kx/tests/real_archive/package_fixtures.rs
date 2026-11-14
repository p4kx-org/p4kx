// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::ffi::OsStr;
use std::fmt;
use std::sync::Arc;

use p4kx_assembly::{Entities, EntityLookupError};
use p4kx_extraction::Record;
use p4kx_testkit::media_fixture::{Candidate, CaseOutcome, RunReport};
use serde_json::Value;

use crate::media_fixtures::{self, Case, DockedVehicles, Family, Identity, Index, Requirements};
use crate::package_fault::{self, Check};
use crate::package_support::{self, Built, Measured, Scratch, TestResult};

struct SelectorFailure(Vec<EntityLookupError>);

impl fmt::Debug for SelectorFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "package_selector_failed({})", self.0.len())
    }
}
impl fmt::Display for SelectorFailure {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, output)
    }
}
impl Error for SelectorFailure {}

#[derive(Clone, Copy)]
pub(crate) enum PackageScope {
    Hull,
    Interior,
}

pub(crate) fn matrix(
    scratch: &Scratch,
    families: &[Family],
    scope: PackageScope,
    extra: &[&OsStr],
    verify: fn(&Built, &Measured) -> TestResult,
) -> TestResult {
    media_fixtures::with_source(|archive, forge, files| {
        let entities = Arc::new(Entities::new(forge));
        let index = Index::new(Arc::clone(&entities), files)?;
        let mut report = RunReport::default();
        let cases = families.iter().copied().enumerate();
        for (ordinal, family) in cases {
            let case = Case::from_family(
                "cli_package",
                family,
                Requirements {
                    geometry_lod: Some(0),
                    item_size: None,
                    requires_interior: matches!(scope, PackageScope::Interior),
                    requires_audio: false,
                    docked: DockedVehicles::Stop,
                },
            );
            report.run(format!("package_{ordinal}"), || {
                let Some(choice) = index.choose(archive, &case)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&case, &choice);
                let argument = package_fault::report(argument(&entities, &choice.candidate))?;
                let built = package_fault::report(package_support::build(
                    scratch,
                    "entity",
                    Some(&argument),
                    &format!("case-{ordinal}"),
                    extra,
                ))?;
                package_fault::report(package_fault::require(
                    built.report.get("guid").and_then(Value::as_str)
                        == Some(choice.candidate.identity.guid.to_string().as_str())
                        && built.report.get("entity").and_then(Value::as_str)
                            == Some(choice.candidate.class_name.as_str()),
                    Check::SourceIdentity,
                ))?;
                let measured = package_fault::report(package_support::verify(&built))?;
                package_fault::report(verify(&built, &measured))?;
                Ok(CaseOutcome::Completed(()))
            });
        }
        media_fixtures::finish(&report)
    })
}

pub(crate) fn argument(
    entities: &Entities<'_>,
    candidate: &Candidate<Identity>,
) -> package_fault::Result<String> {
    let class = format!("EntityClassDefinition.{}", candidate.class_name);
    let class_failure = match entities.find(&class) {
        Ok(record) => {
            exact(&record, candidate)?;
            return Ok(class);
        }
        Err(error) => error,
    };
    match entities.find(&candidate.identity.record_path) {
        Ok(record) => {
            exact(&record, candidate)?;
            Ok(candidate.identity.record_path.clone())
        }
        Err(error) => Err(package_fault::Fault::at(
            Check::SourceAmbiguous,
            SelectorFailure(vec![class_failure, error]),
        )),
    }
}

fn exact(record: &Record<'_>, candidate: &Candidate<Identity>) -> package_fault::Result<()> {
    package_fault::require(
        record.index() == candidate.identity.record_index
            && record.guid() == candidate.identity.guid
            && record.record_path()? == candidate.identity.record_path
            && record.name()?.strip_prefix("EntityClassDefinition.")
                == Some(candidate.class_name.as_str()),
        Check::SourceIdentity,
    )
}
