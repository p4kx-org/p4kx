// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;

use p4kx_export::CatalogError;
use p4kx_export::catalog_queries::crafting::BlueprintRewardSelection;
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_extraction::{Archive, CancelToken, Guid, Language};

use crate::commodity_v3_fixture::{TestResult, guid};
use crate::crafting_detail_fixture::TWO_TIERS;

use super::cache::{Published, refresh};
use super::fixture;
use super::queries::{by_guid, collect, require_complete};

#[test]
fn consumer_refresh_commits_changed_build_and_reuses_warm_cache() -> TestResult {
    let (_cold_folder, cold) = fixture::archive("11650001", false, false)?;
    let (_changed_folder, changed) = fixture::archive("11650002", true, true)?;
    let language = Language::English;
    let mut cache = None;
    refresh(
        &mut cache,
        &cold,
        language,
        || collect(&cold, language, None, || {}),
        |_| Ok(()),
    )?;
    let first = cache.clone().ok_or("cold cache should publish")?;
    check_cold(&first)?;

    refresh(
        &mut cache,
        &changed,
        language,
        || collect(&changed, language, None, || {}),
        |_| Ok(()),
    )?;
    let second = cache.clone().ok_or("changed Build should publish")?;
    assert_eq!(
        second.identity.build.change_number(),
        "11650002",
        "changed Build identity"
    );
    assert!(
        second.dashboard.location_disabled,
        "changed original location field"
    );
    assert!(
        second.dashboard.reward_link_encrypted,
        "changed input is encrypted"
    );
    assert_eq!(
        second.dashboard.first_port, first.dashboard.first_port,
        "stable original port"
    );
    assert_eq!(
        second.dashboard.blueprint, first.dashboard.blueprint,
        "stable Blueprint target"
    );
    refresh(
        &mut cache,
        &changed,
        language,
        || panic!("warm cache must avoid opening a DataForge session"),
        |_| panic!("warm cache must avoid publication"),
    )?;
    assert_eq!(
        cache.as_ref(),
        Some(&second),
        "warm cache retains successful state"
    );
    Ok(())
}

fn check_cold(first: &Published) -> TestResult {
    assert_eq!(
        first.identity.build.change_number(),
        "11650001",
        "cold Build identity"
    );
    assert!(!first.dashboard.location_disabled, "cold location field");
    assert!(
        !first.dashboard.reward_link_encrypted,
        "cold input is plain"
    );
    assert_eq!(first.dashboard.first_port, "mount", "first original port");
    assert_eq!(
        first.dashboard.blueprint,
        guid(TWO_TIERS).parse::<Guid>()?,
        "physical Blueprint target"
    );

    Ok(())
}

#[test]
fn consumer_refresh_invalidates_same_build_when_requested_locale_changes() -> TestResult {
    let (_folder, archive) = fixture::archive("11650003", false, false)?;
    let mut cache = None;
    refresh(
        &mut cache,
        &archive,
        Language::English,
        || collect(&archive, Language::English, None, || {}),
        |_| Ok(()),
    )?;
    let reads = Cell::new(0);
    let writes = Cell::new(0);
    refresh(
        &mut cache,
        &archive,
        Language::German,
        || {
            reads.set(reads.get() + 1);
            collect(&archive, Language::German, None, || {})
        },
        |_| {
            writes.set(writes.get() + 1);
            Ok(())
        },
    )?;
    let current = cache.as_ref().ok_or("locale change should publish")?;
    assert_eq!(
        current.identity.language,
        Language::German,
        "requested locale is part of the cache key"
    );
    assert_eq!(
        (reads.get(), writes.get()),
        (1, 1),
        "locale change reads and publishes once"
    );
    Ok(())
}

#[test]
fn consumer_refresh_keeps_last_good_on_cancellation_and_replacement_failure() -> TestResult {
    let (_cold_folder, cold) = fixture::archive("11650001", false, false)?;
    let (_changed_folder, changed) = fixture::archive("11650002", true, true)?;
    let language = Language::English;
    let mut cache = None;
    refresh(
        &mut cache,
        &cold,
        language,
        || collect(&cold, language, None, || {}),
        |_| Ok(()),
    )?;
    let first = cache.clone().ok_or("cold cache should publish")?;
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = refresh(
        &mut cache,
        &changed,
        language,
        || {
            collect(&changed, language, Some(&cancel), || {
                panic!("pre-cancelled callback")
            })
        },
        |_| panic!("pre-cancelled publication"),
    );
    check_cancelled(result)?;
    assert_eq!(
        cache.as_ref(),
        Some(&first),
        "pre-cancelled refresh retains last good cache"
    );
    let cancel = CancelToken::new();
    let result = refresh(
        &mut cache,
        &changed,
        language,
        || collect(&changed, language, Some(&cancel), || cancel.cancel()),
        |_| panic!("cancelled outer query must not publish"),
    );
    check_cancelled(result)?;
    assert_eq!(
        cache.as_ref(),
        Some(&first),
        "post-callback cancellation retains last good cache"
    );
    check_failed_replacement(&mut cache, &changed, &first)
}

fn check_failed_replacement(
    cache: &mut Option<Published>,
    archive: &Archive,
    previous: &Published,
) -> TestResult {
    let language = Language::English;
    let result = refresh(
        cache,
        archive,
        language,
        || collect(archive, language, None, || {}),
        |_| Err(std::io::Error::other("synthetic cache replacement failure").into()),
    );
    let Err(error) = result else {
        return Err("consumer must propagate replacement failure".into());
    };
    assert!(
        error.downcast_ref::<std::io::Error>().is_some(),
        "the original replacement error survives"
    );
    assert_eq!(
        cache.as_ref(),
        Some(previous),
        "failed replacement retains last good cache"
    );
    Ok(())
}

fn check_cancelled(result: TestResult) -> TestResult {
    let Err(error) = result else {
        return Err("outer refresh must report cancellation".into());
    };
    assert!(
        matches!(
            error.downcast_ref::<CatalogError>(),
            Some(CatalogError::Cancelled)
        ),
        "the typed cancellation cause survives",
    );
    Ok(())
}

#[test]
fn consumer_refresh_distinguishes_absent_owner_from_unreadable_requested_body() -> TestResult {
    let (_folder, archive) = fixture::archive("11650001", false, false)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let missing = BlueprintRewardSelection::SourceOwners(by_guid(fixture::MISSING_OWNER)?);
    parsed.with_blueprint_reward_sources(missing, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "no requested source owner was admitted"
        );
        assert_eq!(
            snapshot.source_records_read(),
            0,
            "no unmatched body was opened"
        );
        require_complete(snapshot.source_coverage())
    })??;
    let broken = BlueprintRewardSelection::SourceOwners(by_guid(fixture::BROKEN_OWNER)?);
    parsed.with_blueprint_reward_sources(broken, None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "requested original body was unreadable"
        );
        assert_eq!(
            snapshot.source_records_read(),
            1,
            "the requested body was attempted"
        );
        assert_eq!(
            snapshot.source_coverage().unreadable_bodies.len(),
            1,
            "one physical read failure"
        );
        assert!(
            require_complete(snapshot.source_coverage()).is_err(),
            "unreadable is not complete empty"
        );
    })?;
    Ok(())
}
