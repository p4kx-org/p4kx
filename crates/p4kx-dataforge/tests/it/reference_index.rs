// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The index of incoming and outgoing References over a synthetic DataForge in both layouts.

use p4kx_dataforge::query::{Query, QueryErrorKind};
use p4kx_dataforge::{DataForge, Guid, ReferenceIndex};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const TERRA: u128 = 10;
const AEGIS: u128 = 20;
const ALPHA: u128 = 31;
const BETA: u128 = 32;
const LOOSE: u128 = 40;
const MISSING: u128 = 99;

fn reference_to(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

fn part(target: u128) -> TestValue {
    TestValue::Struct(TestInstance::new("Part").value("target", TestValue::Reference(target)))
}

fn builder() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Planet").field("name", TestType::String))
        .structure(
            TestStruct::new("Manufacturer")
                .field("name", TestType::String)
                .field("home", reference_to("Planet")),
        )
        .structure(TestStruct::new("Part").field("target", reference_to("Manufacturer")))
        .structure(
            TestStruct::new("Ship")
                .field("maker", reference_to("Manufacturer"))
                .array("escorts", reference_to("Ship"))
                .array("parts", TestType::Class("Part".to_owned()))
                .field("me", TestType::Reference)
                .field("twin", TestType::Reference),
        )
        .record(TestRecord::new(
            TERRA,
            "Planet",
            "Planet.Terra",
            "libs/planets/terra.xml",
        ))
        .record(
            TestRecord::new(
                AEGIS,
                "Manufacturer",
                "Manufacturer.Aegis",
                "libs/makers/aegis.xml",
            )
            .value("home", TestValue::Reference(TERRA)),
        )
        .record(
            TestRecord::new(ALPHA, "Ship", "Ship.Alpha", "libs/ships/ships.xml")
                .value("maker", TestValue::Reference(AEGIS))
                .value(
                    "escorts",
                    TestValue::Array(vec![
                        TestValue::Null,
                        TestValue::Reference(BETA),
                        TestValue::Reference(MISSING),
                    ]),
                )
                .value("parts", TestValue::Array(vec![part(AEGIS), part(TERRA)]))
                .value("me", TestValue::Reference(ALPHA)),
        )
        .record(
            TestRecord::new(BETA, "Ship", "Ship.Beta", "Libs\\Ships\\SHIPS.xml")
                .value("maker", TestValue::Reference(AEGIS))
                .value("twin", TestValue::Reference(ALPHA)),
        )
        .record(TestRecord::new(
            LOOSE,
            "Ship",
            "Ship.Loose",
            "libs/ships/loose.xml",
        ))
}

fn in_both_versions(check: impl Fn(&DataForge<'_>) -> TestResult) -> TestResult {
    for version in [6, 8] {
        let bytes = builder().version(version).build();
        let forge = DataForge::open(&bytes)?;
        check(&forge).map_err(|error| format!("version {version}: {error}"))?;
    }
    Ok(())
}

fn index_of(forge: &DataForge<'_>, guid: u128) -> Result<u32, String> {
    let guid: Guid = guid_text(guid)
        .parse()
        .map_err(|error| format!("{error}"))?;
    forge
        .record_by_guid(guid)
        .map(|record| record.index())
        .ok_or_else(|| format!("no Record {guid}"))
}

fn guid_text(value: u128) -> String {
    let hex = format!("{value:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn places<'a>(
    forge: &DataForge<'_>,
    links: impl Iterator<Item = p4kx_dataforge::IndexedReference<'a>>,
) -> Vec<(String, String)> {
    links
        .map(|link| {
            let name = forge
                .record(link.source())
                .and_then(|record| record.name().ok())
                .unwrap_or_default()
                .to_owned();
            (name, link.location())
        })
        .collect()
}

#[test]
fn incoming_references_keep_the_record_and_the_field_that_hold_them() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        let aegis = index_of(forge, AEGIS)?;
        let found = places(forge, index.incoming(aegis));
        let expected = [
            ("Ship.Alpha", "maker"),
            ("Ship.Alpha", "parts[0].target"),
            ("Ship.Beta", "maker"),
        ]
        .map(|(name, at)| (name.to_owned(), at.to_owned()));
        assert_eq!(found, expected, "the places of the References to Aegis");
        let first = index.incoming(aegis).next().ok_or("no Reference")?;
        assert_eq!(first.field_path(), "maker", "field path");
        assert_eq!(first.indices(), &[] as &[u32], "no array on the way");
        Ok(())
    })
}

#[test]
fn array_elements_are_kept_as_indices() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        let beta = index_of(forge, BETA)?;
        let link = index.incoming(beta).next().ok_or("no Reference")?;
        assert_eq!(link.field_path(), "escorts[]", "shape");
        assert_eq!(link.indices(), &[1], "index in the array");
        assert_eq!(link.location(), "escorts[1]", "location");
        let terra = index_of(forge, TERRA)?;
        let nested = places(forge, index.incoming(terra));
        assert_eq!(
            nested,
            [
                ("Manufacturer.Aegis".to_owned(), "home".to_owned()),
                ("Ship.Alpha".to_owned(), "parts[1].target".to_owned()),
            ],
            "a Reference inside an array element of structs"
        );
        Ok(())
    })
}

#[test]
fn outgoing_references_follow_the_order_of_the_fields() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        let alpha = index_of(forge, ALPHA)?;
        let found = places(forge, index.outgoing(alpha));
        let expected = [
            "maker",
            "escorts[1]",
            "escorts[2]",
            "parts[0].target",
            "parts[1].target",
        ]
        .map(|at| ("Ship.Alpha".to_owned(), at.to_owned()));
        assert_eq!(found, expected, "the References Alpha holds");
        assert_eq!(
            index.outgoing(index_of(forge, TERRA)?).len(),
            0,
            "a leaf Record"
        );
        Ok(())
    })
}

#[test]
fn a_reference_to_a_missing_guid_is_unresolved_and_counted() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        assert_eq!(index.unresolved_count(), 1, "one unresolved Reference");
        let link = index.unresolved().next().ok_or("none unresolved")?;
        assert_eq!(link.target(), None, "no target");
        assert_eq!(link.location(), "escorts[2]", "where it stands");
        assert_eq!(
            link.unresolved_guid().map(|guid| guid.to_string()),
            Some(guid_text(MISSING)),
            "its GUID"
        );
        assert_eq!(
            link.source(),
            index_of(forge, ALPHA)?,
            "the Record that holds it"
        );
        assert_eq!(
            index.len(),
            index.resolved_count() + index.unresolved_count(),
            "every Reference is resolved or not"
        );
        assert!(
            !index.is_truncated() && index.unreadable_records() == 0,
            "complete"
        );
        Ok(())
    })
}

#[test]
fn a_reference_to_the_record_itself_is_no_reference() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        let alpha = index_of(forge, ALPHA)?;
        let from_beta = index
            .incoming(alpha)
            .map(|link| link.source())
            .collect::<Vec<_>>();
        assert_eq!(
            from_beta,
            [index_of(forge, BETA)?],
            "only Beta refers to Alpha"
        );
        Ok(())
    })
}

#[test]
fn referrers_are_distinct_and_ascending() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        let aegis = index_of(forge, AEGIS)?;
        let expected = [index_of(forge, ALPHA)?, index_of(forge, BETA)?];
        assert_eq!(
            index.referrers(aegis),
            expected,
            "Alpha holds two References to Aegis"
        );
        assert!(
            index.referrers(index_of(forge, LOOSE)?).is_empty(),
            "nobody refers to Loose"
        );
        assert!(index.referrers(u32::MAX).is_empty(), "no such Record");
        Ok(())
    })
}

#[test]
fn files_refer_to_other_files_ignoring_case_and_separators() -> TestResult {
    in_both_versions(|forge| {
        let index = ReferenceIndex::build(forge);
        let graph = index.file_graph(forge);
        assert_eq!(
            graph.paths(),
            [
                "libs/makers/aegis",
                "libs/planets/terra",
                "libs/ships/loose",
                "libs/ships/ships"
            ],
            "Beta shares the file of Alpha under another spelling"
        );
        let ships = graph
            .position("LIBS\\SHIPS\\ships")
            .ok_or("no ships file")?;
        let referred: Vec<_> = graph.refers_to(ships).collect();
        assert_eq!(
            referred,
            ["libs/makers/aegis", "libs/planets/terra"],
            "files of Alpha and Beta"
        );
        assert_eq!(
            graph.edges().len(),
            3,
            "ships to makers and planets, makers to planets"
        );
        Ok(())
    })
}

#[test]
fn the_index_stops_at_its_limit_and_says_so() -> TestResult {
    in_both_versions(|forge| {
        let all = ReferenceIndex::build(forge);
        let capped = ReferenceIndex::build_with(forge, 3, &|| false).ok_or("an index")?;
        assert!(capped.is_truncated(), "more References than the limit");
        assert_eq!(capped.len(), 3, "the first three in Record order");
        assert!(!all.is_truncated(), "the full index is complete");
        Ok(())
    })
}

#[test]
fn a_limited_index_can_be_cancelled() -> TestResult {
    in_both_versions(|forge| {
        let capped =
            ReferenceIndex::build_with(forge, 3, &|| false).ok_or("an index without a stop")?;
        assert!(
            capped.is_truncated() && capped.len() == 3,
            "the limit holds"
        );
        assert!(
            ReferenceIndex::build_with(forge, 3, &|| true).is_none(),
            "a stop gives no index"
        );
        Ok(())
    })
}

fn selected(forge: &DataForge<'_>, text: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    Ok(run
        .indexes()
        .iter()
        .filter_map(|index| forge.record(*index)?.name().ok().map(str::to_owned))
        .collect())
}

#[test]
fn referencing_selects_the_records_that_refer_to_a_record() -> TestResult {
    in_both_versions(|forge| {
        let by_path = selected(forge, r#"referencing "libs/makers/aegis""#)?;
        assert_eq!(
            by_path,
            ["Ship.Alpha", "Ship.Beta"],
            "the referrers of Aegis by record path"
        );
        let guid = guid_text(AEGIS);
        let by_guid = selected(forge, &format!("referencing \"{guid}\""))?;
        assert_eq!(by_guid, by_path, "by GUID");
        let typed = selected(
            forge,
            r#"from Manufacturer referencing "libs/planets/terra""#,
        )?;
        assert_eq!(typed, ["Manufacturer.Aegis"], "combined with from");
        let located = selected(forge, r#"at "ships" referencing "LIBS\PLANETS\Terra""#)?;
        assert_eq!(
            located,
            ["Ship.Alpha"],
            "combined with at, ignoring case and separators"
        );
        assert!(
            selected(forge, r#"referencing "libs/ships/loose""#)?.is_empty(),
            "nobody refers to Loose"
        );
        Ok(())
    })
}

#[test]
fn referencing_an_unknown_record_is_marked() -> TestResult {
    in_both_versions(|forge| {
        let text = r#"from Ship referencing "libs/nowhere""#;
        let error = Query::parse(text)?.compile(forge).err().ok_or("compiled")?;
        assert_eq!(
            error.kind(),
            &QueryErrorKind::UnknownReferencedRecord {
                text: "libs/nowhere".to_owned()
            },
            "kind"
        );
        assert_eq!(&text[error.span().range()], r#""libs/nowhere""#, "span");
        let canonical = Query::parse(r#"REFERENCING  "a""b" select x"#)?.to_string();
        assert_eq!(
            canonical, r#"referencing "a""b" select x"#,
            "canonical form"
        );
        let out_of_order = Query::parse(r#"at "x" select a referencing "y""#)
            .err()
            .ok_or("parsed")?;
        assert!(
            out_of_order.to_string().contains("has to come before"),
            "{out_of_order}"
        );
        Ok(())
    })
}
