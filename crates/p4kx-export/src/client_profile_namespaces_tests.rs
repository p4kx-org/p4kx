// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::TestEntry;
use serde_json::Value;

use super::{GENERAL, TestResult, at, coordinate, location, observe, rows, state};

const XML: &str = concat!(
    "<Attributes xmlns:p='urn:outer'>",
    "<Attr p:gain='01'/>",
    "<Attr xmlns:p='urn:inner' p:gain='02'/>",
    "</Attributes>",
);

#[test]
fn namespace_declarations_and_qualified_settings_retain_distinct_authored_sites() -> TestResult {
    let report = observe(vec![TestEntry::stored(GENERAL, XML)], None)?;
    let document = serde_json::to_value(report)?;
    let general = at(&document, "/settings/documents/0")?;
    state(general, "read", true)?;
    let [entry] = rows(general, "/candidates")? else {
        return Err("the original General Entry".into());
    };
    let [root, inherited, shadowed] = rows(general, "/elements")? else {
        return Err("root and the two original settings".into());
    };
    assert_eq!(
        rows(root, "/declarations")?.len(),
        1,
        "only the root declaration"
    );
    assert_eq!(
        rows(inherited, "/declarations")?.len(),
        1,
        "namespace inheritance adds no authored declaration"
    );
    assert_eq!(
        rows(shadowed, "/declarations")?.len(),
        2,
        "the local namespace and its qualified setting"
    );

    for (element, pointer, name, value, raw) in [
        (
            root,
            "/declarations/0",
            "xmlns:p",
            "urn:outer",
            "xmlns:p='urn:outer'",
        ),
        (inherited, "/declarations/0", "p:gain", "01", "p:gain='01'"),
        (
            shadowed,
            "/declarations/0",
            "xmlns:p",
            "urn:inner",
            "xmlns:p='urn:inner'",
        ),
        (shadowed, "/declarations/1", "p:gain", "02", "p:gain='02'"),
    ] {
        let declaration = at(element, pointer)?;
        check_declaration(&document, entry, declaration, name, value, raw)?;
    }
    for child_attribute in [
        at(inherited, "/declarations/0")?,
        at(shadowed, "/declarations/1")?,
    ] {
        assert!(
            child_attribute
                .pointer("/namespace/value")
                .and_then(Value::as_str)
                .is_none(),
            "the namespace URI belongs to its original declaration"
        );
    }
    Ok(())
}

fn check_declaration(
    report: &Value,
    entry: &Value,
    declaration: &Value,
    name: &str,
    value: &str,
    raw: &str,
) -> TestResult {
    let name_cell = at(declaration, "/name")?;
    let value_cell = at(declaration, "/value/value")?;
    assert_eq!(
        at(name_cell, "/value")?,
        name,
        "the authored QName survives"
    );
    assert_eq!(
        at(value_cell, "/value")?,
        value,
        "the authored string stays in its own attribute"
    );
    let start = XML
        .find(raw)
        .ok_or("unique authored declaration in the fixture")?;
    let end = start + raw.len();
    let span = location(
        "textXmlAttribute",
        &[&start.to_string(), &end.to_string()],
        name,
    );
    coordinate(report, entry, name_cell, false, "selector", &span)?;
    coordinate(report, entry, value_cell, false, "payload", &span)
}
