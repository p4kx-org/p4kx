// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataForge;
use p4kx_dataforge::query::{Query, QueryErrorKind};

use super::{AEGIS, TestResult, guid_text, in_both_versions};

fn selected(forge: &DataForge<'_>, text: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let compiled = Query::parse(text)?.compile(forge)?;
    let run = compiled.run()?;
    Ok(run
        .indexes()
        .iter()
        .filter_map(|index| forge.record(*index)?.name().ok().map(str::to_owned))
        .collect())
}

#[test]
fn referencing_selects_the_records_that_refer_to_a_record() -> TestResult {
    in_both_versions(|forge| {
        let by_path = selected(forge, r#"referencing "libs/makers/aegis""#)?;
        assert_eq!(
            by_path,
            ["Ship.Alpha", "Ship.Beta"],
            "the referrers of Aegis by record path"
        );
        let guid = guid_text(AEGIS);
        let by_guid = selected(forge, &format!("referencing \"{guid}\""))?;
        assert_eq!(by_guid, by_path, "by GUID");
        let typed = selected(
            forge,
            r#"from Manufacturer referencing "libs/planets/terra""#,
        )?;
        assert_eq!(typed, ["Manufacturer.Aegis"], "combined with from");
        let located = selected(forge, r#"at "ships" referencing "LIBS\PLANETS\Terra""#)?;
        assert_eq!(
            located,
            ["Ship.Alpha"],
            "combined with at, ignoring case and separators"
        );
        assert!(
            selected(forge, r#"referencing "libs/ships/loose""#)?.is_empty(),
            "nobody refers to Loose"
        );
        Ok(())
    })
}

#[test]
fn referencing_an_unknown_record_is_marked() -> TestResult {
    in_both_versions(|forge| {
        let text = r#"from Ship referencing "libs/nowhere""#;
        let error = Query::parse(text)?.compile(forge).err().ok_or("compiled")?;
        assert_eq!(
            error.kind(),
            &QueryErrorKind::UnknownReferencedRecord {
                text: "libs/nowhere".to_owned()
            },
            "kind"
        );
        assert_eq!(&text[error.span().range()], r#""libs/nowhere""#, "span");
        let canonical = Query::parse(r#"REFERENCING  "a""b" select x"#)?.to_string();
        assert_eq!(
            canonical, r#"referencing "a""b" select x"#,
            "canonical form"
        );
        let out_of_order = Query::parse(r#"at "x" select a referencing "y""#)
            .err()
            .ok_or("parsed")?;
        assert!(
            out_of_order.to_string().contains("has to come before"),
            "{out_of_order}"
        );
        Ok(())
    })
}
