// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::ops::Range;

use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::read_loadouts;
use roxmltree::{Document, Node};
use serde_json::{Value, json};

use super::{catalog_from, fixture, loadout, proof};
use crate::commodity_v3_fixture::TestResult;

const VEHICLE: &str = r#"<Vehicle name="FixtureTopology">
    <Parts>
        <Part id="earlier" name="different_port">
            <ItemPort display_name="@earlier"/>
        </Part>
        <Part id="disabled" name="disabled_parent" skipPart="true">
            <Parts>
                <Part id="hidden" name="vehicle_only">
                    <ItemPort display_name="@hidden"/>
                </Part>
            </Parts>
        </Part>
        <Part id="parent" name="selected_parent">
            <Parts>
                <Part id="selected" name="old_port">
                    <Other/>
                    <ItemPort display_name="@winner"/>
                    <ItemPort display_name="@later_child"/>
                </Part>
            </Parts>
        </Part>
        <Part id="later" name="vehicle_only">
            <ItemPort display_name="@later_part"/>
        </Part>
    </Parts>
    <Modifications>
        <Modification name="variant">
            <Elems>
                <Elem idRef="selected" name="name" value="vehicle_only"/>
            </Elems>
        </Modification>
    </Modifications>
</Vehicle>"#;

#[test]
fn vehicle_port_fields_retain_the_original_first_match_prefix_in_catalog_and_sdk() -> TestResult {
    for encrypted in [false, true] {
        let (_directory, archive) =
            fixture::graph_archive_with_vehicle(VEHICLE.as_bytes(), encrypted)?;
        check_cached_port_callers(&catalog_from(&archive)?, encrypted)?;
        let selected = read_loadouts(&archive, CatalogRecordSelection::All, None)?;
        check_cached_port_callers(
            &json!({"loadouts": selected.rows, "provenance": selected.source_inventory}),
            encrypted,
        )?;
    }
    Ok(())
}

fn check_cached_port_callers(projection: &Value, encrypted: bool) -> TestResult {
    for guid in [fixture::SHIP, fixture::SHIP_SAME_VEHICLE] {
        let (_, ship) = loadout(projection, guid)?;
        check_projection(projection, ship, encrypted)?;
    }
    Ok(())
}

fn check_projection(projection: &Value, ship: &Value, encrypted: bool) -> TestResult {
    let port = proof::rows(ship, "/ports")?
        .iter()
        .find(|row| row.pointer("/name/value").and_then(Value::as_str) == Some("vehicle_only"))
        .ok_or("the XML-selected vehicle port")?;
    let cell = proof::cell(port, "displayNameKey", "present")?;
    assert_eq!(
        cell["value"], "@winner",
        "the first effective Part and ItemPort decide"
    );
    let expected = format!("Data\\{}", fixture::SHIP_XML.replace('/', "\\"));
    let mut original = Vec::new();
    let mut caller_indices = BTreeSet::new();
    for site in proof::uses(projection, cell)? {
        if site.pointer("/location/namespace").and_then(Value::as_str)
            == Some("dataforgeRecordHeader")
            && site.pointer("/location/field").and_then(Value::as_str) == Some("recordName")
        {
            let index = proof::at(site, "/location/indices/0")?
                .as_str()
                .ok_or("physical caller index")?;
            caller_indices.insert(index);
        }
        let physical = proof::source(projection, site)?;
        if physical.pointer("/path/value").and_then(Value::as_str) == Some(expected.as_str()) {
            original.push(site);
        }
    }
    check_selectors(&original)?;
    let index = proof::at(ship, "/sourceRecordIndex/value")?
        .as_u64()
        .ok_or("current owner index")?
        .to_string();
    assert_eq!(
        caller_indices,
        BTreeSet::from([index.as_str()]),
        "shared XML resolution cites only today's physical DataForge caller",
    );
    proof::present(port, "displayNameKey", &json!("@winner"), Some(encrypted))?;
    proof::closure(projection)
}

fn check_selectors(original: &[&Value]) -> TestResult {
    let document = Document::parse(VEHICLE)?;
    let parent = part(&document, "parent")?;
    let selected = part(&document, "selected")?;
    let before_port = selected
        .children()
        .find(|node| node.has_tag_name("Other"))
        .ok_or("first non-port child")?;
    let winner = item_port(selected)?;
    let donor = document
        .descendants()
        .find(|node| node.has_tag_name("Elem") && node.attribute("idRef") == Some("selected"))
        .ok_or("authored name donor")?;
    for (node, field) in [
        (part(&document, "earlier")?, "name"),
        (part(&document, "disabled")?, "skipPart"),
        (donor, "idRef"),
        (donor, "value"),
    ] {
        let range = node
            .attribute_node(field)
            .ok_or("exact selector attribute")?
            .range();
        assert!(
            has_site(original, range, "textXmlAttribute", field, "selector"),
            "the exact original {field} occurrence supplies selection",
        );
    }
    for (node, field) in [
        (parent, "$children"),
        (before_port, "$name"),
        (winner, "$name"),
    ] {
        assert!(
            has_site(original, node.range(), "textXmlElement", field, "selector"),
            "the original ancestor and first child search supply selection",
        );
    }
    let display = winner
        .attribute_node("display_name")
        .ok_or("selected name")?;
    assert!(
        has_site(
            original,
            display.range(),
            "textXmlAttribute",
            "display_name",
            "payload"
        ),
        "only the selected port supplies the displayed key",
    );
    check_unvisited(original, &document)
}

fn check_unvisited(original: &[&Value], document: &Document<'_>) -> TestResult {
    let later = part(document, "later")?
        .attribute_node("name")
        .ok_or("later duplicate")?;
    assert!(
        !has_site(
            original,
            later.range(),
            "textXmlAttribute",
            "name",
            "selector"
        ),
        "the later duplicate does not select the first match",
    );
    let skipped = item_port(part(document, "hidden")?)?
        .attribute_node("display_name")
        .ok_or("skipped child display name")?;
    assert!(
        !has_site(
            original,
            skipped.range(),
            "textXmlAttribute",
            "display_name",
            "payload"
        ),
        "unvisited descendant payload does not supply the result",
    );
    Ok(())
}

fn part<'document, 'source>(
    document: &'document Document<'source>,
    id: &str,
) -> TestResult<Node<'document, 'source>> {
    document
        .descendants()
        .find(|node| node.has_tag_name("Part") && node.attribute("id") == Some(id))
        .ok_or_else(|| format!("missing fixture Part {id}").into())
}

fn item_port<'document, 'source>(
    part: Node<'document, 'source>,
) -> TestResult<Node<'document, 'source>> {
    part.children()
        .find(|node| node.has_tag_name("ItemPort"))
        .ok_or_else(|| "missing original ItemPort".into())
}

fn has_site(
    uses: &[&Value],
    range: Range<usize>,
    namespace: &str,
    field: &str,
    role: &str,
) -> bool {
    let indices = json!([range.start.to_string(), range.end.to_string()]);
    uses.iter().any(|original| {
        original.pointer("/location/indices") == Some(&indices)
            && original
                .pointer("/location/namespace")
                .and_then(Value::as_str)
                == Some(namespace)
            && original.pointer("/location/field").and_then(Value::as_str) == Some(field)
            && original.get("role").and_then(Value::as_str) == Some(role)
    })
}
