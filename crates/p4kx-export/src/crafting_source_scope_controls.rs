// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, DataType, Guid, Record, RecordTree, StructValue, Value};

use super::scope_fixture::{BASE, ID, NAME, ROOT};
use super::support::{self, TestResult};

pub(super) fn original<'source>(forge: &'source DataForge<'source>) -> TestResult<Record<'source>> {
    schema(forge)?;
    let record = forge.record(0).ok_or("original scope owner")?;
    assert_eq!(record.type_name()?, ROOT, "the actual scope owner type");
    assert_eq!(
        record.record_path()?,
        "rewards/scopes",
        "the original scope owner path"
    );
    Ok(record)
}

fn schema(forge: &DataForge<'_>) -> TestResult {
    let name = forge
        .structs()
        .find(|structure| structure.name().is_ok_and(|name| name == NAME))
        .ok_or("original name-only scope")?;
    let identity = forge
        .structs()
        .find(|structure| structure.name().is_ok_and(|name| name == ID))
        .ok_or("original identity scope")?;
    assert_eq!(
        name.parent().ok_or("original base")?.name()?,
        BASE,
        "name scope has its actual base"
    );
    assert_eq!(
        identity.parent().ok_or("original base")?.name()?,
        BASE,
        "identity scope has its actual base"
    );
    assert_eq!(
        name.properties()
            .map(|property| property.name())
            .collect::<Result<Vec<_>, _>>()?,
        ["debugName", "children", "reward"],
        "name-only scopes truly have no id declaration"
    );
    assert_eq!(
        identity
            .properties()
            .map(|property| property.name())
            .collect::<Result<Vec<_>, _>>()?,
        ["debugName", "children", "reward", "id"],
        "only identity scopes actually declare id"
    );
    assert_eq!(
        (name.instance_count(), identity.instance_count()),
        (4, 2),
        "all original named and identity instances exist independently"
    );
    let survey = forge.survey()?;
    assert_eq!(
        (survey.unreadable_structs, survey.pointers.dangling),
        (0, 0),
        "all original layouts and every actual array/owned pointer target are readable"
    );
    Ok(())
}

pub(super) fn complete<'source>(
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> TestResult {
    support::clean(tree, record);
    root(forge, tree.root())?;
    let roots = support::array(tree.root(), "children")?;
    assert_eq!(roots.len(), 4, "the actual array retains every source slot");
    assert_eq!(
        roots.get(1),
        Some(&Value::Null),
        "the original null array slot is retained"
    );
    let loose = child(roots, 0)?;
    name_scope(loose, "nearest")?;
    reward(forge, loose, 1, 0.5)?;
    let bound = child(roots, 2)?;
    id_scope(bound, [0x55; 16], "bound")?;
    nested(forge, bound)?;
    reward(forge, bound, 2, 0.25)?;
    let later = child(roots, 3)?;
    name_scope(later, "")?;
    reward(forge, later, 1, 0.75)?;
    Ok(())
}

fn root<'source>(forge: &'source DataForge<'source>, node: &StructValue<'source>) -> TestResult {
    support::fields(node, &["debugName", "children", "arrayOnly"]);
    assert_eq!(
        node.field("debugName"),
        Some(&Value::String("root-fallback")),
        "original outer name"
    );
    support::declaration(node, "children", DataType::StrongPointer, true, Some(0))?;
    support::declaration(node, "arrayOnly", DataType::Reference, true, Some(4))?;
    let references = support::array(node, "arrayOnly")?;
    assert_eq!(
        references.len(),
        2,
        "the original Reference array is complete"
    );
    support::target(forge, references.first().ok_or("first array Reference")?, 1)?;
    support::target(forge, references.get(1).ok_or("second array Reference")?, 2)?;
    Ok(())
}

fn nested<'source>(forge: &'source DataForge<'source>, bound: &StructValue<'source>) -> TestResult {
    let children = support::array(bound, "children")?;
    assert_eq!(
        children.len(),
        2,
        "both original nested scopes remain ordered"
    );
    let named = child(children, 0)?;
    name_scope(named, "misleading")?;
    reward(forge, named, 2, 0.0)?;
    let nameless = child(children, 1)?;
    id_scope(nameless, [0x66; 16], "")?;
    assert_eq!(
        nameless.field("reward"),
        Some(&Value::Null),
        "no direct reward is invented"
    );
    let descendants = support::array(nameless, "children")?;
    assert_eq!(descendants.len(), 1, "the actual final scope slot");
    let descendant = child(descendants, 0)?;
    name_scope(descendant, "ignored")?;
    reward(forge, descendant, 1, 1.5)?;
    Ok(())
}

fn child<'tree, 'source>(
    values: &'tree [Value<'source>],
    at: usize,
) -> TestResult<&'tree StructValue<'source>> {
    support::structure(values.get(at).ok_or("original scope array element")?)
}

fn name_scope(node: &StructValue<'_>, name: &str) -> TestResult {
    assert_eq!(
        node.type_name(),
        NAME,
        "the actual scope has no id declaration"
    );
    support::fields(node, &["debugName", "children", "reward"]);
    assert_eq!(
        node.field("id"),
        None,
        "absent id is not an invented zero GUID"
    );
    assert!(
        support::array(node, "children")?.is_empty(),
        "the original name-only leaf has no hidden children"
    );
    scope_fields(node, name)
}

fn id_scope(node: &StructValue<'_>, id: [u8; 16], name: &str) -> TestResult {
    assert_eq!(node.type_name(), ID, "the actual scope declares its own id");
    support::fields(node, &["debugName", "children", "reward", "id"]);
    support::declaration(node, "id", DataType::Guid, false, None)?;
    assert_eq!(
        node.field("id"),
        Some(&Value::Guid(Guid::from_stored(id))),
        "the exact original identity token"
    );
    scope_fields(node, name)
}

fn scope_fields(node: &StructValue<'_>, name: &str) -> TestResult {
    assert_eq!(
        node.field("debugName"),
        Some(&Value::String(name)),
        "the exact source name, including empty"
    );
    support::declaration(node, "debugName", DataType::String, false, None)?;
    support::declaration(node, "children", DataType::StrongPointer, true, Some(0))?;
    support::declaration(node, "reward", DataType::StrongPointer, false, Some(3))
}

fn reward<'source>(
    forge: &'source DataForge<'source>,
    node: &StructValue<'source>,
    pool: u32,
    chance: f32,
) -> TestResult {
    support::leaf(
        forge,
        support::structure(support::value(node, "reward")?)?,
        [Some(pool), None, None],
        chance,
    )
}
