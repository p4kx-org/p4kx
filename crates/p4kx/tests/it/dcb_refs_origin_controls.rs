// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Record, ReferenceIndex, Value};
use p4kx_testkit::{TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value as Json, json};

use super::fixture::{
    Fixture, Input, Site, TestResult, array, field_origin, has_header, header, site, value,
};

const BETA_FILE: &str = "libs/ships/Beta";
const GAMMA_FILE: &str = "libs/ships/beta";
const BROKEN: u128 = 0x44;

struct FileSources {
    source: u32,
    representative: u32,
    actual_target: u32,
    links: [Site; 2],
    omitted: Site,
}

#[test]
fn filtered_file_edge_uses_its_distinct_representative_and_both_original_links() -> TestResult {
    let fixture = Fixture::open(&files_database(), &[Input::Plain], original_files)?;
    let case = fixture.inputs.first().ok_or("actual file graph fixture")?;
    let document = case.run(&["libs/ships/alpha", "--files", "--json"])?;
    assert_eq!(
        document.get("file_count"),
        Some(&json!(4)),
        "all file groups"
    );
    assert_eq!(
        document.get("edge_count"),
        Some(&json!(1)),
        "only one selected pair"
    );
    let [edge] = array(&document, "edges")? else {
        return Err("the filter keeps exactly one distinct edge".into());
    };
    assert_eq!(
        edge.get("from"),
        Some(&json!("libs/ships/alpha")),
        "source display path"
    );
    assert_eq!(
        edge.get("to"),
        Some(&json!(BETA_FILE)),
        "selected file representative"
    );
    let original = &case.original;
    let from = case.uses(&document, edge, "/from")?;
    header(&from, original.source, "fileName", "conversion");
    let to = case.uses(&document, edge, "/to")?;
    check_file_representative_uses(&to, original);
    let count = case.uses(&document, &document, "/edge_count")?;
    for index in [original.representative, original.actual_target] {
        assert!(
            !has_header(&count, index, "recordName", "selector"),
            "the displayed path's name tie-break does not determine the edge count"
        );
    }
    for contributors in [&from, &to, &count] {
        for link in original.links {
            value(contributors, link, "$value", "selector");
        }
        for role in ["payload", "selector", "conversion"] {
            assert!(
                !super::fixture::has_value(contributors, original.omitted, "$value", role),
                "the unselected file pair contributes to no retained edge"
            );
        }
    }
    Ok(())
}

fn check_file_representative_uses(uses: &[&Json], original: &FileSources) {
    header(uses, original.representative, "fileName", "conversion");
    for index in [original.representative, original.actual_target] {
        for field in ["fileName", "recordName"] {
            header(uses, index, field, "selector");
        }
    }
    assert!(
        !has_header(uses, original.actual_target, "fileName", "conversion"),
        "the target Record does not supply the representative's display spelling"
    );
}

fn files_database() -> Vec<u8> {
    use super::super::{AEGIS, ALPHA, BETA, GAMMA, TERRA, schema, ship};

    schema()
        .record(ship(ALPHA, "Alpha", "libs/ships/alpha.xml").value(
            "escorts",
            TestValue::Array(vec![
                TestValue::Reference(GAMMA),
                TestValue::Reference(GAMMA),
            ]),
        ))
        .record(ship(GAMMA, "Gamma", "libs/ships/beta.xml"))
        .record(ship(BETA, "Beta", "libs/ships/Beta.XML"))
        .record(
            TestRecord::new(
                AEGIS,
                "Manufacturer",
                "Manufacturer.Aegis",
                "libs/makers/aegis.xml",
            )
            .value("home", TestValue::Reference(TERRA)),
        )
        .record(TestRecord::new(
            TERRA,
            "Planet",
            "Planet.Terra",
            "libs/planets/terra.xml",
        ))
        .build()
}

fn original_files(forge: &DataForge<'_>) -> TestResult<FileSources> {
    let alpha = forge.record(0).ok_or("physical Alpha")?;
    let gamma = forge.record(1).ok_or("physical Gamma target")?;
    let beta = forge.record(2).ok_or("physical Beta representative")?;
    assert_representative(forge, &beta, &gamma)?;
    let alpha_tree = alpha.tree()?;
    assert!(alpha_tree.is_from(&alpha), "original physical source tree");
    let Some(Value::Array(links)) = alpha_tree.field("escorts") else {
        return Err("actual source array".into());
    };
    let [first, second] = links.as_ref() else {
        return Err("two actual authored source slots".into());
    };
    for link in [first, second] {
        let Value::Reference(target) = link else {
            return Err("the source slot actually resolves".into());
        };
        assert_eq!(
            target.original_target().record()?.index(),
            gamma.index(),
            "both source slots resolve to Gamma rather than the file representative"
        );
    }
    let omitted = forge.record(3).ok_or("physical Aegis")?.tree()?;
    let omitted_ref = omitted
        .field("home")
        .ok_or("unselected original file link")?;
    assert!(
        matches!(omitted_ref, Value::Reference(target) if target.record_index() == 4),
        "the unfiltered graph has an independent supporting source"
    );
    let index = ReferenceIndex::build(forge);
    let graph = index.file_graph(forge);
    assert_eq!(graph.paths().len(), 4, "actual normalized file groups");
    assert_eq!(
        graph.edges().len(),
        2,
        "duplicate links support one of two edges"
    );
    Ok(FileSources {
        source: alpha.index(),
        representative: beta.index(),
        actual_target: gamma.index(),
        links: [site(&alpha_tree, first)?, site(&alpha_tree, second)?],
        omitted: site(&omitted, omitted_ref)?,
    })
}

fn assert_representative(
    forge: &DataForge<'_>,
    beta: &Record<'_>,
    gamma: &Record<'_>,
) -> TestResult {
    assert_eq!(
        (beta.stored_file_name()?, gamma.stored_file_name()?),
        ("libs/ships/Beta.XML", "libs/ships/beta.xml"),
        "both distinct raw path spellings supply the normalized path comparison"
    );
    assert_eq!(
        (beta.record_path()?, gamma.record_path()?),
        (BETA_FILE, GAMMA_FILE),
        "both authored path spellings survive extension removal"
    );
    assert_eq!(
        (beta.name()?, gamma.name()?),
        ("Ship.Beta", "Ship.Gamma"),
        "the actual Record names break the normalized path tie"
    );
    assert!(
        gamma.index() < beta.index(),
        "the name-ordered representative is not the earliest stored Record"
    );
    let member_order: Vec<_> = forge
        .path_order()
        .iter()
        .copied()
        .filter(|index| [beta.index(), gamma.index()].contains(index))
        .collect();
    assert_eq!(
        member_order,
        [beta.index(), gamma.index()],
        "the original FileGraph order uses the two normalized members' names"
    );
    Ok(())
}

#[test]
fn an_unreadable_record_limits_global_counts_but_keeps_local_and_positive_proof() -> TestResult {
    let fixture = Fixture::open(&broken_database(), &Input::ALL, assert_broken_original)?;
    for case in &fixture.inputs {
        let (incoming, warning) =
            case.run_with_warnings(&["libs/ships/beta", "--incoming", "--json"])?;
        assert_warning(&warning);
        let [target, _] = array(&incoming, "records")? else {
            return Err("both same-path incoming rows".into());
        };
        assert_eq!(
            target.get("reference_count"),
            Some(&json!(1)),
            "retained incoming link"
        );
        for field in ["/reference_count", "/record_count"] {
            assert_incomplete(&incoming, target, field, case.input)?;
        }
        let [link] = array(target, "references")? else {
            return Err("one retained incoming link".into());
        };
        case.uses(&incoming, link, "/record/guid")?;

        let (outgoing, warning) =
            case.run_with_warnings(&["libs/ships/alpha", "--outgoing", "--json"])?;
        assert_warning(&warning);
        let [holder] = array(&outgoing, "records")? else {
            return Err("one clean outgoing source".into());
        };
        for (field, expected) in [
            ("reference_count", 2),
            ("record_count", 2),
            ("unresolved_count", 1),
        ] {
            assert_eq!(holder.get(field), Some(&json!(expected)), "local {field}");
            case.uses(&outgoing, holder, &format!("/{field}"))?;
        }

        let (unresolved, warning) = case.run_with_warnings(&["--unresolved", "--json"])?;
        assert_warning(&warning);
        assert_eq!(
            unresolved.get("count"),
            Some(&json!(2)),
            "known missing occurrences"
        );
        assert_incomplete(&unresolved, &unresolved, "/count", case.input)?;
    }
    Ok(())
}

fn broken_database() -> Vec<u8> {
    super::super::original_database()
        .structure(
            TestStruct::new("BrokenReferenceHolder")
                .field("maker", TestType::Reference)
                .declared_size(21),
        )
        .record(
            TestRecord::new(
                BROKEN,
                "BrokenReferenceHolder",
                "BrokenReferenceHolder.Control",
                "libs/ships/broken.xml",
            )
            .value("maker", TestValue::Reference(super::super::AEGIS)),
        )
        .build()
}

fn assert_broken_original(forge: &DataForge<'_>) -> TestResult {
    let record = forge.record(5).ok_or("actual unreadable Record")?;
    assert_eq!(
        record.type_name()?,
        "BrokenReferenceHolder",
        "the physical Record header is readable"
    );
    assert!(
        record.tree().is_err(),
        "the actual original Record body is unreadable"
    );
    let index = ReferenceIndex::build(forge);
    assert_eq!(index.unreadable_records(), 1, "one actual scan failure");
    assert_eq!(
        index.unresolved().len(),
        2,
        "the two readable missing links remain"
    );
    Ok(())
}

fn assert_warning(warning: &str) {
    assert!(
        warning.contains("1 Records cannot be read"),
        "the existing command reports its one whole-Record read failure: {warning}"
    );
}

fn assert_incomplete(document: &Json, row: &Json, field: &str, input: Input) -> TestResult {
    let origin = field_origin(row, field)?;
    assert_eq!(
        origin.get("sourceSetComplete"),
        Some(&json!(false)),
        "a failed physical scan prevents a complete {field}"
    );
    let marker = matches!(input, Input::Encrypted).then_some(true);
    assert_eq!(
        origin.get("containsEncryptedSource"),
        Some(&json!(marker)),
        "known positive contributions preserve their proven origin"
    );
    let inventory = document
        .get("provenance")
        .ok_or("actual source inventory")?;
    let refs = array(origin, "sourceUsesRefs")?;
    let mut attempts = array(inventory, "uses")?
        .iter()
        .filter(|used| used.get("role") == Some(&json!("attempted")))
        .peekable();
    assert!(
        attempts.peek().is_some(),
        "the unreadable original Record remains an attempted read"
    );
    assert!(
        attempts.all(|used| used.get("id").is_some_and(|id| !refs.contains(id))),
        "attempted reads do not contribute to the retained count"
    );
    Ok(())
}
