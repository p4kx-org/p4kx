// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::DataForge;

use crate::LoadoutResolver;
use crate::resolver_observation::alias_fixture;

#[test]
fn worn_piece_source_preserves_the_known_physical_alias() -> Result<(), Box<dyn std::error::Error>>
{
    let bytes = alias_fixture::database();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    resolver.resolve(&forge.record(0).ok_or("first alias missing")?)?;
    let holder = resolver.resolve(&forge.record(2).ok_or("holder missing")?)?;
    let pieces = resolver.worn(&holder);
    let piece = pieces.first().ok_or("worn piece absent")?;
    let super::super::piece::Source::Wearable(wearable) = &piece.source else {
        return Err("wearable source missing".into());
    };
    assert_eq!(
        wearable.geometry().ok_or("piece geometry missing")?.path(),
        "Second.cgf",
        "worn projection keeps the known physical owner instead of a canonical GUID alias"
    );
    assert_eq!(
        wearable
            .variants()
            .first()
            .ok_or("piece variant missing")?
            .geometry()
            .path(),
        "Second-male.cgf",
        "the clothing source carries the same physical owner's body geometry"
    );
    Ok(())
}
