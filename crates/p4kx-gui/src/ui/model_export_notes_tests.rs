// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::model::MaterialStatus;

use crate::{Language, MaterialProblem, Texts};

use super::library_note;

type TestResult = Result<(), Box<dyn Error>>;

const ORIGINAL: &str = "/materials/fixture/library";
const TARGET: &str = "Data/Materials/Fixture/Library.mtl";
const CAUSE: &str = "XML: fixture fault";

#[test]
fn a_mapped_missing_library_keeps_its_status_in_both_localised_notes() -> TestResult {
    let problem = MaterialProblem::of(&MaterialStatus::Missing {
        path: ORIGINAL.to_owned(),
        target: Some(TARGET.to_owned()),
    })
    .ok_or("missing status was discarded")?;
    assert!(
        matches!(&problem, MaterialProblem::Missing { library, target: Some(target) }
        if library == ORIGINAL && target == TARGET),
        "mapping does not turn a missing library into a read error"
    );
    for (language, expected) in [
        (
            Language::BritishEnglish,
            "The material library /materials/fixture/library (resolved to Data/Materials/Fixture/Library.mtl) does not exist; the default material is used.",
        ),
        (
            Language::German,
            "Die Materialbibliothek /materials/fixture/library (aufgelöst zu Data/Materials/Fixture/Library.mtl) existiert nicht; es wird das Standardmaterial verwendet.",
        ),
    ] {
        let line = library_note(&Texts::new(language), &problem);
        assert_eq!(
            line, expected,
            "missing-library status and mapping are localised together"
        );
        assert!(
            !line.contains("cannot be read") && !line.contains("nicht lesbar"),
            "a missing library is not labelled unreadable"
        );
    }
    Ok(())
}

#[test]
fn a_mapped_unreadable_library_keeps_only_the_underlying_cause() -> TestResult {
    let problem = MaterialProblem::of(&MaterialStatus::Unreadable {
        path: ORIGINAL.to_owned(),
        target: Some(TARGET.to_owned()),
        reason: CAUSE.to_owned(),
    })
    .ok_or("unreadable status was discarded")?;
    assert!(
        matches!(&problem, MaterialProblem::Unreadable { library, target: Some(target), reason }
        if library == ORIGINAL && target == TARGET && reason == CAUSE),
        "the cause does not contain an English library sentence"
    );
    for (language, expected) in [
        (
            Language::BritishEnglish,
            "The material library /materials/fixture/library (resolved to Data/Materials/Fixture/Library.mtl) cannot be read (XML: fixture fault); the default material is used.",
        ),
        (
            Language::German,
            "Die Materialbibliothek /materials/fixture/library (aufgelöst zu Data/Materials/Fixture/Library.mtl) ist nicht lesbar (XML: fixture fault); es wird das Standardmaterial verwendet.",
        ),
    ] {
        let line = library_note(&Texts::new(language), &problem);
        assert_eq!(
            line, expected,
            "mapping prose follows the selected language"
        );
        assert_eq!(
            line.matches(ORIGINAL).count(),
            1,
            "original reference is not repeated in a complete nested reason"
        );
        assert_eq!(line.matches(TARGET).count(), 1, "destination is shown once");
        assert_eq!(
            line.matches(CAUSE).count(),
            1,
            "underlying cause is shown once"
        );
    }
    Ok(())
}

#[test]
fn unmapped_missing_and_unreadable_libraries_keep_the_existing_messages() -> TestResult {
    let missing = MaterialProblem::of(&MaterialStatus::Missing {
        path: ORIGINAL.to_owned(),
        target: None,
    })
    .ok_or("missing status was discarded")?;
    let unreadable = MaterialProblem::of(&MaterialStatus::Unreadable {
        path: ORIGINAL.to_owned(),
        target: None,
        reason: CAUSE.to_owned(),
    })
    .ok_or("unreadable status was discarded")?;
    for (language, missing_line, unreadable_line) in [
        (
            Language::BritishEnglish,
            "The material library /materials/fixture/library does not exist; the default material is used.",
            "The material library /materials/fixture/library cannot be read (XML: fixture fault); the default material is used.",
        ),
        (
            Language::German,
            "Die Materialbibliothek /materials/fixture/library existiert nicht; es wird das Standardmaterial verwendet.",
            "Die Materialbibliothek /materials/fixture/library ist nicht lesbar (XML: fixture fault); es wird das Standardmaterial verwendet.",
        ),
    ] {
        let texts = Texts::new(language);
        assert_eq!(
            library_note(&texts, &missing),
            missing_line,
            "unmapped missing status remains localised"
        );
        assert_eq!(
            library_note(&texts, &unreadable),
            unreadable_line,
            "unmapped unreadable cause remains localised"
        );
    }
    assert!(
        MaterialProblem::of(&MaterialStatus::Unnamed).is_none(),
        "unnamed materials have no library failure"
    );
    assert!(
        MaterialProblem::of(&MaterialStatus::Skipped).is_none(),
        "skipped material reads are not failures"
    );
    Ok(())
}
