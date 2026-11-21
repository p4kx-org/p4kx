// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Record, ReferenceIndex, Value, ValueError};
use serde_json::{Value as Json, json};

#[path = "dcb_refs_origin_controls.rs"]
mod controls;
#[path = "dcb_refs_origin_fixture.rs"]
mod fixture;
#[path = "dcb_refs_origin_scalar_error.rs"]
mod scalar_error;

use self::fixture::{Fixture, Input, Site, TestResult, array, header, route, site, value};

struct Original {
    linked: Site,
    missing: Site,
    other_missing: Site,
    alpha: u32,
    beta: u32,
    gamma: u32,
}

fn original(forge: &DataForge<'_>) -> TestResult<Original> {
    let alpha = forge.record(2).ok_or("original Alpha")?;
    let beta = forge.record(3).ok_or("original Beta")?;
    let gamma = forge.record(4).ok_or("original Gamma")?;
    let (linked, missing, other_missing) = original_sites(&alpha, &beta, &gamma)?;
    assert_eq!(
        beta.record_path()?,
        gamma.record_path()?,
        "Beta and Gamma share the same display path"
    );
    assert_ne!(
        beta.guid(),
        gamma.guid(),
        "the Records are physically distinct"
    );
    let index = ReferenceIndex::build(forge);
    assert_eq!(
        index.unreadable_records(),
        0,
        "the controlled Record trees all build"
    );
    let incoming: Vec<_> = index.incoming(beta.index()).collect();
    let [link] = incoming.as_slice() else {
        return Err("one indexed original Beta reference".into());
    };
    assert_eq!(
        (link.source(), link.target(), link.indices()),
        (alpha.index(), Some(beta.index()), &[1][..]),
        "the index retains the actual decoder target and array occurrence"
    );
    assert_eq!(
        index.incoming(gamma.index()).len(),
        0,
        "the exact same-path Gamma Record has no incoming reference"
    );
    assert_eq!(
        index.unresolved().count(),
        2,
        "both known missing GUID occurrences are indexed"
    );
    assert_eq!(
        beta.stored_file_name()?,
        "libs/ships/beta.xml",
        "the raw original filename retains its extension"
    );
    Ok(Original {
        linked,
        missing,
        other_missing,
        alpha: alpha.index(),
        beta: beta.index(),
        gamma: gamma.index(),
    })
}

fn original_sites<'source>(
    alpha: &Record<'source>,
    beta: &Record<'source>,
    gamma: &Record<'source>,
) -> TestResult<(Site, Site, Site)> {
    let alpha_tree = alpha.tree()?;
    let gamma_tree = gamma.tree()?;
    assert!(
        alpha_tree.is_from(alpha) && gamma_tree.is_from(gamma),
        "both trees borrow their actual Records"
    );
    let Some(Value::Array(escorts)) = alpha_tree.field("escorts") else {
        return Err("actual Alpha array".into());
    };
    let [
        Value::Null,
        linked @ Value::Reference(target),
        missing @ Value::Error(ValueError::DanglingReference { guid }),
    ] = escorts.as_ref()
    else {
        return Err("actual null, resolved and dangling slots in order".into());
    };
    let other = gamma_tree.field("maker").ok_or("actual Gamma maker slot")?;
    assert!(
        matches!(other, Value::Error(ValueError::DanglingReference { guid: actual }) if actual == guid),
        "a second physical occurrence contains the same missing GUID"
    );
    let admitted = target.original_target();
    assert_eq!(
        admitted.record()?.index(),
        beta.index(),
        "the actual decoder selected Beta"
    );
    assert!(
        admitted.shares_source_with(alpha),
        "the target belongs to the original Forge"
    );
    Ok((
        site(&alpha_tree, linked)?,
        site(&alpha_tree, missing)?,
        site(&gamma_tree, other)?,
    ))
}

#[test]
fn incoming_reference_fields_retain_the_original_holder_and_occurrence() -> TestResult {
    let fixture = Fixture::open(&super::database(), &Input::ALL, original)?;
    for case in &fixture.inputs {
        let document = case.run(&["libs/ships/beta", "--incoming", "--json"])?;
        let [row, same_path] = array(&document, "records")? else {
            return Err("both physical same-path listings".into());
        };
        assert_eq!(
            row.get("reference_count"),
            Some(&json!(1)),
            "incoming count"
        );
        assert!(
            array(same_path, "references")?.is_empty(),
            "the same-path Gamma has no incoming link"
        );
        let [link] = array(row, "references")? else {
            return Err("one resolved incoming link".into());
        };
        check_route(case, &document, link)?;
        let holder = case.uses(&document, link, "/record/guid")?;
        header(&holder, case.original.alpha, "guid", "payload");
        value(&holder, case.original.linked, "$value", "selector");
        check_empty_incoming(case, &document, same_path)?;
    }
    Ok(())
}

fn check_empty_incoming(case: &fixture::Case<Original>, document: &Json, row: &Json) -> TestResult {
    let target = case.original.gamma;
    let record = case.uses(document, row, "/record/guid")?;
    header(&record, target, "guid", "payload");
    for field in ["reference_count", "record_count"] {
        assert_eq!(
            row.get(field),
            Some(&json!(0)),
            "no links for the exact target"
        );
        let uses = case.uses(document, row, &format!("/{field}"))?;
        header(&uses, target, "guid", "selector");
        assert!(
            !fixture::has_header(&uses, case.original.beta, "guid", "selector"),
            "the other Record at the same path is not the zero-count query target"
        );
    }
    Ok(())
}

fn check_route(case: &fixture::Case<Original>, document: &Json, link: &Json) -> TestResult {
    assert_eq!(
        link.get("location"),
        Some(&json!("escorts[1]")),
        "authored array slot"
    );
    assert_eq!(
        link.get("field"),
        Some(&json!("escorts[]")),
        "existing field shape"
    );
    assert_eq!(
        link.get("indices"),
        Some(&json!([1])),
        "original numeric index"
    );
    let site = case.original.linked;
    let location = case.uses(document, link, "/location")?;
    route(&location, site, "fieldDeclarations", None);
    route(&location, site, "arrayPositions", None);
    let field = case.uses(document, link, "/field")?;
    route(&field, site, "fieldDeclarations", None);
    assert!(
        field
            .iter()
            .all(|used| used.pointer("/location/field") != Some(&json!("arrayPositions"))),
        "the numeric position does not contribute to the field shape"
    );
    let indices = case.uses(document, link, "/indices")?;
    route(&indices, site, "arrayPositions", None);
    assert!(
        indices
            .iter()
            .all(|used| { used.pointer("/location/field") != Some(&json!("fieldDeclarations")) }),
        "the declaration names do not contribute to the numeric array indices"
    );
    let first_index = case.uses(document, link, "/indices/0")?;
    route(&first_index, site, "arrayPosition", Some(0));
    for contributors in [&location, &field, &indices, &first_index] {
        assert!(
            !fixture::has_value(contributors, site, "$value", "conversion"),
            "the stored reference value does not supply the decoded route"
        );
    }
    Ok(())
}

#[test]
fn outgoing_reference_fields_retain_the_decoder_selected_target() -> TestResult {
    let fixture = Fixture::open(&super::database(), &Input::ALL, original)?;
    for case in &fixture.inputs {
        let document = case.run(&["libs/ships/alpha", "--outgoing", "--format", "json"])?;
        let [row] = array(&document, "records")? else {
            return Err("one physical outgoing listing".into());
        };
        assert_eq!(
            row.get("reference_count"),
            Some(&json!(2)),
            "outgoing count"
        );
        assert_eq!(
            row.get("unresolved_count"),
            Some(&json!(1)),
            "separate dangling count"
        );
        let [_, link] = array(row, "references")? else {
            return Err("two ordered outgoing links".into());
        };
        assert_eq!(
            link.get("location"),
            Some(&json!("escorts[1]")),
            "linked occurrence"
        );
        let target = case.uses(&document, link, "/record/guid")?;
        header(&target, case.original.beta, "guid", "payload");
        value(&target, case.original.linked, "$value", "selector");
        let [missing] = array(row, "unresolved")? else {
            return Err("one known missing target".into());
        };
        assert_eq!(missing.get("record"), Some(&Json::Null), "missing target");
        value(
            &case.uses(&document, missing, "/record")?,
            case.original.missing,
            "$error.guid",
            "selector",
        );
        let guid = case.uses(&document, missing, "/guid")?;
        assert_eq!(guid.len(), 1, "raw missing GUID retains its own sole input");
        value(&guid, case.original.missing, "$error.guid", "payload");
    }
    Ok(())
}

#[test]
fn unresolved_reference_fields_keep_equal_guids_at_distinct_original_sites() -> TestResult {
    let fixture = Fixture::open(&super::database(), &Input::ALL, original)?;
    for case in &fixture.inputs {
        let document = case.run(&["--unresolved", "--json"])?;
        assert_eq!(
            document.get("count"),
            Some(&json!(2)),
            "two missing GUID occurrences"
        );
        let [first, second] = array(&document, "unresolved")? else {
            return Err("both unresolved occurrences".into());
        };
        assert_eq!(
            first.get("guid"),
            second.get("guid"),
            "equal raw missing GUIDs"
        );
        assert_eq!(
            first.get("location"),
            Some(&json!("escorts[2]")),
            "first source slot"
        );
        assert_eq!(
            second.get("location"),
            Some(&json!("maker")),
            "second source slot"
        );
        for (row, site, holder) in [
            (first, case.original.missing, case.original.alpha),
            (second, case.original.other_missing, case.original.gamma),
        ] {
            let guid = case.uses(&document, row, "/guid")?;
            assert_eq!(
                guid.len(),
                1,
                "the raw GUID has its own sole original input"
            );
            value(&guid, site, "$error.guid", "payload");
            header(
                &case.uses(&document, row, "/record/guid")?,
                holder,
                "guid",
                "payload",
            );
        }
        case.uses(&document, &document, "/count")?;
    }
    Ok(())
}

#[test]
fn file_graph_edge_fields_keep_original_file_representatives_and_link_witnesses() -> TestResult {
    let fixture = Fixture::open(&super::database(), &Input::ALL, original)?;
    for case in &fixture.inputs {
        let document = case.run(&["--files", "--json"])?;
        assert_eq!(
            document.get("file_count"),
            Some(&json!(4)),
            "existing file count"
        );
        assert_eq!(
            document.get("edge_count"),
            Some(&json!(4)),
            "existing edge count"
        );
        let edge = array(&document, "edges")?
            .iter()
            .find(|edge| {
                edge.get("from") == Some(&json!("libs/ships/alpha"))
                    && edge.get("to") == Some(&json!("libs/ships/beta"))
            })
            .ok_or("the actual Alpha-to-Beta edge")?;
        for (field, index) in [("/from", case.original.alpha), ("/to", case.original.beta)] {
            let uses = case.uses(&document, edge, field)?;
            header(&uses, index, "fileName", "conversion");
            value(&uses, case.original.linked, "$value", "selector");
        }
        case.uses(&document, &document, "/file_count")?;
        case.uses(&document, &document, "/edge_count")?;
    }
    Ok(())
}
