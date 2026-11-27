// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io::{self, ErrorKind};

use p4kx_dataforge::{FlatFormat, Resolution, TextResolver};

use super::{HEAD, TestResult, Texts, flat};

struct UnsupportedText;

impl TextResolver for UnsupportedText {
    fn resolve<'a>(&'a self, _: &str) -> Resolution<'a> {
        Resolution::Text(Cow::Borrowed("a,\té"))
    }
}

#[test]
fn quotes_missing_fields_and_preserves_numeric_and_annotated_values() -> TestResult {
    let text = flat("=name", FlatFormat::CsvStrict, Some(&Texts))?;
    assert!(
        text.starts_with(
            "record_guid,record_name,record_path,field_path,array_indices,kind,value,target_path,text,record_tag\r\n"
        ),
        "strict flat CSV has a header with no BOM"
    );
    assert!(
        text.contains(&format!("{HEAD}small,\"\",int,-5,\"\",\"\",\"\"\r\n")),
        "numeric cells remain unmodified and absent cells are quoted"
    );
    assert!(
        text.contains(&format!("{HEAD}name,\"\",string,=name,\"\",\"\",\"\"\r\n")),
        "the field retains the original formula character"
    );
    assert!(
        text.contains(&format!(
            "{HEAD}label,\"\",locale,@item_Name,\"\",\"=Crate, \"\"big\"\"\",\"\"\r\n"
        )),
        "locale text is escaped through the same strict writer"
    );
    Ok(())
}

#[test]
fn rejects_non_ascii_cells_and_unsupported_locale_annotations() {
    for (name, texts) in [
        ("é", None),
        ("value", Some(&UnsupportedText as &dyn TextResolver)),
    ] {
        let error = flat(name, FlatFormat::CsvStrict, texts)
            .expect_err("strict flat CSV refuses unsupported field data");
        assert_eq!(
            error.downcast_ref::<io::Error>().map(io::Error::kind),
            Some(ErrorKind::InvalidData),
            "the flat writer preserves the invalid data error"
        );
    }
}
