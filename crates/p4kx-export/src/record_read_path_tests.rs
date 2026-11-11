// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_extraction::{Filter, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use super::read_kind;
use crate::Kind;
use crate::build::original_input::test_support::{self, TestResult};
use crate::guid_text::CatalogGuid;

const PATH: &str = "libs/foundry/records/callback/unreadable.xml";

#[test]
fn dedicated_kind_prefilter_does_not_replace_the_public_generic_selection_scan() -> TestResult {
    let bytes = unreadable_path_bytes()?;
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        assert!(
            record.record_path().is_err(),
            "genuine lazy metadata path failure"
        );
        assert_eq!(
            record.type_name()?,
            "ContractTemplate",
            "independently readable type"
        );
        let calls = AtomicUsize::new(0);
        let read = read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            |record, _tree| {
                calls.fetch_add(1, Ordering::Relaxed);
                Ok(CatalogGuid(record.guid()))
            },
            |guid| *guid,
        )?;
        assert!(
            read.items.is_empty(),
            "the dedicated path rule cannot select this source"
        );
        assert!(
            read.failures.is_empty(),
            "this internal reader is not the generic candidate scan"
        );
        assert!(
            read.ledger.sources.is_empty(),
            "no fabricated known-kind candidate"
        );
        assert_eq!(
            calls.load(Ordering::Relaxed),
            0,
            "no projection through an unreadable path"
        );
        let inventory = input.origins().uses.freeze()?;
        assert!(
            inventory.uses().iter().all(|used| used.role() != SourceUseRole::Payload
                || !matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
                    if namespace == "dataforgeRecordHeader" && (field == "guid" || field == "recordName"))),
            "the unreadable prefilter creates no unused GUID/name Payload claims"
        );
        Ok(())
    })
}
fn unreadable_path_bytes() -> TestResult<Vec<u8>> {
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractTemplate"))
        .record(TestRecord::new(
            0x5411,
            "ContractTemplate",
            "ContractTemplate.Subject",
            PATH,
        ))
        .build();
    let positions: Vec<_> = bytes
        .windows(PATH.len())
        .enumerate()
        .filter_map(|(index, value)| (value == PATH.as_bytes()).then_some(index))
        .collect();
    assert_eq!(positions.len(), 1, "one actual original fileName token");
    let position = *positions.first().ok_or("unique original fileName")?;
    *bytes.get_mut(position).ok_or("original fileName byte")? = 0xff;
    Ok(bytes)
}

#[test]
fn filtered_and_successful_unretained_subjects_create_no_unused_header_payloads() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractTemplate"))
        .record(TestRecord::new(
            0x5412,
            "ContractTemplate",
            "ContractTemplate.Subject",
            "libs/foundry/records/kept/one.xml",
        ))
        .build();
    test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("actual original Record")?;
        assert!(
            record.tree()?.is_from(&record),
            "the selected original body really is readable"
        );
        let filter = Filter::new("unselected/*");
        let filtered = read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (Some(&filter), None),
            |record, _| Ok(record.index()),
            |_| CatalogGuid(p4kx_extraction::Guid::default()),
        )?;
        assert!(
            filtered.items.is_empty() && filtered.ledger.sources.is_empty(),
            "ordinary filtering never invents an admitted subject"
        );
        no_unused_payload(&input.origins().uses.freeze()?);
        let read = read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            |record, _| Ok(record.index()),
            |_| CatalogGuid(p4kx_extraction::Guid::default()),
        )?;
        assert_eq!(
            read.items,
            [record.index()],
            "actual successful projection retained its scalar"
        );
        assert!(
            read.failures.is_empty(),
            "the real selected source succeeded"
        );
        let mut ledger = read.ledger;
        for kind in Kind::ALL {
            ledger.inventory(kind);
        }
        let rows = [crate::KindSummary {
            kind: Kind::ContractTemplates,
            count: 1,
            skipped: 0,
            complete: true,
        }];
        let prepared = crate::export_health_compile::prepare_report(
            ledger,
            &rows,
            [],
            &[],
            crate::CatalogPublicationMode::Normal,
        )?;
        let health = prepared.finish(input, &[], None)?;
        assert!(
            health.failed_records.is_empty() && health.diagnostics.is_empty(),
            "the successful subject has no retained diagnostic occurrence"
        );
        assert!(
            health.metadata().texts().is_empty(),
            "unretained header text does not become a report storage atom"
        );
        no_unused_payload(&input.origins().uses.freeze()?);
        Ok(())
    })
}

fn no_unused_payload(inventory: &SourceInventory) {
    assert!(
        inventory
            .uses()
            .iter()
            .all(|used| used.role() != SourceUseRole::Payload
                || !matches!(used.location(), SourceLocation::Subresource { namespace, field, .. }
            if namespace == "dataforgeRecordHeader" && (field == "guid" || field == "recordName"))),
        "selection and successful unretained subjects mint no unused GUID/name payloads"
    );
}
