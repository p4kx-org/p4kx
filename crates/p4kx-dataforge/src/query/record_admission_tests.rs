// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Mutex;

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use super::{
    Query, QueryAdmissionOutcome, QueryRecordAdmission, QueryTypeSource, ReferenceCoverage,
};
use crate::DataForge;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn from_and_at_report_actual_raw_candidates_and_excluded_headers() -> TestResult {
    let bytes = with_unreadable_path()?;
    let forge = DataForge::open(&bytes)?;
    let query = Query::parse("from Base at \"libs/wanted*\"")?;
    let mut types = Vec::new();
    let (compiled, references) =
        query.compile_observing_selection(&forge, |site| types.push(site), |_| {})?;
    assert_eq!(
        references,
        ReferenceCoverage::Complete,
        "fixture performs no Reference lookup"
    );
    assert!(
        types.iter().any(|site| matches!(
            site,
            QueryTypeSource::Name {
                index: 0,
                span: Some(_)
            }
        )),
        "the selected original FROM type has its stored spelling"
    );
    assert!(
        types.iter().any(|site| matches!(
            site,
            QueryTypeSource::Parent {
                candidate: 1,
                index: 1,
                stored_parent: 0
            }
        )),
        "type admission follows the actual physical parent word"
    );
    let observed = Mutex::new(Vec::new());
    let traced = compiled
        .run_observing_records(
            || true,
            |_| {},
            |site| {
                observed
                    .lock()
                    .expect("the fixture owns candidate evidence")
                    .push(site);
            },
        )?
        .ok_or("candidate scan was not stopped")?;
    assert_eq!(
        traced.indexes(),
        query.compile(&forge)?.run()?.indexes(),
        "observed admission preserves the ordinary exact query selection"
    );
    assert_eq!(
        traced.indexes(),
        &[0],
        "the first physical inherited Record matches"
    );
    let mut observed = observed.into_inner()?;
    observed.sort_by_key(|row| row.record_index);
    check_outcomes(&observed, &bytes)
}

fn check_outcomes(observed: &[QueryRecordAdmission], bytes: &[u8]) -> TestResult {
    use QueryAdmissionOutcome::{Admitted, NoTypePlan, PathRejected, UnreadablePath};
    let [keep, path, kind, bad] = observed else {
        return Err("all four actual physical headers must be recorded".into());
    };
    for (row, expected) in [
        (keep, Admitted),
        (path, PathRejected),
        (kind, NoTypePlan),
        (bad, UnreadablePath),
    ] {
        assert_eq!(
            row.outcome, expected,
            "the first decision for physical Record {}",
            row.record_index
        );
    }
    assert!(
        keep.path_filter_checked && path.path_filter_checked,
        "AT evaluates actual matching type candidates"
    );
    assert!(
        !kind.path_filter_checked && !bad.path_filter_checked,
        "rejected earlier candidates never evaluate AT"
    );
    assert_eq!(
        kind.structure_index,
        Some(2),
        "FROM rejection retains the original type word"
    );
    assert_eq!(
        bad.structure_index, None,
        "the unreadable filename prevents type admission"
    );
    assert_eq!(
        bad.file_name, None,
        "there is no fabricated raw filename span"
    );
    let range = keep
        .file_name
        .as_ref()
        .ok_or("the exact original filename extent")?;
    let range = usize::try_from(range.start)?..usize::try_from(range.end)?;
    assert_eq!(
        bytes.get(range),
        Some(b"libs/wanted_one.xml\0".as_slice()),
        "the stored filename includes its stripped suffix and checked terminator"
    );
    let range = keep
        .record_name
        .as_ref()
        .ok_or("the exact original ordering-name extent")?;
    let range = usize::try_from(range.start)?..usize::try_from(range.end)?;
    assert_eq!(
        bytes.get(range),
        Some(b"One\0".as_slice()),
        "the ordering-name extent includes its checked terminator"
    );
    Ok(())
}

fn with_unreadable_path() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Base"))
        .structure(TestStruct::new("Child").parent("Base"))
        .structure(TestStruct::new("Other"))
        .record(TestRecord::new(1, "Child", "One", "libs/wanted_one.xml"))
        .record(TestRecord::new(
            1,
            "Child",
            "OtherPath",
            "libs/elsewhere.xml",
        ))
        .record(TestRecord::new(
            3,
            "Other",
            "OtherType",
            "libs/wanted_other.xml",
        ))
        .record(TestRecord::new(4, "Child", "Bad", "libs/wanted_bad.xml"))
        .build();
    let start = {
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(3).ok_or("original bad-path fixture Record")?;
        let span = forge
            .original_text_span(record.stored_file_name()?)
            .ok_or("original filename bytes")?;
        usize::try_from(span.start)?
    };
    *bytes
        .get_mut(start)
        .ok_or("the first byte of the original filename")? = 0xff;
    Ok(bytes)
}
