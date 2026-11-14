// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{Arc, DataForge, Entities, LoadoutResolver, Observer, Seen, TestResult};

use crate::resolver_observation::alias_fixture;

#[test]
fn explicit_physical_aliases_keep_independent_geometry_facts_and_read_proofs() -> TestResult {
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = forge.records().collect();
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    for (index, geometry, size) in [
        (0, "First.cgf", 1),
        (1, "Second.cgf", 2),
        (0, "First.cgf", 1),
    ] {
        let record = records.get(index).ok_or("alias missing")?;
        let node = resolver.resolve(record)?;
        assert_eq!(
            node.geometry().ok_or("geometry absent")?.path(),
            geometry,
            "an explicit physical alias never reuses another root's geometry"
        );
        assert_eq!(
            node.record_index,
            Some(record.index()),
            "the actual source index follows the resolved node"
        );
        assert_eq!(
            resolver.item_facts(record)?.size(),
            Some(size),
            "facts use the explicitly selected physical root"
        );
        assert_eq!(
            resolver
                .wearable(record)?
                .geometry()
                .ok_or("wearable geometry absent")?
                .path(),
            geometry,
            "clothing geometry uses the explicitly selected physical root"
        );
    }
    let seen = observer.events();
    for index in [0, 1] {
        assert_eq!(
            seen.iter()
                .filter(|event| matches!(event, Seen::Read(found, true, _) if *found == index))
                .count(),
            1,
            "each actual physical root is decoded once"
        );
        assert!(
            seen.contains(&Seen::Reused(index)),
            "cache reuse retains the exact source index"
        );
    }
    Ok(())
}

#[test]
fn a_wrong_alias_supplied_tree_is_ignored_and_the_actual_root_is_decoded_once() -> TestResult {
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let first = forge.record(0).ok_or("first alias missing")?;
    let second = forge.record(1).ok_or("second alias missing")?;
    let foreign_tree = first.tree()?;
    let observer = Arc::new(Observer::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    let node = resolver.resolve_tree(&second, &foreign_tree)?;
    assert_eq!(
        node.geometry().ok_or("geometry absent")?.path(),
        "Second.cgf",
        "GUID equality does not authorize another physical owner's canonical tree"
    );
    assert_eq!(
        node.record_index,
        Some(1),
        "the returned node belongs to the actual requested root"
    );
    let seen = observer.events();
    assert_eq!(
        seen.iter()
            .filter(|event| matches!(event, Seen::Read(1, true, _)))
            .count(),
        1,
        "the rejected supplied tree causes exactly one actual root read"
    );
    assert!(
        !seen
            .iter()
            .any(|event| matches!(event, Seen::Provided(_) | Seen::Read(0, ..))),
        "a foreign alias is neither accepted as provided proof nor decoded as the requested root"
    );
    Ok(())
}

#[test]
fn known_node_sub_geometry_keeps_its_physical_alias() -> TestResult {
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    resolver.resolve(&forge.record(0).ok_or("first alias missing")?)?;
    let holder = resolver.resolve(&forge.record(2).ok_or("holder missing")?)?;
    let crate::Attachment::Entity(child) =
        holder.ports().first().ok_or("port missing")?.attachment()
    else {
        return Err("alias attachment missing".into());
    };
    assert_eq!(
        child.record_index,
        Some(1),
        "the class target retains its exact resolved physical index"
    );
    assert_eq!(
        child.geometry().ok_or("sub-geometry absent")?.path(),
        "Second-male.cgf",
        "port flags select sub-geometry from the known child owner, not another cached GUID alias"
    );
    Ok(())
}
