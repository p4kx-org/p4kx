// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use p4kx_export::CatalogPublicationMode;
use p4kx_gui::{CatalogExportOptions, Language};
use p4kx_testkit::TestEntry;

use super::{TestResult, harness, makers, opened, wait_for_the_end};

#[test]
fn malformed_inventory_diagnostics_appear_in_both_catalog_outcomes() -> TestResult {
    for (language, detail) in [
        (
            Language::BritishEnglish,
            "Offline shop inventories: invalid source data (1 Archive Entry operation)",
        ),
        (
            Language::German,
            "Offline-Shop-Inventare: ungültige Quelldaten (1 Archivvorgang)",
        ),
    ] {
        for mode in [
            CatalogPublicationMode::Normal,
            CatalogPublicationMode::DiagnosticPartial,
        ] {
            let fixture = makers().entry(TestEntry::stored(
                "Data/Scripts/ShopInventories/Inv_Fixture_Orphan.json",
                b"",
            ));
            let (_dir, mut model, out) = opened(&fixture)?;
            model.set_language(language);
            let file = out.join("catalog.json");
            model.start_catalog_with_options(
                file.clone(),
                CatalogExportOptions {
                    publication_mode: mode,
                    ..CatalogExportOptions::default()
                },
                || {},
            );
            let mut harness = harness(model);
            wait_for_the_end(&mut harness)?;
            let notice = harness.get_by_label(detail);
            let mut outcome = notice.parent();
            while let Some(parent) = outcome {
                if parent.accesskit_node().role() == Role::Window {
                    break;
                }
                outcome = parent.parent();
            }
            let outcome = outcome.ok_or("the diagnostic belongs to an outcome window")?;
            assert!(
                outcome
                    .query_all_by_label_contains("Inv_Fixture_Orphan")
                    .next()
                    .is_none(),
                "the outcome does not render authored identifiers"
            );
            assert_eq!(
                file.exists(),
                mode == CatalogPublicationMode::DiagnosticPartial,
                "the shared notice does not change publication behaviour"
            );
        }
    }
    Ok(())
}
