// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Language, LocalizationEntry, SourceInventory, UiMovie};
use p4kx_testkit::{
    TestEditText, TestEntry, TestPlace, exports_body, font3_body, import_assets_body,
};
use serde_json::Value;

use crate::ui_import_support::{
    CLIENT, FIELD, GREEN, TestResult, archive, client, font_movie, options, square, stage,
};

const ROUTER: &str = "Data\\UI\\Screens\\SWF\\router.gfx";
const LEAF: &str = "Data\\UI\\Screens\\SWF\\leaf.gfx";

#[test]
fn a_drawn_imported_glyph_binds_the_forwarded_font_and_the_exact_winning_export_item() -> TestResult
{
    let folder = tempfile::tempdir()?;
    let router = stage()
        .tag(71, &import_assets_body("leaf.gfx", 2, &[(4, "$Leaf")]))
        .tag(56, &exports_body(&[(4, "$Font")]))
        .bytes();
    let leaf = stage()
        .tag(75, &font3_body(4, "Font", &[square()]))
        .tag(56, &exports_body(&[(4, "$Other"), (4, "$Leaf")]))
        .bytes();
    let archive = archive(
        folder.path(),
        vec![
            TestEntry::zstd(CLIENT, client("router.gfx", "$Font")),
            TestEntry::zstd(ROUTER, router),
            TestEntry::zstd(LEAF, leaf).encrypted(),
        ],
    )?;
    let movie = UiMovie::from_archive(&archive, CLIENT)?;
    let ordinary = movie.render(&options())?;
    let observed = movie.render_with_bitmap_sources(&options())?;
    assert_eq!(
        ordinary.image, observed.render.image,
        "identical glyph pixels"
    );
    assert_eq!(
        ordinary.imports, observed.render.imports,
        "identical resolution"
    );
    assert_eq!(observed.glyphs.len(), 1, "one actually painted glyph");
    let drawn = observed.glyphs.first().ok_or("drawn outline")?;
    assert_eq!((drawn.font, drawn.glyph), (0, 0));
    assert_eq!(drawn.origin.contains_encrypted_source(), Some(true));
    assert!(drawn.origin.source_set_complete());
    assert_eq!(
        observed.movie_geometry.contains_encrypted_source(),
        Some(false)
    );
    assert_leaf_export(&observed.provenance)
}

fn assert_leaf_export(provenance: &SourceInventory) -> TestResult {
    let inventory = serde_json::to_value(provenance)?;
    let leaf = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("source inventory")?
        .iter()
        .find(|source| source.pointer("/path/value").and_then(Value::as_str) == Some(LEAF))
        .and_then(|source| source.get("id"))
        .ok_or("selected leaf Entry")?;
    let selected = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("source uses")?
        .iter()
        .find(|usage| {
            usage.get("source") == Some(leaf)
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("fontExport")
        })
        .ok_or("exact leaf export")?;
    assert_eq!(
        selected
            .pointer("/location/indices/5")
            .and_then(Value::as_str),
        Some("1"),
        "the earlier export of the same physical font has a different name"
    );
    Ok(())
}

#[test]
fn an_encrypted_candidate_that_lacks_the_export_supplies_no_drawn_font_data() -> TestResult {
    let folder = tempfile::tempdir()?;
    let rejected = "Data\\UI\\Screens\\SWF\\font.gfx";
    let used = "Data\\UI\\Q\\font.gfx";
    let archive = archive(
        folder.path(),
        vec![
            TestEntry::zstd(CLIENT, client("font.gfx", "$Font")),
            TestEntry::zstd(rejected, font_movie("$Other")).encrypted(),
            TestEntry::zstd(used, font_movie("$Font")),
        ],
    )?;
    let movie = UiMovie::from_archive(&archive, CLIENT)?;
    let observed = movie.render_with_bitmap_sources(&options())?;
    assert_eq!(observed.render.imports.movies, [rejected, used]);
    let drawn = observed.glyphs.first().ok_or("drawn outline")?;
    assert_eq!(drawn.origin.contains_encrypted_source(), Some(false));
    assert!(drawn.origin.source_set_complete());
    let inventory = serde_json::to_value(&observed.provenance)?;
    let sources = inventory
        .get("sources")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    assert!(
        sources.iter().all(|source| {
            source.pointer("/path/value").and_then(Value::as_str) != Some(rejected)
        }),
        "a read movie that supplied no glyph cannot contribute to the drawn outline"
    );
    Ok(())
}

#[test]
fn localization_marks_only_the_value_used_by_an_actual_draw() -> TestResult {
    let folder = tempfile::tempdir()?;
    let language = Language::English;
    let text = |id, initial| {
        TestEditText::new(id, FIELD)
            .text(initial)
            .font(4, 400)
            .color(GREEN)
    };
    let movie = stage()
        .tag(75, &font3_body(4, "Font", &[square()]))
        .tag(37, &text(7, "@synthetic_visible").body())
        .tag(37, &text(8, "@synthetic_outside").body())
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(26, &TestPlace::new(2, 8).at(10_000, 0).body2())
        .tag(1, &[])
        .bytes();
    let archive = archive(
        folder.path(),
        vec![
            TestEntry::zstd(CLIENT, movie),
            TestEntry::zstd(
                language.global_ini_path(),
                b"synthetic_visible=A\nsynthetic_outside=A\n",
            )
            .encrypted(),
        ],
    )?;
    let texts = LocalizationEntry::read(&archive, language)?;
    let movie = UiMovie::from_archive(&archive, CLIENT)?.with_texts(texts);
    let ordinary = movie.render(&options())?;
    let observed = movie.render_with_bitmap_sources(&options())?;
    assert_eq!(
        ordinary.image, observed.render.image,
        "identical translation"
    );
    assert_eq!(
        observed.localizations.len(),
        1,
        "the off-canvas text is unused"
    );
    let translation = observed.localizations.first().ok_or("drawn translation")?;
    assert_eq!(translation.definition, 0);
    assert_eq!(translation.origin.contains_encrypted_source(), Some(true));
    assert!(translation.origin.source_set_complete());
    let glyph = observed.glyphs.first().ok_or("local font outline")?;
    assert_eq!(glyph.origin.contains_encrypted_source(), Some(false));
    assert_localization_lookup(&observed.provenance)
}

fn assert_localization_lookup(provenance: &SourceInventory) -> TestResult {
    let inventory = serde_json::to_value(provenance)?;
    let uses = inventory
        .get("uses")
        .and_then(Value::as_array)
        .ok_or("uses")?;
    let lookup = uses
        .iter()
        .find(|use_| {
            use_.pointer("/location/type").and_then(Value::as_str) == Some("localizationLookup")
        })
        .ok_or("actual language lookup")?;
    assert!(
        lookup
            .pointer("/location/selector")
            .and_then(Value::as_str)
            .is_some(),
        "the retained authored selector is opaque"
    );
    assert!(
        !inventory.to_string().contains("synthetic_visible"),
        "the provenance does not disclose the localization key"
    );
    Ok(())
}

#[test]
fn a_space_uses_encrypted_font_metrics_without_drawing_an_encrypted_outline() -> TestResult {
    let folder = tempfile::tempdir()?;
    let mut space = square();
    space.code = u16::from(b' ');
    space.advance = 16_000;
    let wide = stage()
        .tag(75, &font3_body(4, "Wide", &[space]))
        .tag(56, &exports_body(&[(4, "$Wide")]))
        .bytes();
    let movie = stage()
        .tag(75, &font3_body(4, "Base", &[square()]))
        .tag(71, &import_assets_body("wide.gfx", 2, &[(9, "$Wide")]))
        .tag(37, &html_edit(7, 4, "A<font face=\"Wide\">&#32;</font>A"))
        .tag(37, &html_edit(8, 9, "A"))
        .tag(26, &TestPlace::new(1, 7).body2())
        .tag(26, &TestPlace::new(2, 8).at(10_000, 0).body2())
        .tag(1, &[])
        .bytes();
    let archive = archive(
        folder.path(),
        vec![
            TestEntry::zstd(CLIENT, movie),
            TestEntry::zstd("Data\\UI\\Screens\\SWF\\wide.gfx", wide).encrypted(),
        ],
    )?;
    let movie = UiMovie::from_archive(&archive, CLIENT)?;
    let ordinary = movie.render(&options())?;
    let observed = movie.render_with_bitmap_sources(&options())?;
    assert_eq!(observed.render.image, ordinary.image, "identical layout");
    let metric = observed
        .font_metrics
        .iter()
        .find(|metric| metric.font == 1 && metric.glyph == Some(0))
        .ok_or("space advance from the imported font")?;
    assert_eq!(metric.origin.contains_encrypted_source(), Some(true));
    assert!(metric.origin.source_set_complete());
    assert!(
        observed.glyphs.iter().all(|glyph| glyph.font == 0),
        "the space shapes the layout while only the clear local glyph is painted"
    );
    Ok(())
}

fn html_edit(id: u16, font: u16, text: &str) -> Vec<u8> {
    TestEditText::new(id, FIELD)
        .text(text)
        .font(font, 400)
        .color(GREEN)
        .html()
        .body()
}
