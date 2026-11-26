// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use crate::Kind;
use crate::catalog_queries::{ReputationContextDefinition, ReputationRows, ReputationScopeContext};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

pub(super) const PATH: &str = "definitions.reputationContexts";

pub(super) fn stage(
    source: &ReputationRows<ReputationContextDefinition<'_>>,
    scopes: &RecordTargets,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<(StagedCatalogTable, RecordTargets), ReducedError> {
    completeness::inventory(PATH, source.availability, &source.coverage)?;
    let mut output = StagedCatalogRows::new(scratch, cancel);
    let mut targets = RecordTargets::default();
    for original in &source.rows {
        targets.insert(PATH, original.source())?;
        output.append(&Context::read(original, scopes)?)?;
    }
    let result = output.finish(source.availability)?;
    completeness::rows(PATH, source.rows.len(), result.len())?;
    Ok((result, targets))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Context<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    sort_order_scope: Cell<&'read str>,
    primary_scope_context: Cell<Scope<'read>>,
    scope_context_list: Cell<Vec<Cell<Scope<'read>>>>,
}

impl<'read> Context<'read> {
    fn read(
        source: &'read ReputationContextDefinition<'_>,
        scopes: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let header = source.source();
        completeness::value(PATH, &header.guid())?;
        completeness::value(PATH, header.record_index())?;
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        Ok(Self {
            identity: header.into(),
            sort_order_scope: completeness::cell(PATH, source.sort_order_scope())?
                .map(|sort| *sort),
            primary_scope_context: completeness::cell(PATH, source.primary_scope_context())?
                .try_map(|scope| Scope::read(scope, scopes))?,
            scope_context_list: completeness::list(PATH, source.scope_context_list(), |scope| {
                Scope::read(scope, scopes)
            })?,
        })
    }
}

#[derive(Serialize)]
struct Scope<'read> {
    scope: Cell<RecordLink<'read>>,
}

impl<'read> Scope<'read> {
    fn read(
        source: &'read ReputationScopeContext<'_>,
        scopes: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            scope: completeness::cell(PATH, source.scope())?.try_map(|link| {
                RecordLink::read_source(PATH, link, scopes, |target| {
                    target == Kind::ReputationScopes.records().type_name
                })
            })?,
        })
    }
}
