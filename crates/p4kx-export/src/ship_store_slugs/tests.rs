// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::str::FromStr;

use p4kx_extraction::Guid;

use super::StoreSlugs;

const AURORA: &str = "11111111-1111-1111-1111-111111111111";
const GONE: &str = "22222222-2222-2222-2222-222222222222";
const OK: &str = "44444444-4444-4444-4444-444444444444";

fn data(ships: &str) -> String {
    format!(r#"{{"format": 1, "ships": [{ships}]}}"#)
}

fn mapped(slug: &str, id: u32, class: &str, guid: &str, retired: bool) -> String {
    format!(
        r#"{{"slug": "{slug}", "rsiId": {id}, "storeName": "{slug}", "className": "{class}", "guid": "{guid}", "retired": {retired}}}"#
    )
}

fn unmapped(slug: &str, id: u32) -> String {
    format!(
        r#"{{"slug": "{slug}", "rsiId": {id}, "storeName": "{slug}", "className": null, "guid": null, "retired": false}}"#
    )
}

fn ship(class: &'static str, guid: &str) -> (Guid, &'static str) {
    (Guid::from_str(guid).unwrap(), class)
}

fn error_of(text: &str) -> String {
    StoreSlugs::parse(text).err().unwrap().to_string()
}

#[test]
fn embedded_data_is_valid_and_maps_most_slugs() {
    let slugs = StoreSlugs::embedded().unwrap();
    let mapped = slugs
        .slugs
        .iter()
        .filter(|slug| slug.mapped.is_some())
        .count();
    assert!(mapped > 200, "{mapped} slugs are mapped");
    assert!(mapped <= slugs.slugs.len(), "mapped slugs are slugs");
    assert!(
        slugs.slugs.iter().all(|slug| !slug.slug.is_empty()),
        "every slug has text"
    );
    assert!(
        slugs
            .slugs
            .iter()
            .filter(|slug| slug.retired)
            .all(|slug| slug.mapped.is_some()),
        "a retired slug keeps its last mapping"
    );
}

fn with_ok(entry: &str) -> StoreSlugs {
    StoreSlugs::parse(&data(&format!(
        "{entry}, {}",
        mapped("Ok", 9, "RSI_Ok", OK, false)
    )))
    .unwrap()
}

#[test]
fn mapped_slugs_without_any_resolving_skip_the_stale_check() {
    let slugs = StoreSlugs::parse(&data(&mapped("A", 1, "RSI_A", GONE, false))).unwrap();
    let resolved = slugs.resolve([ship("RSI_Aurora_GS_ES", AURORA)], false);
    assert!(resolved.summary.stale_check_skipped);
    assert_eq!(resolved.summary.stale, Vec::<String>::new());
    assert!(resolved.exported.is_empty());
}

#[test]
fn some_resolving_slug_keeps_the_stale_check() {
    let slugs = with_ok(&mapped("A", 1, "RSI_A", GONE, false));
    let resolved = slugs.resolve([ship("RSI_Ok", OK)], false);
    assert!(!resolved.summary.stale_check_skipped);
    assert_eq!(resolved.summary.stale, ["A"]);
}

#[test]
fn a_mapped_slug_that_resolves_is_exported_with_its_ship() {
    let slugs = StoreSlugs::parse(&data(&mapped(
        "Aurora-ES",
        1,
        "RSI_Aurora_GS_ES",
        AURORA,
        false,
    )))
    .unwrap();
    let ships = [ship("RSI_Aurora_GS_ES", AURORA)];
    let resolved = slugs.resolve(ships, false);
    assert_eq!(resolved.exported.len(), 1);
    assert_eq!(resolved.exported[0].class_name, Some("RSI_Aurora_GS_ES"));
    assert!(resolved.exported[0].ship_guid.is_some());
    assert!(!resolved.exported[0].retired);
    assert_eq!(resolved.summary.resolved, 1);
    assert_eq!(resolved.summary.stale, Vec::<String>::new());
}

#[test]
fn a_mapped_slug_whose_ship_is_gone_is_stale_and_not_exported() {
    let slugs = with_ok(&mapped("Aurora-ES", 1, "RSI_Aurora_GS_ES", GONE, false));
    let ships = [ship("RSI_Aurora_GS_ES", AURORA), ship("RSI_Ok", OK)];
    let resolved = slugs.resolve(ships, false);
    assert_eq!(resolved.exported.len(), 1);
    assert_eq!(resolved.summary.stale, ["Aurora-ES"]);
}

#[test]
fn a_ship_that_changed_its_class_name_makes_the_slug_stale() {
    let slugs = with_ok(&mapped("Aurora-ES", 1, "RSI_Aurora_GS_ES", AURORA, false));
    let ships = [ship("RSI_Aurora_Renamed", AURORA), ship("RSI_Ok", OK)];
    let resolved = slugs.resolve(ships, false);
    assert_eq!(resolved.exported.len(), 1);
    assert_eq!(resolved.summary.stale, ["Aurora-ES"]);
}

#[test]
fn a_guid_alias_does_not_hide_a_matching_class() {
    let slugs = with_ok(&mapped("Aurora-ES", 1, "RSI_Aurora_GS_ES", AURORA, false));
    let mapped = ship("RSI_Aurora_GS_ES", AURORA);
    let alias = ship("RSI_Aurora_Alias", AURORA);
    let other = ship("RSI_Ok", OK);
    for ships in [[mapped, alias, other], [alias, mapped, other]] {
        let resolved = slugs.resolve(ships, false);
        assert_eq!(resolved.summary.resolved, 2);
        assert_eq!(resolved.summary.stale, Vec::<String>::new());
    }
}

#[test]
fn a_retired_slug_is_exported_flagged_without_a_ship() {
    let slugs = StoreSlugs::parse(&data(&mapped("Old", 1, "RSI_Old", GONE, true))).unwrap();
    let resolved = slugs.resolve([ship("RSI_Aurora_GS_ES", AURORA)], false);
    assert_eq!(resolved.exported.len(), 1);
    assert!(resolved.exported[0].retired);
    assert!(resolved.exported[0].ship_guid.is_none());
    assert!(resolved.exported[0].class_name.is_none());
    assert_eq!(resolved.summary.retired, 1);
    assert_eq!(resolved.summary.stale, Vec::<String>::new());
}

#[test]
fn a_retired_slug_whose_ship_exists_again_is_stale() {
    let slugs = with_ok(&mapped("Old", 1, "RSI_Aurora_GS_ES", AURORA, true));
    let resolved = slugs.resolve(
        [ship("RSI_Aurora_GS_ES", AURORA), ship("RSI_Ok", OK)],
        false,
    );
    assert_eq!(resolved.exported.len(), 1);
    assert_eq!(resolved.summary.stale, ["Old"]);
}

#[test]
fn an_unmapped_slug_is_counted_and_not_exported() {
    let slugs = StoreSlugs::parse(&data(&unmapped("Orion", 7))).unwrap();
    let resolved = slugs.resolve([], false);
    assert!(resolved.exported.is_empty());
    assert_eq!(resolved.summary.unmapped, 1);
    assert_eq!(resolved.summary.slugs, 1);
}

#[test]
fn a_filter_exports_only_resolving_slugs_and_reports_nothing_stale() {
    let text = data(&format!(
        "{}, {}, {}",
        mapped("A", 1, "RSI_A", AURORA, false),
        mapped("B", 2, "RSI_B", GONE, false),
        mapped(
            "C",
            3,
            "RSI_C",
            "33333333-3333-3333-3333-333333333333",
            true
        )
    ));
    let slugs = StoreSlugs::parse(&text).unwrap();
    let resolved = slugs.resolve([ship("RSI_A", AURORA)], true);
    assert_eq!(resolved.exported.len(), 1);
    assert_eq!(resolved.exported[0].slug, "A");
    assert_eq!(resolved.summary.stale, Vec::<String>::new());
}

#[test]
fn exported_entries_are_ordered_by_slug() {
    let text = data(&format!(
        "{}, {}",
        mapped("b", 1, "RSI_B", GONE, false),
        mapped("a", 2, "RSI_A", AURORA, false)
    ));
    let slugs = StoreSlugs::parse(&text).unwrap();
    let ships = [ship("RSI_A", AURORA), ship("RSI_B", GONE)];
    let order: Vec<&str> = slugs
        .resolve(ships, false)
        .exported
        .iter()
        .map(|entry| entry.slug)
        .collect();
    assert_eq!(order, ["a", "b"]);
}

#[test]
fn invalid_data_is_rejected() {
    assert!(error_of("{").contains("ship store slug"));
    assert!(error_of(r#"{"format": 2, "ships": []}"#).contains("unknown format 2"));
    let twice = data(&format!("{}, {}", unmapped("x", 1), unmapped("x", 2)));
    assert!(error_of(&twice).contains("slug x occurs twice"));
    let same_id = data(&format!("{}, {}", unmapped("x", 1), unmapped("y", 1)));
    assert!(error_of(&same_id).contains("RSI id 1 occurs twice"));
    let same_guid = data(&format!(
        "{}, {}",
        mapped("x", 1, "A", AURORA, false),
        mapped("y", 2, "B", AURORA, false)
    ));
    assert!(error_of(&same_guid).contains("maps a second slug"));
    let bad_guid = data(&mapped("x", 1, "A", "nope", false));
    assert!(error_of(&bad_guid).contains("slug x"));
    let half = data(
        r#"{"slug": "x", "rsiId": 1, "storeName": "x", "className": "A", "guid": null, "retired": false}"#,
    );
    assert!(error_of(&half).contains("but not both"));
    let retired = data(
        r#"{"slug": "x", "rsiId": 1, "storeName": "x", "className": null, "guid": null, "retired": true}"#,
    );
    assert!(error_of(&retired).contains("retired slug x has no mapping"));
}
