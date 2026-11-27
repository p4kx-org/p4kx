// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::markup::Budget;
use crate::vehicle::aggregate_evidence::support::{self, TestResult};
use crate::{ValueOrigin, VehicleAggregateIssue};

#[test]
fn failed_tree_mutation_restores_selector_membership_and_allows_later_reinsertion() -> TestResult {
    let text = b"<Vehicle><Parts><Part mass=\"2\"/></Parts></Vehicle>";
    support::with_original_root(text, |root, mut evidence| {
        let original = root.clone();
        let before = evidence.evidence().selectors().count();
        let result: Result<(), String> = super::guarded(
            root,
            &mut Budget::new(text.len()),
            None,
            Some(&mut evidence),
            |root, _, aggregate| {
                root.set_attribute("name", "rejected", ValueOrigin::Elem, None);
                let aggregate = aggregate.ok_or("opt-in aggregate scope")?;
                aggregate.element(root, "$attributes");
                aggregate.issue(VehicleAggregateIssue::PatchApplicationFailed);
                Err("synthetic mutation failure".to_owned())
            },
        );
        assert!(result.is_err());
        assert_eq!(*root, original);
        assert_eq!(evidence.evidence().selectors().count(), before);
        assert_eq!(
            evidence.evidence().issues(),
            &[],
            "failed mutation issues roll back"
        );
        super::guarded(
            root,
            &mut Budget::new(text.len()),
            None,
            Some(&mut evidence),
            |root, _, aggregate| {
                root.set_attribute("name", "accepted", ValueOrigin::Elem, None);
                aggregate
                    .ok_or("opt-in aggregate scope")?
                    .element(root, "$attributes");
                Ok(())
            },
        )?;
        assert_eq!(root.attribute("name"), Some("accepted"));
        let evidence = evidence.finish();
        assert_eq!(evidence.selectors().count(), before + 1);
        assert!(evidence.has_complete_coordinates());
        Ok(())
    })
}
