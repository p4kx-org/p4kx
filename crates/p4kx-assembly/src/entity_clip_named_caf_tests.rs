// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::TestResult;
use crate::{ClipSourceSelection, EntityClipCatalogue, NamedCafTarget};

pub(super) fn verify_named_caf(catalogue: &EntityClipCatalogue) -> TestResult {
    let [_, child] = catalogue.rigs.as_slice() else {
        return Err("root and child rigs".into());
    };
    let [
        folder,
        _,
        relative,
        rooted,
        missing,
        invalid,
        ambiguous,
        missing_scope,
    ] = child.links.as_slice()
    else {
        return Err("direct CAF outcomes in authored order".into());
    };
    let ClipSourceSelection::NamedCaf {
        filepath_parameter: Some(scope),
        target: NamedCafTarget::File(relative_source),
        ..
    } = relative.selection
    else {
        return Err("relative CAF uses the preceding same-list folder".into());
    };
    let ClipSourceSelection::NamedCaf {
        filepath_parameter: None,
        target: NamedCafTarget::File(rooted_source),
        ..
    } = rooted.selection
    else {
        return Err("rooted CAF stands independently of the folder".into());
    };
    assert_eq!(scope, folder.parameter, "record actual folder selector");
    assert_eq!(relative_source, rooted_source, "share one physical CAF");
    verify_named_failures(missing, invalid, ambiguous, missing_scope);
    Ok(())
}

fn verify_named_failures(
    missing: &crate::ClipSourceLink,
    invalid: &crate::ClipSourceLink,
    ambiguous: &crate::ClipSourceLink,
    missing_scope: &crate::ClipSourceLink,
) {
    assert!(matches!(
        missing.selection,
        ClipSourceSelection::NamedCaf {
            filepath_parameter: Some(_),
            target: NamedCafTarget::Missing,
            archive_path: Some(_),
        }
    ));
    assert!(matches!(
        invalid.selection,
        ClipSourceSelection::NamedCaf {
            target: NamedCafTarget::InvalidPath,
            ..
        }
    ));
    assert!(matches!(
        ambiguous.selection,
        ClipSourceSelection::NamedCaf {
            target: NamedCafTarget::Ambiguous,
            ..
        }
    ));
    assert!(matches!(
        missing_scope.selection,
        ClipSourceSelection::NamedCaf {
            filepath_parameter: None,
            target: NamedCafTarget::MissingScope,
            archive_path: None,
        }
    ));
}
