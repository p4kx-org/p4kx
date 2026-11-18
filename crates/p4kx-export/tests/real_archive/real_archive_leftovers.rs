// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The ship tokens of the development-leftover rule on a real Archive; runs with `--ignored` when
//! `P4KX_DATA_P4K` names one. Run with `--nocapture` to see the Records the tokens leave out.

use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, EXCLUDED_TOKENS, SHIP_EXCLUDED_TOKENS, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;
use serde_json::Value;

use crate::catalog_v3_consumer;

type TestResult = Result<(), Box<dyn Error>>;

#[expect(clippy::print_stdout, reason = "the lists are the result of the run")]
fn print_line(line: &str) {
    println!("{line}");
}

fn has_token(text: &str, tokens: &[&str]) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| tokens.iter().any(|token| word.eq_ignore_ascii_case(token)))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_ship_tokens_leave_out_leftover_ships_only() -> TestResult {
    let archive =
        Archive::open_with_key(real_archive_path()?, p4kx_testkit::real_archive_key().ok())?;
    let built = build_catalog(&archive, &CatalogOptions::new(SystemTime::now()))?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut newly = Vec::new();
    for record in forge.records() {
        let (path, name) = (record.record_path()?, record.name()?);
        let by_new =
            has_token(path, &SHIP_EXCLUDED_TOKENS) || has_token(name, &SHIP_EXCLUDED_TOKENS);
        let by_old = has_token(path, &EXCLUDED_TOKENS) || has_token(name, &EXCLUDED_TOKENS);
        if by_new
            && !by_old
            && (path.contains("/entities/spaceships/")
                || path.contains("/entities/groundvehicles/"))
        {
            newly.push(path.to_owned());
        }
    }
    newly.sort();
    print_line(&format!("{} ship Records newly left out", newly.len()));
    assert!(newly.len() >= 21, "{} ships left out", newly.len());
    for known in [
        "glsn_shiv_temp_cosa",
        "aegs_avenger_stalker_display",
        "orig_890jump_hijacked",
    ] {
        assert!(
            newly.iter().any(|path| path.ends_with(known)),
            "{known} is left out"
        );
    }
    for path in &newly {
        print_line(path);
    }
    check_catalog(&catalog)
}

fn check_catalog(catalog: &Value) -> TestResult {
    for entity in catalog["ships"].as_array().ok_or("no ships")? {
        let path = catalog_v3_consumer::present(&entity["recordPath"])?
            .as_str()
            .ok_or("Ship record path")?;
        let class = catalog_v3_consumer::present(&entity["className"])?
            .as_str()
            .ok_or("Ship class name")?;
        assert!(
            !has_token(path, &SHIP_EXCLUDED_TOKENS) && !has_token(class, &SHIP_EXCLUDED_TOKENS),
            "ship {class} is in the catalog"
        );
    }
    let items = catalog["items"].as_array().ok_or("no items")?;
    assert!(
        items.iter().any(|item| {
            has_token(
                item.pointer("/className/value")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                &SHIP_EXCLUDED_TOKENS,
            )
        }),
        "items keep their display Records"
    );
    Ok(())
}
