// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveFiles, Entities, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutResolver,
};
use p4kx_extraction::{DataForge, SourceUseRole, SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout};

use crate::build::original_input::test_support::TestResult;
use crate::build::original_input::{OriginalCatalogInput, ship_pool_test_input};
use crate::catalog_read::original::{
    ExampleBodyReceipt, health_example_retention::RetainedExampleBody,
};
use crate::contract_loadout::EntityKind;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;
use crate::{CatalogError, ExportAccountingError, Kind};

#[path = "health_loadout_example_assertions.rs"]
mod assertions;

use self::assertions::{assert_cached_requests, assert_third_origins};

const REQUEST: &str = "synthetic/request-missing.xml";

fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(71, "Source", "synthetic/source.xml")
                .loadout(TestLoadout::Xml(REQUEST.to_owned()))
                .record(),
        )
        .build()
}

fn body<'input>(
    files: &'input ArchiveFiles<'input>,
    forge: &'input DataForge<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
) -> TestResult<ExampleBodyReceipt> {
    let record = forge.record(0).ok_or("the original source Record")?;
    let tree = record.tree()?;
    assert!(
        tree.is_from(&record),
        "the independent original canonical tree belongs to the Record"
    );
    assert!(
        matches!(
            files.read_observed(REQUEST).outcome(),
            Err(LoadoutFileError::NotFound)
        ),
        "the actual original Archive has no selected file"
    );
    let capture = Arc::new(Capture::for_input(
        input,
        &ExportLedger::default(),
        None,
        Kind::Loadouts,
    ));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), capture.clone())
            .with_files(files);
    let node = resolver.resolve(&record)?;
    assert!(node.issues().iter().any(|issue| matches!(issue,
        LoadoutError::MissingXmlLoadout { path } if path.value() == REQUEST && path.selection().is_some())),
        "the actual original getter pair survives the genuine missing read");
    let before_factory = input.origins().uses.freeze()?;
    capture.root(&record, EntityKind::Ship, Ok(&node));
    let ledger = capture.finish()?;
    assert_eq!(
        before_factory,
        input.origins().uses.freeze()?,
        "the actual unretained request-body construction adds no source uses after the reader work"
    );
    let diagnostic = ledger
        .field_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.warning_code.as_deref() == Some("xml_loadout"))
        .ok_or("the actual typed missing issue")?;
    let example = diagnostic
        .examples
        .first()
        .ok_or("the original issue example")?;
    Ok(example.body()?.clone())
}

#[test]
fn request_body_keeps_original_text_and_defers_payload_until_retention() -> TestResult {
    ship_pool_test_input::with_input(&database(), Vec::new(), |_, files, forge, input| {
        let mut original = body(files, forge, input)?;
        let before = input.origins().uses.freeze()?;
        original.merge(original.clone())?;
        let description = original.description()?;
        assert_eq!(
            description.0.as_deref(),
            Some(REQUEST),
            "the complete genuine request text"
        );
        assert_eq!(
            description.1.as_deref(),
            Some("EntityClassDefinition"),
            "the owning Record Header type"
        );
        assert!(description.2.is_none(), "no fabricated target GUID");
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "unretained clone/merge/description adds no payload"
        );
        let retained = RetainedExampleBody::read(original, input)?;
        let fields = retained.texts();
        let field = fields.first().ok_or("the required retained field")?;
        assert!(
            matches!(field.value(), SourceValue::Present(text) if text.text().as_str() == REQUEST),
            "retention materializes the same exact pooled text"
        );
        assert!(
            field
                .contributions()
                .0
                .iter()
                .any(|used| used.role() == SourceUseRole::Payload),
            "the genuine original request supplies retained field payload"
        );
        Ok(())
    })
}

#[test]
fn a_genuine_request_body_refuses_an_independent_report_before_retaining_payload() -> TestResult {
    ship_pool_test_input::with_input(&database(), Vec::new(), |_, files, forge, input| {
        let original = body(files, forge, input)?;
        ship_pool_test_input::with_input(&database(), Vec::new(), |_, files, forge, foreign| {
            let own = body(files, forge, foreign)?;
            assert_eq!(
                original.description()?,
                own.description()?,
                "equal source values do not pair the reports"
            );
            let before = foreign.origins().uses.freeze()?;
            let outcome = RetainedExampleBody::read(original, foreign);
            assert!(
                matches!(
                    outcome,
                    Err(CatalogError::Accounting(
                        ExportAccountingError::UnboundHealthSubject
                            | ExportAccountingError::ConflictingSourceDatum
                    ))
                ),
                "a genuine foreign owner cannot authorize this report's retained body"
            );
            assert_eq!(
                before,
                foreign.origins().uses.freeze()?,
                "foreign refusal registers no payload"
            );
            Ok(())
        })
    })
}

#[path = "health_loadout_cache_test_probe.rs"]
mod cache_probe;

#[test]
fn actual_shared_resolver_cache_keeps_the_first_request_and_the_second_caller() -> TestResult {
    with_counted_requests(2, |ledgers, input| {
        let first = ledgers.first().ok_or("the genuine first caller ledger")?;
        let second = ledgers.get(1).ok_or("the genuine second caller ledger")?;
        assert_cached_requests(first, second, input)
    })
}

fn with_counted_requests(
    root_count: u32,
    check: impl FnOnce(&[ExportLedger], &OriginalCatalogInput<'_, '_>) -> TestResult,
) -> TestResult {
    let mut bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new());
    for index in 0..root_count {
        bytes = bytes.record(
            TestEntity::new(
                71 + u128::from(index),
                format!("Source{index}"),
                format!("source{index}.xml"),
            )
            .loadout(TestLoadout::Xml(REQUEST.to_owned()))
            .record(),
        );
    }
    ship_pool_test_input::with_input(&bytes.build(), Vec::new(), |_, files, forge, input| {
        assert_eq!(
            forge.records().count(),
            usize::try_from(root_count)?,
            "all genuine physical source Records"
        );
        let capture = Arc::new(Capture::for_input(
            input,
            &ExportLedger::default(),
            None,
            Kind::Loadouts,
        ));
        let probe = Arc::new(cache_probe::Probe::new(Arc::clone(&capture)));
        let mut resolver =
            LoadoutResolver::with_observed_entities(Arc::new(Entities::new(forge)), probe.clone())
                .with_files(files);
        let mut ledgers = Vec::new();
        for index in 0..root_count {
            let record = forge
                .record(index)
                .ok_or("the actual sequential source Record")?;
            assert_eq!(
                record.tree()?.error_count(),
                0,
                "the original source tree has no masking read failure"
            );
            let node = resolver.resolve(&record)?;
            assert!(node.issues().iter().any(|issue| matches!(issue,
                LoadoutError::MissingXmlLoadout { path } if path.value() == REQUEST && path.selection().is_some())),
                "the real SDK retains the same actually selected Missing request");
            probe.counts(1, usize::try_from(index)?);
            capture.root(&record, EntityKind::Ship, Ok(&node));
            ledgers.push(capture.finish()?);
        }
        check(&ledgers, input)
    })
}

#[test]
fn a_third_shared_document_caller_excludes_the_independent_middle_request() -> TestResult {
    with_counted_requests(3, |ledgers, input| {
        let third = ledgers.get(2).ok_or("the genuine third caller ledger")?;
        let diagnostic = third
            .field_diagnostics
            .iter()
            .find(|row| row.warning_code.as_deref() == Some("xml_loadout"))
            .ok_or("the actual third-call missing request diagnostic")?;
        assert_eq!(
            diagnostic.count, 1,
            "the third actual caller produces one issue"
        );
        assert_eq!(
            diagnostic.examples.len(),
            1,
            "one physical third caller example"
        );
        let example = diagnostic
            .examples
            .first()
            .ok_or("the actual third caller example")?;
        let retained = RetainedExampleBody::read(example.body()?.clone(), input)?;
        assert_third_origins(&retained)
    })
}
