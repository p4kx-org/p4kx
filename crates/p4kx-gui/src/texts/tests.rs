// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use fluent_bundle::FluentValue;
use fluent_syntax::ast;
use p4kx_export::CraftingWarning;

use super::{BRITISH_ENGLISH, GERMAN, Text, Texts};
use crate::Language;

fn message_ids(source: &str) -> BTreeSet<String> {
    let resource = fluent_syntax::parser::parse(source).unwrap_or_else(|(_, errors)| {
        panic!("the Fluent file has syntax errors: {errors:?}");
    });
    resource
        .body
        .iter()
        .filter_map(|entry| match entry {
            ast::Entry::Message(message) => Some(message.id.name.to_owned()),
            _ => None,
        })
        .collect()
}

#[expect(
    clippy::too_many_lines,
    reason = "one line per placeholder name of the interface texts"
)]
fn sample_args() -> Vec<(&'static str, FluentValue<'static>)> {
    vec![
        ("name", "Data".into()),
        ("divisor", 2.into()),
        ("material", "hull".into()),
        ("role", "diffuse".into()),
        ("method", "1".into()),
        ("path", "Data.p4k".into()),
        ("target", "Data/Objects/Test/paint.mtl".into()),
        ("parent", "ordinary-copy-source".into()),
        ("reason", "no Archive".into()),
        ("settings", "bad, =2".into()),
        ("count", 3.into()),
        ("number", "3".into()),
        ("tracks", 3.into()),
        ("hidden", "1".into()),
        ("build", "4.3.1".into()),
        ("size", "300.0 MiB".into()),
        ("limit", "256.0 MiB".into()),
        ("compressed", "2.0 MiB".into()),
        ("shown", "1,048,576".into()),
        ("total", "2,000,000".into()),
        ("done", "1".into()),
        ("percent", "50".into()),
        ("extracted", "2".into()),
        ("converted", "1".into()),
        ("skipped", "1".into()),
        ("player", "5".into()),
        ("ai", "3".into()),
        ("mission", "2".into()),
        ("special", "1".into()),
        ("unclassified", "4".into()),
        ("failed", "0".into()),
        ("cancelled", "0".into()),
        ("left", "2".into()),
        ("written", "3".into()),
        ("errors", "1".into()),
        ("erroneous", "1".into()),
        ("version", "0.2.0".into()),
        ("channel", "LIVE".into()),
        ("change", "Added".into()),
        ("filter", "*.xml".into()),
        ("stopped", "yes".into()),
        ("current", "1".into()),
        ("first", "10".into()),
        ("last", "16".into()),
        ("line", "12".into()),
        ("text", "<a/>".into()),
        ("key", "@item_Name".into()),
        ("lines", "4,194,304".into()),
        ("old", "libs/ships/old".into()),
        ("new", "libs/ships/new".into()),
        ("type", "Hull".into()),
        ("at", "nodes[0]".into()),
        ("field", "hull.armor".into()),
        ("level", "2".into()),
        ("width", "512".into()),
        ("height", "256".into()),
        ("format", "BC7_UNORM".into()),
        ("rate", "48,000".into()),
        ("position", "0:03".into()),
        ("duration", "1:12".into()),
        ("codec", "AAC".into()),
        ("id", "1234".into()),
        ("bank", "Data\\Sounds\\ship.bnk".into()),
        ("column", "13".into()),
        ("shortcut", "Ctrl+Enter".into()),
        ("page", "1".into()),
        ("pages", "3".into()),
        ("max", "120".into()),
        ("guid", "cbee34fd-6441-4000-b99f-924f7f26e19f".into()),
        ("rule", "the name is empty".into()),
        ("char", "'?'".into()),
        ("bytes", 300.into()),
        ("kind", "manufacturers".into()),
        ("label", "Structs".into()),
        ("added", "1".into()),
        ("removed", "2".into()),
        ("changed", "3".into()),
        ("complete", "yes".into()),
        ("language", "de".into()),
        ("resolved", "4".into()),
        ("missing", "1".into()),
        ("ports", "12".into()),
        ("unresolved", "2".into()),
        ("docked", "1".into()),
        ("outside", "3".into()),
        ("placeholders", "2".into()),
        ("value", "1.5".into()),
        ("depth", "4.0".into()),
        ("vertices", "24".into()),
        ("triangles", "12".into()),
        ("textures", "6".into()),
        ("palettes", "2".into()),
        ("files", "20".into()),
        ("max_vertices", "4,000,000".into()),
        ("max_triangles", "8,000,000".into()),
        ("references", "3 References".into()),
        ("records", "2 Records".into()),
        ("texts", "90,946".into()),
        ("entries", "49,149".into()),
        ("binary", "1".into()),
        ("large", "0".into()),
        ("unreadable", "0".into()),
        ("seconds", "1.24".into()),
        ("place", "libs/ships/alpha name".into()),
        ("digits", "4".into()),
        ("masked", "****1234".into()),
        ("installation", "C:\\StarCitizen".into()),
        ("read", "9".into()),
        ("unknown", "1".into()),
        ("library", "objects/ship".into()),
        ("type_id", "0xCCCC0001".into()),
        ("index", "4".into()),
        ("file", "Model file".into()),
        ("nodes", "3".into()),
        ("meshes", "2".into()),
        ("materials", "2".into()),
        ("bones", "5".into()),
        ("rig", "protos_human_male_face_t1_pu".into()),
        ("by", "TEST_Plate".into()),
        ("skinned", "1".into()),
        ("morphs", "4".into()),
        ("animations", "2".into()),
        ("channels", "12".into()),
        ("source", "Data\\Animations\\ship.caf".into()),
        ("port", "hardpoint_weapon_nose".into()),
        ("entity", "TEST_Gun".into()),
        ("facts", "WeaponGun, size 1".into()),
        ("single", "yes".into()),
        ("images", 3.into()),
        ("textured", "2".into()),
        ("types", "WeaponGun (Gun)".into()),
        ("sizes", "1\u{2013}2".into()),
        ("excluded", "2".into()),
        ("replaced", "1".into()),
        ("parts", "48".into()),
        ("modelled", "40".into()),
        ("containers", "7".into()),
        ("models", "300".into()),
        ("of", "1,200".into()),
        ("lights", "12".into()),
        ("heading", "Failed (3)".into()),
        ("variants", "2 variants".into()),
        ("message", "Something is incomplete.".into()),
    ]
}

#[test]
fn both_files_define_exactly_the_texts_of_the_interface() {
    let expected: BTreeSet<String> = Text::all().map(|text| text.id().to_owned()).collect();

    assert_eq!(message_ids(GERMAN), expected, "de-DE");
    assert_eq!(message_ids(BRITISH_ENGLISH), expected, "en-GB");
}

#[test]
fn every_warning_kind_has_a_sentence_in_both_languages() {
    let warnings = [
        CraftingWarning::UnreadableRewardRecords(1),
        CraftingWarning::UnknownModifierRanges {
            count: 1,
            details: Vec::new(),
        },
        CraftingWarning::AmbiguousDismantle(2),
        CraftingWarning::UnreadableVehicleFiles {
            count: 1,
            details: Vec::new(),
        },
        CraftingWarning::UnplacedStarmapContainers(1),
        CraftingWarning::UnreadableShopRegistry,
        CraftingWarning::MissingShopRegistry,
        CraftingWarning::UnreadableShopContainers(1),
        CraftingWarning::UnreadableShopInventories {
            count: 1,
            details: Vec::new(),
        },
        CraftingWarning::MissingShopInventories(1),
        CraftingWarning::NoShopComponents,
    ];
    for source in [GERMAN, BRITISH_ENGLISH] {
        for warning in &warnings {
            assert!(
                source.contains(&format!("    [{}] ", warning.kind())),
                "{} has no sentence",
                warning.kind()
            );
        }
    }
}

#[test]
fn every_text_formats_without_errors_in_every_language() {
    let texts = Texts::new(Language::German);
    for language in Language::ALL {
        let bundle = texts.bundle(language);
        let mut args = fluent_bundle::FluentArgs::new();
        for (name, value) in sample_args() {
            args.set(name, value);
        }
        for text in Text::all() {
            let pattern = bundle
                .get_message(text.id())
                .and_then(|message| message.value())
                .unwrap_or_else(|| panic!("{} has no value in {language:?}", text.id()));
            let mut errors = Vec::new();
            let _formatted = bundle.format_pattern(pattern, Some(&args), &mut errors);
            assert!(
                errors.is_empty(),
                "{} in {language:?}: {errors:?}",
                text.id()
            );
        }
    }
}

#[test]
fn entry_counts_use_the_plural_of_the_language() {
    let mut texts = Texts::new(Language::BritishEnglish);
    let entries = |texts: &Texts, count: i32| {
        texts.format(
            Text::StatusEntries,
            &[
                ("count", count.into()),
                ("number", count.to_string().into()),
            ],
        )
    };

    assert_eq!(entries(&texts, 1), "1 Entry", "en-GB singular");
    assert_eq!(entries(&texts, 3), "3 Entries", "en-GB plural");
    texts.set_language(Language::German);
    assert_eq!(entries(&texts, 1), "1 Eintrag", "de-DE singular");
    assert_eq!(entries(&texts, 3), "3 Einträge", "de-DE plural");
}

#[test]
fn placeables_are_not_wrapped_in_isolation_marks() {
    let texts = Texts::new(Language::BritishEnglish);

    let text = texts.format(Text::SelectEntry, &[("name", "ship.cgf".into())]);

    assert_eq!(text, "Select ship.cgf", "no FSI/PDI around the name");
}
