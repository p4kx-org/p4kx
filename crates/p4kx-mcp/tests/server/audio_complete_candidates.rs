// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::json;

use crate::audio_candidate_assertions as check;
use crate::audio_candidate_fixture::{self as fixture, Scenario};
use crate::support::TestResult;

#[test]
fn audio_candidate_info_reaches_the_ninth_real_location_after_eight_prefetches() -> TestResult {
    let input = fixture::create(Scenario::PrefetchThenFull)?;
    let expected = input
        .original
        .full
        .as_deref()
        .ok_or("the original ninth complete candidate")?;
    assert_eq!(
        expected,
        fixture::bank_path(9),
        "the original source proof establishes the ninth location"
    );
    let mut server = input.install.serve(&["--archive", "LIVE"])?;
    check::info(
        &mut server,
        &json!({"wem_id":fixture::ID,"limit":1}),
        expected,
        9,
    )
}

#[test]
fn audio_candidate_excerpt_reaches_the_ninth_real_location_after_eight_prefetches() -> TestResult {
    let input = fixture::create(Scenario::PrefetchThenFull)?;
    let expected = input
        .original
        .full
        .as_deref()
        .ok_or("the original ninth complete candidate")?;
    assert_eq!(
        expected,
        fixture::bank_path(9),
        "the original source proof establishes the ninth location"
    );
    let mut server = input.install.serve(&["--archive", "LIVE"])?;
    check::excerpt(&mut server, &check::arguments(), expected)
}

#[test]
fn audio_candidate_info_does_not_let_eight_errors_hide_a_later_readable_sound() -> TestResult {
    let input = fixture::create(Scenario::ErrorsThenFull)?;
    let expected = input
        .original
        .full
        .as_deref()
        .ok_or("the original ninth readable candidate")?;
    let mut server = input.install.serve(&["--archive", "LIVE"])?;
    check::info(&mut server, &json!({"wem_id":fixture::ID}), expected, 9)
}

#[test]
fn audio_candidate_excerpt_success_keeps_priority_over_eight_prior_errors() -> TestResult {
    let input = fixture::create(Scenario::ErrorsThenFull)?;
    let expected = input
        .original
        .full
        .as_deref()
        .ok_or("the original ninth readable candidate")?;
    let mut server = input.install.serve(&["--archive", "LIVE"])?;
    check::excerpt(&mut server, &check::arguments(), expected)
}

#[test]
fn audio_candidate_all_unsuitable_preserves_first_error_and_prefetch_fallbacks() -> TestResult {
    for scenario in [Scenario::OnlyPrefetch, Scenario::MixedNoFull] {
        let input = fixture::create(scenario)?;
        assert!(
            input.original.full.is_none(),
            "all nine actual original candidates are unsuitable"
        );
        let mut server = input.install.serve(&["--archive", "LIVE"])?;
        check::fallbacks(&mut server, &input.original)?;
    }
    Ok(())
}

#[test]
fn audio_candidate_first_readable_and_explicit_path_selection_remain_stable() -> TestResult {
    let input = fixture::create(Scenario::FirstFull)?;
    let expected = input
        .original
        .full
        .as_deref()
        .ok_or("actual first full sound")?;
    assert_eq!(
        expected,
        fixture::bank_path(1),
        "an earlier actual readable candidate is present"
    );
    let mut server = input.install.serve(&["--archive", "LIVE"])?;
    check::info(&mut server, &json!({"wem_id":fixture::ID}), expected, 9)?;
    check::excerpt(&mut server, &check::arguments(), expected)?;
    check::info(
        &mut server,
        &json!({"path":fixture::DIRECT}),
        fixture::DIRECT,
        1,
    )?;
    check::excerpt(
        &mut server,
        &json!({"path":fixture::DIRECT,"duration_seconds":0.001}),
        fixture::DIRECT,
    )?;
    Ok(())
}
#[test]
fn audio_candidate_prefetch_only_is_scoped_to_the_supplied_index() -> TestResult {
    let input = fixture::create(Scenario::OnlyPrefetch)?;
    assert!(
        input.original.full.is_none(),
        "the actual indexed candidates are prefetched"
    );
    let mut server = input.install.serve(&["--archive", "LIVE"])?;
    let error = server.error("get_audio_excerpt", &check::arguments())?;
    assert!(
        error.contains("indexed locations"),
        "the negative conclusion is limited to the actual index: {error}"
    );
    assert!(
        error.contains("prefetched start"),
        "the actual unsuitable source state remains explicit: {error}"
    );
    Ok(())
}
