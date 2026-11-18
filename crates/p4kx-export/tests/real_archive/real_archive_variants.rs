// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups of ships and items in the P4KX catalog of a real Archive; runs with
//! `--ignored` when `P4KX_DATA_P4K` names one.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, Kind as CatalogKind, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

const BIGGEST: usize = 15;
const MEMBERS: usize = 14;
const SAMPLES: usize = 40;

#[expect(
    clippy::print_stdout,
    reason = "the diagnostics are the result of the run"
)]
fn print_line(line: &str) {
    println!("{line}");
}

struct Kind<'c> {
    name: CatalogKind,
    entries: &'c [Value],
}

impl Kind<'_> {
    fn class(entry: &Value, kind: CatalogKind) -> &str {
        catalog_v3_consumer::field(entry, kind, "className")
            .ok()
            .and_then(Value::as_str)
            .unwrap_or_default()
    }

    fn guid<'row>(&self, entry: &'row Value) -> Option<&'row str> {
        catalog_v3_consumer::field(entry, self.name, "guid")
            .ok()
            .and_then(Value::as_str)
    }

    fn groups(&self) -> BTreeMap<&str, Vec<&Value>> {
        let by_guid: HashMap<&str, &Value> = self
            .entries
            .iter()
            .filter_map(|entry| Some((self.guid(entry)?, entry)))
            .collect();
        let mut groups: BTreeMap<&str, Vec<&Value>> = BTreeMap::new();
        for entry in self.entries {
            if let Some(base) = entry["variantOf"].as_str() {
                groups.entry(base).or_default().push(entry);
            }
        }
        for (base, members) in &mut groups {
            if let Some(entry) = by_guid.get(base) {
                members.insert(0, entry);
            }
        }
        groups
    }

    fn print(&self) {
        let groups = self.groups();
        let variants: usize = groups.values().map(|members| members.len() - 1).sum();
        print_line(&format!(
            "{}: {} entries, {} groups, {} variants",
            self.name,
            self.entries.len(),
            groups.len(),
            variants
        ));
        let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
        for members in groups.values() {
            *sizes.entry(members.len()).or_default() += 1;
        }
        print_line(&format!(
            "{} group sizes (size: groups): {sizes:?}",
            self.name
        ));
        let mut biggest: Vec<_> = groups.values().collect();
        biggest.sort_by_key(|members| std::cmp::Reverse(members.len()));
        for members in biggest.into_iter().take(BIGGEST) {
            let classes: Vec<&str> = members
                .iter()
                .take(MEMBERS)
                .map(|m| Self::class(m, self.name))
                .collect();
            print_line(&format!(
                "  {} members, base {}: {}",
                members.len(),
                classes.first().copied().unwrap_or_default(),
                classes.join(", ")
            ));
        }
        self.print_near_misses(&groups);
    }

    fn print_near_misses(&self, groups: &BTreeMap<&str, Vec<&Value>>) {
        let grouped: HashSet<&str> = groups
            .values()
            .flatten()
            .filter_map(|entry| self.guid(entry))
            .collect();
        let stem = |class: &str| {
            class
                .rsplit_once('_')
                .map_or_else(|| class.to_owned(), |(s, _)| s.to_owned())
        };
        let mut stems: HashMap<String, usize> = HashMap::new();
        for entry in self.entries {
            *stems
                .entry(stem(Self::class(entry, self.name)))
                .or_default() += 1;
        }
        let singles: Vec<&str> = self
            .entries
            .iter()
            .filter(|entry| self.guid(entry).is_some_and(|g| !grouped.contains(g)))
            .map(|entry| Self::class(entry, self.name))
            .filter(|class| stems.get(&stem(class)).is_some_and(|count| *count > 1))
            .take(SAMPLES)
            .collect();
        print_line(&format!(
            "{} singletons that share a name stem with another entry (first {SAMPLES}): {}",
            self.name,
            singles.join(", ")
        ));
    }

    fn assert_resolves(&self) {
        let guids: HashSet<&str> = self
            .entries
            .iter()
            .filter_map(|entry| self.guid(entry))
            .collect();
        for entry in self.entries {
            let value = &entry["variantOf"];
            assert!(
                value.is_null() || value.is_string(),
                "{} variantOf",
                Self::class(entry, self.name)
            );
            if let Some(base) = value.as_str() {
                assert!(
                    guids.contains(base),
                    "{} {} variantOf resolves to another kind or nothing",
                    self.name,
                    Self::class(entry, self.name)
                );
                assert_ne!(
                    Some(base),
                    self.guid(entry),
                    "{} is its own base",
                    Self::class(entry, self.name)
                );
            }
        }
    }
}

/// Returns the class name (lower case) of the base of the ship `class`, itself for a base.
fn base_class(catalog: &Value, class: &str) -> Option<String> {
    let ships = catalog["ships"].as_array()?;
    let ship = ships
        .iter()
        .find(|ship| Kind::class(ship, CatalogKind::Ships).eq_ignore_ascii_case(class))?;
    let Some(base) = ship["variantOf"].as_str() else {
        return Some(class.to_ascii_lowercase());
    };
    ships
        .iter()
        .find(|ship| ship.pointer("/guid/value").and_then(Value::as_str) == Some(base))
        .map(|ship| Kind::class(ship, CatalogKind::Ships).to_ascii_lowercase())
}

fn assert_cutlass_family(catalog: &Value) {
    let bases: Vec<Option<String>> = [
        "DRAK_Cutlass_Black",
        "DRAK_Cutlass_Steel",
        "DRAK_Cutlass_Red",
        "DRAK_Cutlass_Blue",
    ]
    .iter()
    .map(|class| base_class(catalog, class))
    .collect();
    assert!(
        bases.iter().all(Option::is_some),
        "the four Cutlass ships exist"
    );
    let distinct: HashSet<&Option<String>> = bases.iter().collect();
    assert_eq!(distinct.len(), 4, "Black, Steel, Red and Blue stay apart");
    let black = Some("drak_cutlass_black".to_owned());
    for class in [
        "DRAK_Cutlass_Black_Exec_Stealth",
        "DRAK_Cutlass_Black_PU_AI_CRIM",
    ] {
        assert_eq!(
            base_class(catalog, class),
            black,
            "{class} groups under Black"
        );
    }
}

fn print_family(catalog: &Value, needle: &str) {
    let ships = catalog["ships"].as_array().map_or(&[][..], Vec::as_slice);
    let by_guid: HashMap<&str, &str> = ships
        .iter()
        .filter_map(|ship| {
            Some((
                ship.pointer("/guid/value").and_then(Value::as_str)?,
                ship.pointer("/className/value").and_then(Value::as_str)?,
            ))
        })
        .collect();
    for ship in ships {
        let class = Kind::class(ship, CatalogKind::Ships);
        if class.to_ascii_lowercase().contains(needle) {
            let base = ship["variantOf"]
                .as_str()
                .and_then(|guid| by_guid.get(guid));
            print_line(&format!(
                "  {class}: definition {}, playability {}, variantOf {}",
                ship.pointer("/vehicleDefinition/value")
                    .unwrap_or(&Value::Null),
                ship["playability"],
                base.copied().unwrap_or("-")
            ));
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_variant_groups_of_a_real_archive_resolve_within_their_kind() -> TestResult {
    let archive =
        Archive::open_with_key(real_archive_path()?, p4kx_testkit::real_archive_key().ok())?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let summary = built.summary().variants();
    print_line(&format!(
        "summary ships {:?}, items {:?}",
        summary.ships, summary.items
    ));
    let kinds = [
        Kind {
            name: CatalogKind::Ships,
            entries: catalog["ships"].as_array().map_or(&[][..], Vec::as_slice),
        },
        Kind {
            name: CatalogKind::Items,
            entries: catalog_v3_consumer::rows(&catalog, CatalogKind::Items)?,
        },
    ];
    for kind in &kinds {
        kind.print();
    }
    for needle in ["cutlass", "hornet", "constellation"] {
        print_line(&format!("family {needle}:"));
        print_family(&catalog, needle);
    }
    for kind in &kinds {
        kind.assert_resolves();
    }
    assert_cutlass_family(&catalog);
    assert!(summary.ships.groups > 0, "ship groups");
    assert_eq!(
        summary.ships.groups,
        kinds[0].groups().len(),
        "summary matches the entries"
    );
    assert_eq!(
        summary.items.groups,
        kinds[1].groups().len(),
        "summary matches the entries"
    );
    Ok(())
}
