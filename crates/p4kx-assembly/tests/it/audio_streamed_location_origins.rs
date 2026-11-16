// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, InputScope, SourceLocation, SourceUses};

use super::{TestResult, german, with_audio};

#[test]
fn loose_location_keeps_the_selected_entry_and_the_original_language_spelling() -> TestResult {
    with_audio(|audio| {
        let sounds = audio
            .resolve_trigger_with_path_sources("Play_Line", &german())
            .ok_or("synthetic trigger")?;
        let variant = sounds.variants().first().ok_or("streamed sound")?;
        assert_eq!(
            variant.language(),
            Some("German"),
            "canonical bank spelling"
        );
        let uses = SourceUses::with_compact_use_refs();
        let origins = variant.wem_location_origins(audio.archive(), &uses)?;
        assert!(
            origins.language.source_set_complete(),
            "bound spelling proof"
        );
        assert!(origins.offset.is_none(), "a loose WEM has no DIDX offset");
        let inventory = uses.freeze()?;
        let mut paths = Vec::new();
        for site in inventory
            .iter_uses()
            .filter(|site| origins.language.source_uses_refs().contains(site.id()))
        {
            assert_eq!(
                site.scope(),
                InputScope::ArchiveDirectoryMetadata,
                "location metadata does not read a WEM payload"
            );
            assert!(
                matches!(site.location(), SourceLocation::Metadata { field } if field == "path"),
                "language folders are read from their original directory paths"
            );
            let source = inventory
                .iter_sources()
                .find(|source| source.id() == site.source())
                .ok_or("physical path source")?;
            let wire = serde_json::to_value(source)?;
            let path = wire
                .pointer("/path/value")
                .and_then(serde_json::Value::as_str);
            paths.push(path.ok_or("known source path")?.to_owned());
        }
        paths.sort_unstable();
        assert_eq!(
            paths,
            [
                "Data\\Sounds\\wwise\\German\\Voice.bnk",
                "data\\sounds\\WWISE\\media\\german\\9000.WEM",
            ],
            "the source of spelling differs from the selected media Entry"
        );
        let foreign = Archive::open(audio.archive().path())?;
        let foreign_origins = variant.wem_location_origins(&foreign, &SourceUses::new())?;
        assert!(
            !foreign_origins.kind.source_set_complete(),
            "another Archive view cannot claim the retained binding"
        );
        Ok(())
    })
}
