// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Compression, build_catalog_from, write_catalog_from};
use p4kx_extraction::{Language, LocalizationSet, SourceEncryption, StructValue, Value};
use p4kx_testkit::{
    DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::Value as Json;

use super::records;
use crate::catalog_contracts;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

const GENERATOR: u128 = 0x7101_0000_0000_4000_8000_0000_0000_0001;
const CONTRACT: u128 = 0x7101_0000_0000_4000_8000_0000_0000_0002;
const TITLE_KEY: &str = "@title_boundary";
const GERMAN: &str = "Kontrolltitel";

struct Binding {
    token: &'static str,
    key: &'static str,
    text: String,
}

struct Original {
    text: String,
    bindings: Vec<Binding>,
}

impl Original {
    fn one(text: String, replacement: String) -> Self {
        Self {
            text,
            bindings: vec![Binding {
                token: "Piece",
                key: "@boundary_piece",
                text: replacement,
            }],
        }
    }

    fn new(text: &str, bindings: &[(&'static str, &'static str, &str)]) -> Self {
        Self {
            text: text.to_owned(),
            bindings: bindings
                .iter()
                .map(|&(token, key, text)| Binding {
                    token,
                    key,
                    text: text.to_owned(),
                })
                .collect(),
        }
    }

    fn open(&self, input: Input) -> TestResult<Fixture> {
        let properties = self
            .bindings
            .iter()
            .map(|binding| records::property(binding.token, records::string_hash(binding.key)))
            .collect();
        let generator = title_record(properties);
        let database = super::structures::structures(
            DataForgeBuilder::new()
                .version(8)
                .enumeration("ECurrency", ["UEC"]),
        );
        let bytes = database.record(generator).build();
        Fixture::open(&bytes, input, self.locales())
    }

    fn locales(&self) -> Vec<TestEntry> {
        let english = self.bindings.iter().fold(
            GlobalIniBuilder::new().text("title_boundary", self.text.as_str()),
            |ini, binding| ini.text(binding.key.trim_start_matches('@'), binding.text.as_str()),
        );
        let german = GlobalIniBuilder::new().text("title_boundary", GERMAN);
        vec![
            TestEntry::stored("Data\\Localization\\english\\global.ini", english.build())
                .encrypted(),
            TestEntry::stored(
                "Data\\Localization\\german_(germany)\\global.ini",
                german.build(),
            ),
        ]
    }

    fn check(&self, fixture: &Fixture) -> TestResult {
        self.generator(fixture)?;
        self.translations(fixture)
    }

    fn generator(&self, fixture: &Fixture) -> TestResult {
        let forge = fixture.entry.open()?;
        let identity = health_origin_fixture::guid(GENERATOR);
        let record = forge
            .records()
            .find(|record| record.guid().to_string() == identity)
            .ok_or("actual title-control Generator Record")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the actual Generator and its original decoded tree"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "all controlled title fields are actually readable"
        );
        let handler = only_child(tree.root(), "generators")?;
        let contract = only_child(handler, "contracts")?;
        assert!(
            matches!(contract.field("id"), Some(Value::Guid(id))
            if id.to_string() == health_origin_fixture::guid(CONTRACT)),
            "the actual contract identity"
        );
        let parameters = structure(
            contract
                .field("paramOverrides")
                .ok_or("actual authored parameters")?,
        )?;
        let title = only_child(parameters, "stringParamOverrides")?;
        assert_eq!(
            title.field("param"),
            Some(&Value::String("Title")),
            "actual source title parameter"
        );
        assert_eq!(
            title.field("value"),
            Some(&Value::Locale(TITLE_KEY)),
            "exact authored source key"
        );
        self.properties(parameters)
    }

    fn properties(&self, parameters: &StructValue<'_>) -> TestResult {
        let properties = array(parameters, "propertyOverrides")?;
        assert_eq!(
            properties.len(),
            self.bindings.len(),
            "the exact actual original binding count"
        );
        for (property, binding) in properties.iter().zip(&self.bindings) {
            let property = structure(property)?;
            assert_eq!(
                property.field("extendedTextToken"),
                Some(&Value::String(binding.token)),
                "the exact actual authored token spelling"
            );
            let string_hash = structure(property.field("value").ok_or("actual authored value")?)?;
            assert_eq!(
                string_hash.type_name(),
                "MissionPropertyValue_StringHash",
                "actual source resolver family"
            );
            let option = only_child(string_hash, "options")?;
            assert_eq!(
                option.field("textId"),
                Some(&Value::Locale(binding.key)),
                "the real selected Locale of this token, without computed display fallback"
            );
        }
        Ok(())
    }

    fn translations(&self, fixture: &Fixture) -> TestResult {
        let sources = LocalizationSet::read_all(&fixture.archive)?;
        let indexed = sources.index()?;
        for (language, encryption) in [
            (Language::English, SourceEncryption::Encrypted),
            (Language::German, SourceEncryption::Unencrypted),
        ] {
            let entry = sources
                .entry(language)
                .ok_or("actual original language entry")?;
            assert_eq!(
                entry
                    .origin()
                    .ok_or("actual archive entry origin")?
                    .encryption(),
                encryption,
                "the two real locale inputs have distinct origin evidence"
            );
        }
        assert_eq!(
            indexed.text(TITLE_KEY, Language::English),
            Some(self.text.as_str()),
            "the unexpanded authored template is actually readable from the Archive"
        );
        assert_eq!(
            indexed.text(TITLE_KEY, Language::German),
            Some(GERMAN),
            "the independently sourced other language is unchanged"
        );
        for binding in &self.bindings {
            assert_eq!(
                indexed.text(binding.key, Language::English),
                Some(binding.text.as_str()),
                "the exact original replacement text from its actual locale key"
            );
        }
        Ok(())
    }
}

fn title_record(properties: Vec<TestValue>) -> TestRecord {
    let title = TestInstance::new("ContractStringParam")
        .value("param", TestValue::text("Title"))
        .value("value", TestValue::text(TITLE_KEY));
    let parameters = TestInstance::new("ContractParamOverrides")
        .value(
            "stringParamOverrides",
            TestValue::Array(vec![TestValue::Struct(title)]),
        )
        .value("propertyOverrides", TestValue::Array(properties));
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("debugName", TestValue::text("TitleBoundary"))
        .value("paramOverrides", TestValue::Struct(parameters));
    let handler = TestInstance::new("ContractGeneratorHandler_List")
        .value("debugName", TestValue::text("TitleBoundary"))
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        );
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.TitleBoundary",
        "libs/foundry/records/contracts/contractgenerator/title_boundary.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn structure<'tree, 'source>(
    value: &'tree Value<'source>,
) -> TestResult<&'tree StructValue<'source>> {
    let Value::Struct(value) = value else {
        return Err("actual source Struct".into());
    };
    Ok(value)
}

fn array<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    field: &str,
) -> TestResult<&'tree [Value<'source>]> {
    let Some(Value::Array(values)) = parent.field(field) else {
        return Err("actual source array".into());
    };
    Ok(values)
}

fn only_child<'tree, 'source>(
    parent: &'tree StructValue<'source>,
    field: &str,
) -> TestResult<&'tree StructValue<'source>> {
    let [value] = array(parent, field)? else {
        return Err("one original selected child".into());
    };
    structure(value)
}

#[test]
fn a_fully_readable_replacement_chain_has_no_fixed_depth() -> TestResult {
    let source = Original::new(
        "Depth: ~mission(A)",
        &[
            ("A", "@boundary_a", "one ~mission(B)"),
            ("B", "@boundary_b", "two ~mission(C)"),
            ("C", "@boundary_c", "three ~mission(D)"),
            ("D", "@boundary_d", "four ~mission(E)"),
            ("E", "@boundary_e", "five"),
        ],
    );
    complete(&source, "Depth: one two three four five")
}

#[test]
fn repeated_original_occurrences_have_no_fixed_count() -> TestResult {
    let occurrences = 66;
    let source = Original::one("~mission(Piece)".repeat(occurrences), "ok".to_owned());
    complete(&source, &"ok".repeat(occurrences))
}

#[test]
fn the_complete_utf8_replacement_can_exceed_four_kibibytes() -> TestResult {
    let full = "\u{03bb}".repeat(2_049);
    assert_eq!(
        full.len(),
        4_098,
        "the actual source replacement is beyond the old byte boundary"
    );
    let source = Original::one("~mission(Piece)".to_owned(), full);
    let [piece] = source.bindings.as_slice() else {
        return Err("one actual test replacement".into());
    };
    complete(&source, &piece.text)
}

#[test]
fn exact_boundaries_and_shared_acyclic_replacements_are_complete() -> TestResult {
    let source = Original::new(
        "<~mission(A)|~mission(C)>",
        &[
            ("A", "@boundary_a", "~mission(B)"),
            ("B", "@boundary_b", "shared"),
            ("C", "@boundary_c", "~mission(B)"),
        ],
    );
    complete(&source, "<shared|shared>")?;
    let source = Original::one("~mission(Piece)".repeat(64), "ok".to_owned());
    complete(&source, &"ok".repeat(64))?;
    let source = Original::one("~mission(Piece)".to_owned(), "\u{03bb}".repeat(2_048));
    complete(&source, &"\u{03bb}".repeat(2_048))
}

#[test]
fn original_two_key_cycle_is_distinct_from_missing_or_dynamic_text() -> TestResult {
    let source = Original::new(
        "Cycle: ~mission(A)",
        &[
            ("A", "@boundary_a", "left ~mission(B)"),
            ("B", "@boundary_b", "right ~mission(a)"),
        ],
    );
    complete(&source, "Cycle: left right [A]")
}

fn complete(source: &Original, expected: &str) -> TestResult {
    let mut inputs = Vec::new();
    for input in [Input::Plain, Input::Standalone] {
        let fixture = source.open(input)?;
        source.check(&fixture)?;
        inputs.push(fixture);
    }
    for fixture in inputs {
        let options = health_origin_fixture::options();
        let built = build_catalog_from(&fixture.archive, &fixture.entry, &options)?;
        let document: Json = serde_json::from_slice(built.json())?;
        let row =
            catalog_contracts::entry(&document, &health_origin_fixture::guid(CONTRACT), None)?;
        assert_eq!(
            catalog_contracts::text(&document, row, "title")?,
            catalog_contracts::v3::Text {
                key: Some(TITLE_KEY),
                en: Some(expected),
                de: Some(GERMAN)
            },
            "all genuine selected replacements are complete, independently of size, count and depth"
        );
        writers(&fixture, built.json())?;
    }
    Ok(())
}

fn writers(fixture: &Fixture, expected: &[u8]) -> TestResult {
    for compression in [Compression::None, Compression::ZSTD] {
        let (_, bytes) = write_catalog_from(
            &fixture.archive,
            &fixture.entry,
            &health_origin_fixture::options(),
            compression,
            || Ok(Vec::new()),
        )?;
        let plain = match compression {
            Compression::None => bytes,
            Compression::Zstd { .. } => zstd::stream::decode_all(bytes.as_slice())?,
        };
        assert_eq!(
            plain, expected,
            "actual uncompressed and ZSTD writers agree with the complete document"
        );
        let _: Json = serde_json::from_slice(&plain)?;
    }
    Ok(())
}
