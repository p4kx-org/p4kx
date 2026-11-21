// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Completion offers the struct names after `from` and in `[…]` and the field names at the start
//! of a path and after `.`, from the schema of one DataForge.

use std::error::Error;

use crate::ships::{database, rows};
use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{CompletionKind, MAX_COMPLETIONS, QuerySchema};
use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

type TestResult = Result<(), Box<dyn Error>>;

fn schema() -> Result<QuerySchema, Box<dyn Error>> {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    Ok(QuerySchema::of(&forge)?)
}

/// Returns the names offered at the end of `text`.
fn names(schema: &QuerySchema, text: &str) -> Vec<String> {
    schema
        .complete(text, text.len())
        .map_or_default(|completion| {
            completion
                .candidates()
                .iter()
                .map(|candidate| candidate.name().to_owned())
                .collect()
        })
}

#[test]
fn from_offers_every_struct_name_in_name_order() -> TestResult {
    let schema = schema()?;

    let completion = schema.complete("from ", 5).ok_or("no completion")?;

    assert_eq!(completion.kind(), CompletionKind::Type, "kind");
    assert_eq!(completion.replace(), 5..5, "inserted at the cursor");
    assert_eq!(
        names(&schema, "from "),
        [
            "Component",
            "Shield",
            "Ship",
            "Station",
            "Tiny",
            "Vector",
            "Weapon"
        ],
        "names"
    );
    Ok(())
}

#[test]
fn the_typed_word_filters_ignoring_case_and_is_replaced() -> TestResult {
    let schema = schema()?;

    let completion = schema.complete("from sh", 7).ok_or("no completion")?;

    assert_eq!(names(&schema, "from sh"), ["Shield", "Ship"], "names");
    assert_eq!(completion.replace(), 5..7, "the typed word");
    assert!(schema.complete("from Shx", 8).is_none(), "nothing fits");
    Ok(())
}

#[test]
fn a_path_starts_with_a_field_of_the_from_type() -> TestResult {
    let schema = schema()?;
    let fields = [
        "anchor",
        "Components",
        "hardpoints",
        "links",
        "mass",
        "position",
        "tags",
    ];

    for text in [
        "from Ship select ",
        "from Ship select mass, ",
        "from Ship select mass where ",
        "from Ship select mass where mass > 1 and not (",
    ] {
        assert_eq!(names(&schema, text), fields, "{text}");
    }
    let completion = schema
        .complete("from Ship select ma", 19)
        .ok_or("no completion")?;
    let detail = completion.candidates().first().and_then(|c| c.detail());
    assert_eq!(detail, Some("Single"), "the field type");
    Ok(())
}

#[test]
fn a_dot_offers_the_fields_of_the_struct_or_pointer_reached() -> TestResult {
    let schema = schema()?;

    assert_eq!(
        names(&schema, "from Ship select position."),
        ["x", "y"],
        "class"
    );
    assert_eq!(
        names(&schema, "from Ship select anchor."),
        ["x", "y"],
        "pointer"
    );
    assert_eq!(
        names(&schema, "from Ship select hardpoints[1]."),
        ["x", "y"],
        "index"
    );
    assert_eq!(
        names(&schema, "from Ship select Components[*]."),
        ["modes", "name"],
        "every element"
    );
    assert_eq!(
        names(&schema, "from Ship select Components[Weapon].da"),
        ["damage"],
        "component selector"
    );
    Ok(())
}

#[test]
fn brackets_offer_the_element_struct_and_its_subtypes() -> TestResult {
    let schema = schema()?;

    assert_eq!(
        names(&schema, "from Ship select Components["),
        ["Component", "Shield", "Weapon"],
        "polymorphic array"
    );
    assert!(
        names(&schema, "from Ship select tags[").is_empty(),
        "scalar array"
    );
    Ok(())
}

#[test]
fn without_from_the_fields_of_every_struct_are_offered_once() -> TestResult {
    let schema = schema()?;

    assert_eq!(names(&schema, "at \"*\" select pos"), ["position"], "once");
    assert_eq!(
        names(&schema, "at \"*\" select position."),
        ["x", "y"],
        "the struct that has fields"
    );
    Ok(())
}

#[test]
fn nothing_is_offered_where_no_name_fits() -> TestResult {
    let schema = schema()?;

    for text in [
        "",
        "fr",
        "from Ship sel",
        "from Ship at \"libs",
        "from Ship select mass.",
        "from Ship select mass = ",
        "from Ship select tags.",
        "from Ship select nope.",
        "from Nope select ",
        "from Ship select position.x.",
    ] {
        assert!(schema.complete(text, text.len()).is_none(), "{text}");
    }
    assert!(
        schema.complete("from Ship", 99).is_none(),
        "beyond the text"
    );
    assert!(schema.complete("from Ä", 6).is_none(), "inside a character");
    Ok(())
}

#[test]
fn the_cursor_decides_not_the_text_behind_it() -> TestResult {
    let schema = schema()?;
    let text = "from Ship select position. where mass > 1";

    assert_eq!(
        schema
            .complete(text, 26)
            .map(|completion| completion.candidates().len()),
        Some(2),
        "x and y"
    );
    Ok(())
}

#[test]
fn an_accepted_candidate_makes_a_query_that_runs() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;
    let typed = "from Ship select Components[Wea";
    let completion = schema.complete(typed, typed.len()).ok_or("no completion")?;
    let candidate = completion.candidates().first().ok_or("no candidate")?;

    let mut text = typed.to_owned();
    text.replace_range(completion.replace(), candidate.insert());
    text.push_str("].damage");

    assert_eq!(
        rows(&forge, &text)?,
        ["alpha: [250.5, 1200]", "beta: []"],
        "{text}"
    );
    Ok(())
}

#[test]
fn names_that_are_no_bare_names_are_inserted_in_backticks() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("select", TestType::Int32)
                .field("referencing", TestType::Int32)
                .field("Size-X", TestType::Single),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;

    let completion = schema
        .complete("from Tag select ", 16)
        .ok_or("no completion")?;
    let inserts: Vec<&str> = completion
        .candidates()
        .iter()
        .map(p4kx_dataforge::query::Candidate::insert)
        .collect();

    assert_eq!(
        inserts,
        ["`referencing`", "`select`", "`Size-X`"],
        "quoted, also the newest keyword"
    );
    Ok(())
}

#[test]
fn a_long_list_is_cut_and_counts_the_rest() -> TestResult {
    let builder = (0..MAX_COMPLETIONS + 7).fold(DataForgeBuilder::new(), |builder, n| {
        builder.structure(TestStruct::new(format!("T{n:03}")).field("v", TestType::Int32))
    });
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;

    let completion = schema.complete("from T", 6).ok_or("no completion")?;

    assert_eq!(completion.candidates().len(), MAX_COMPLETIONS, "listed");
    assert_eq!(completion.more(), 7, "the rest");
    Ok(())
}

#[test]
fn brackets_also_offer_the_bases_and_a_path_needs_a_related_type() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("id", TestType::Int32))
        .structure(
            TestStruct::new("Mid")
                .parent("Base")
                .field("mid", TestType::Int32),
        )
        .structure(
            TestStruct::new("Leaf")
                .parent("Mid")
                .field("leaf", TestType::Int32),
        )
        .structure(TestStruct::new("Other").field("other", TestType::Int32))
        .structure(
            TestStruct::new("Holder").array("parts", TestType::StrongPointer("Mid".to_owned())),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let schema = QuerySchema::of(&forge)?;

    assert_eq!(
        names(&schema, "from Holder select parts["),
        ["Base", "Leaf", "Mid"],
        "bases, the element and its subtypes"
    );
    assert_eq!(
        names(&schema, "from Holder select parts[Leaf]."),
        ["id", "leaf", "mid"],
        "a subtype"
    );
    assert!(
        schema
            .complete("from Holder select parts[Other].", 32)
            .is_none(),
        "an unrelated type"
    );
    Ok(())
}

#[test]
fn the_whole_word_around_the_cursor_is_replaced() -> TestResult {
    let schema = schema()?;
    let text = "from Ship select mxyz, tags";

    let completion = schema.complete(text, 18).ok_or("no completion")?;

    assert_eq!(completion.replace(), 17..21, "the word around the cursor");
    assert_eq!(
        names(&schema, "from Ship select m"),
        ["mass"],
        "filtered by the part before"
    );
    assert_eq!(
        completion
            .candidates()
            .first()
            .map(p4kx_dataforge::query::Candidate::name),
        Some("mass"),
        "candidate"
    );
    Ok(())
}
