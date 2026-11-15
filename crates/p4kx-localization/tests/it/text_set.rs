// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Resolving keys with a fallback language and listing keys, through the public API.

use std::error::Error;

use p4kx_localization::{GlobalIni, IniText, Language, Lookup, TextSet, decode_line_breaks};
use p4kx_testkit::GlobalIniBuilder;

type TestResult = Result<(), Box<dyn Error>>;

fn english() -> Vec<u8> {
    GlobalIniBuilder::new()
        .text("item_Name_Rifle", "Rifle")
        .text("item_Desc_Rifle", "Shoots.\\nWell.")
        .text("Only_English", "English only")
        .text("empty_in_german", "Filled")
        .text("LOC_EMPTY", "")
        .build()
}

fn german() -> Vec<u8> {
    GlobalIniBuilder::new()
        .text("ITEM_NAME_RIFLE", "Gewehr")
        .text("empty_in_german", "")
        .text("only_german", "Nur Deutsch")
        .build()
}

#[test]
fn a_key_resolves_in_the_language_and_falls_back_to_the_fallback_language() -> TestResult {
    let (english, german) = (english(), german());
    let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
    let set = TextSet::new([
        (Language::German, de.index()?),
        (Language::English, en.index()?),
    ]);
    assert_eq!(
        set.languages().collect::<Vec<_>>(),
        [Language::English, Language::German],
        "in the order of Language::ALL"
    );
    assert_eq!(
        set.lookup(
            "@item_Name_Rifle",
            Language::German,
            Some(Language::English)
        ),
        Lookup::Found {
            text: "Gewehr",
            language: Language::German
        },
        "own language first"
    );
    assert_eq!(
        set.lookup("@Only_English", Language::German, Some(Language::English)),
        Lookup::Found {
            text: "English only",
            language: Language::English
        },
        "fallback"
    );
    assert_eq!(
        set.lookup("empty_in_german", Language::German, Some(Language::English))
            .text(),
        Some("Filled"),
        "an empty text falls back too"
    );
    Ok(())
}

#[test]
fn a_key_without_a_text_is_missing() -> TestResult {
    let (english, german) = (english(), german());
    let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
    let set = TextSet::new([
        (Language::English, en.index()?),
        (Language::German, de.index()?),
    ]);
    assert!(
        set.lookup("@Only_English", Language::German, None)
            .is_missing(),
        "no fallback"
    );
    assert!(
        set.lookup("@nowhere", Language::German, Some(Language::English))
            .is_missing(),
        "in neither"
    );
    assert!(
        set.lookup("@Only_English", Language::French, Some(Language::French))
            .is_missing(),
        "a language without a file"
    );
    assert_eq!(
        set.lookup("@LOC_EMPTY", Language::English, None),
        Lookup::Placeholder,
        "placeholder key"
    );
    Ok(())
}

#[test]
fn blank_space_is_a_placeholder_key_in_any_language() -> TestResult {
    let bytes = GlobalIniBuilder::new().text("blank_space", " ").build();
    let text = IniText::decode(&bytes)?;
    let set = TextSet::new([(Language::English, text.index()?)]);
    for key in ["@blank_space", "BLANK_SPACE", "@Blank_Space"] {
        assert_eq!(
            set.lookup(key, Language::English, None),
            Lookup::Placeholder,
            "{key} stands for no text"
        );
        assert_eq!(
            set.to_owned_set().lookup(key, Language::English, None),
            Lookup::Placeholder,
            "{key} in the owned set"
        );
        assert!(!set.is_translated(key, Language::English), "{key}");
    }
    assert!(p4kx_localization::is_placeholder_key("@blank_space"));
    Ok(())
}

#[test]
fn a_plain_key_beats_its_comma_suffixed_line_in_lookups() -> TestResult {
    let bytes = GlobalIniBuilder::new()
        .text("both,P", "Suffixed")
        .text("both", "Plain")
        .text("pending,P", "Pending")
        .build();
    let text = IniText::decode(&bytes)?;
    let set = TextSet::new([(Language::English, text.index()?)]);
    let owned = set.to_owned_set();
    assert_eq!(
        set.lookup("@both", Language::English, None).text(),
        Some("Plain")
    );
    assert_eq!(
        owned.lookup("@both", Language::English, None).text(),
        Some("Plain")
    );
    assert_eq!(
        set.lookup("pending", Language::English, None).text(),
        Some("Pending")
    );
    Ok(())
}

#[test]
fn is_translated_treats_an_empty_text_as_missing_like_lookup() -> TestResult {
    let (english, german) = (english(), german());
    let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
    let set = TextSet::new([
        (Language::English, en.index()?),
        (Language::German, de.index()?),
    ]);
    let owned = set.to_owned_set();
    let cases = [
        ("@item_Name_Rifle", Language::German, true),
        ("item_name_rifle", Language::English, true),
        ("Only_English", Language::German, false),
        ("empty_in_german", Language::German, false),
        ("empty_in_german", Language::English, true),
        ("LOC_EMPTY", Language::English, false),
        ("nowhere", Language::English, false),
        ("item_Name_Rifle", Language::French, false),
    ];
    for (key, language, translated) in cases {
        assert_eq!(
            set.is_translated(key, language),
            translated,
            "{key} {language}"
        );
        assert_eq!(
            owned.is_translated(key, language),
            translated,
            "owned {key} {language}"
        );
        assert_eq!(
            set.is_translated(key, language),
            matches!(set.lookup(key, language, None), Lookup::Found { .. }),
            "same as lookup {key} {language}"
        );
    }
    Ok(())
}

#[test]
fn an_owned_set_resolves_like_the_borrowed_one_and_outlives_it() -> TestResult {
    let owned = {
        let (english, german) = (english(), german());
        let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
        TextSet::new([
            (Language::English, en.index()?),
            (Language::German, de.index()?),
        ])
        .to_owned_set()
    };
    assert_eq!(
        owned.languages().collect::<Vec<_>>(),
        [Language::English, Language::German],
        "languages"
    );
    assert_eq!(owned.len(Language::German), 3, "German keys");
    assert!(owned.is_empty(Language::French), "no French file");
    assert_eq!(
        owned.text("@ITEM_name_rifle", Language::German),
        Some("Gewehr"),
        "case and @"
    );
    assert_eq!(
        owned.text("empty_in_german", Language::German),
        Some(""),
        "empty kept"
    );
    assert_eq!(owned.text("nowhere", Language::German), None, "absent");
    assert_eq!(
        owned.lookup("Only_English", Language::German, Some(Language::English)),
        Lookup::Found {
            text: "English only",
            language: Language::English
        },
        "fallback"
    );
    assert_eq!(
        owned.lookup("@LOC_EMPTY", Language::English, None),
        Lookup::Placeholder,
        "placeholder"
    );
    Ok(())
}

#[test]
fn an_owned_set_lists_the_texts_of_a_language_sorted_ignoring_case() -> TestResult {
    let (english, german) = (english(), german());
    let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
    let owned = TextSet::new([
        (Language::English, en.index()?),
        (Language::German, de.index()?),
    ])
    .to_owned_set();

    let german: Vec<_> = owned.iter(Language::German).collect();

    assert_eq!(
        german,
        [
            ("empty_in_german", ""),
            ("ITEM_NAME_RIFLE", "Gewehr"),
            ("only_german", "Nur Deutsch"),
        ],
        "sorted, empty kept"
    );
    assert_eq!(owned.iter(Language::French).count(), 0, "no French file");
    Ok(())
}

#[test]
fn fallible_key_union_and_owned_rows_preserve_source_text() -> TestResult {
    let owned = {
        let english = String::from("MiXeD,P=Draft\nMiXeD=Définitif\nEmpty=\n界=A\\n界\n");
        let german = String::from("mixed=Deutsch\nLocal=Nur Deutsch\n");
        let set = TextSet::new([
            (Language::German, GlobalIni::parse(&german)?),
            (Language::English, GlobalIni::parse(&english)?),
        ]);
        assert_eq!(
            set.try_keys()?,
            ["Empty", "Local", "MiXeD", "界"],
            "canonical spelling and order of the complete union"
        );
        set.try_to_owned_set()?
    };
    assert_eq!(
        owned.iter(Language::English).collect::<Vec<_>>(),
        [("Empty", ""), ("MiXeD", "Définitif"), ("界", "A\\n界")],
        "packed rows keep exact UTF-8 spans after their source is dropped"
    );
    assert_eq!(
        owned.text("@MIXED", Language::German),
        Some("Deutsch"),
        "other languages retain independent values"
    );
    assert_eq!(owned.text("Local", Language::English), None, "absent key");
    Ok(())
}

#[test]
fn keys_lists_each_key_once_sorted_ignoring_case() -> TestResult {
    let (english, german) = (english(), german());
    let (en, de) = (IniText::decode(&english)?, IniText::decode(&german)?);
    let set = TextSet::new([
        (Language::English, en.index()?),
        (Language::German, de.index()?),
    ]);
    assert_eq!(
        set.keys(),
        [
            "empty_in_german",
            "item_Desc_Rifle",
            "item_Name_Rifle",
            "LOC_EMPTY",
            "Only_English",
            "only_german"
        ],
        "union, English spelling first"
    );
    assert_eq!(set.text("only_german", Language::English), None, "absent");
    assert_eq!(
        set.text("empty_in_german", Language::German),
        Some(""),
        "present but empty"
    );
    Ok(())
}

#[test]
fn line_breaks_are_decoded_only_when_there_are_any() {
    assert_eq!(decode_line_breaks("a\\nb"), "a\nb", "decoded");
    assert!(
        matches!(decode_line_breaks("ab"), std::borrow::Cow::Borrowed(_)),
        "borrowed"
    );
}

#[test]
fn display_text_decodes_line_breaks_and_trims_the_ends() {
    use p4kx_localization::display_text;
    assert_eq!(display_text(" Moss Camo \\n"), "Moss Camo");
    assert_eq!(display_text("A\\nB \\n"), "A\nB");
    assert_eq!(display_text("\t \\n"), "");
    assert_eq!(display_text("Plain"), "Plain");
    assert_eq!(display_text("\u{a0}Moss\u{3000}"), "Moss");
    assert_eq!(display_text("\u{3000}A\\nB\u{a0}\\n"), "A\nB");
}
