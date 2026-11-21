// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{ArchiveModels, Armour, AssemblyOptions, BodyVariant, LoadoutResolver};
use p4kx_gltf::GlbOptions;

use crate::media_fixtures::{self, Check, SourceCheck};

const ARMOUR_LOADOUT: &str =
    "Scripts/Loadouts/Character/Human/Armor/NPC/Criminal/Pyro/Criminal_Gang_Guard_Heavy_Boss.xml";

pub(super) fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let models = ArchiveModels::new(archive);
        let mut resolver = LoadoutResolver::new(forge).with_files(files);
        let tree = resolver.resolve_file(ARMOUR_LOADOUT)?;
        let pieces = resolver.worn(&tree);
        for variant in BodyVariant::ALL {
            let armour = Armour::build(variant, &pieces, &models, &AssemblyOptions::default())?;
            let mut bytes = Vec::new();
            let summary = armour.write_glb(&GlbOptions::default(), &mut bytes)?;
            p4kx_testkit::check_glb(&bytes)?;
            let skipped = armour
                .issues()
                .iter()
                .filter(|issue| issue.kind().skips())
                .count();
            let has_box = armour
                .parts()
                .iter()
                .any(|part| super::measurement::is_box(part.model(), 0));
            media_fixtures::require(skipped * 5 <= armour.parts().len(), Check::SkippedShare)?;
            media_fixtures::require(!has_box, Check::BoxShare)?;
            media_fixtures::require(
                summary.triangles > 10_000 && summary.bones > 100,
                SourceCheck::ModelPresence,
            )?;
        }
        Ok(())
    })
}
