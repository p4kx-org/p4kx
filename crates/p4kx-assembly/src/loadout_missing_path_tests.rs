// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, Value};
use p4kx_testkit::{ArchiveBuilder, TestEntry, TestType};

use super::{Probe, TestResult, database};
use crate::loadout_file_trace::FileTrace;
use crate::{ArchiveFiles, LoadoutError, LoadoutPathOperation, LoadoutRequest, LoadoutSelection};

const PATH: &str = "synthetic/missing.xml";

#[test]
fn a_missing_xml_error_retains_the_actual_getter_selection() -> TestResult {
    let bytes = database(TestType::String, PATH);
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("the actual Record is absent")?;
    let tree = record.tree()?;
    let Some(Value::Struct(parent)) = tree.field("path") else {
        return Err("the actual parent is absent".into());
    };
    assert!(
        matches!(parent.field("loadoutPath"), Some(Value::String(value)) if *value == PATH),
        "the original field stores the complete requested path"
    );
    let probe = Probe::default();
    let path = crate::loadout_path::PathRead::new(Some(&probe), None).field(
        parent,
        "loadoutPath",
        LoadoutPathOperation::Copy,
    );
    let expected = probe
        .0
        .borrow()
        .first()
        .cloned()
        .ok_or("the actual getter was not observed")?;
    let folder = tempfile::tempdir()?;
    let file = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/unrelated.xml",
            b"<Loadout/>".as_slice(),
        ))
        .write_to(&file)?;
    let archive = p4kx_archive::Archive::open(file)?;
    let files = ArchiveFiles::new(&archive);
    let caller = LoadoutRequest::Xml { path: path.clone() };
    let error = crate::xml::load_observed(&files, &path, &caller, None, &mut FileTrace::default())
        .err()
        .ok_or("the actual absent file was accepted")?;
    assert_missing_error(&error, &expected)
}

fn assert_missing_error(error: &LoadoutError, expected: &LoadoutSelection) -> TestResult {
    let LoadoutError::MissingXmlLoadout { path: failed } = error else {
        return Err(format!("wrong actual missing outcome: {error}").into());
    };
    assert_eq!(
        failed.value(),
        PATH,
        "the error keeps the original complete text"
    );
    assert_eq!(
        failed.selection(),
        Some(expected),
        "the whole genuine getter pair survives the error"
    );
    assert!(
        crate::LoadoutPath::unbound(failed.value())
            .selection()
            .is_none(),
        "copying the same error text cannot mint its original selection"
    );
    assert!(
        failed.attempts().is_empty() && failed.caller().is_none(),
        "an unobserved read does not invent source evidence from a previously observed getter"
    );
    assert_eq!(
        error.kind(),
        "xml_loadout",
        "the script code remains unchanged"
    );
    assert_eq!(
        error.to_string(),
        format!("the XML Loadout {PATH} cannot be read: there is no such file"),
        "the display remains unchanged"
    );
    Ok(())
}
