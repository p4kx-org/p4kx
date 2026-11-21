// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestMtl, TestMtlMaterial};

use p4kx_model::{
    CompanionError, CompanionPath, CompanionResolver, LayerCache, MaterialCompanion,
    MaterialCompanionUse, MaterialReadOrigin, MaterialReading, MaterialStatus,
};

struct Reads {
    main: Vec<u8>,
    layer: Vec<u8>,
    main_origin: MaterialReadOrigin,
    layer_origin: MaterialReadOrigin,
    layer_fetches: usize,
    uses: Vec<(MaterialCompanionUse, Option<MaterialReadOrigin>)>,
}

impl CompanionResolver for Reads {
    fn resolve(&mut self, path: CompanionPath<'_>) -> Result<Option<Vec<u8>>, CompanionError> {
        self.resolve_material_file(path).bytes
    }

    fn resolve_material_file(&mut self, path: CompanionPath<'_>) -> MaterialCompanion {
        let (bytes, origin) = match path {
            CompanionPath::Sibling("main.mtl") => (&self.main, &self.main_origin),
            CompanionPath::Material("layers/shared") => {
                self.layer_fetches += 1;
                (&self.layer, &self.layer_origin)
            }
            _ => return MaterialCompanion::unobserved(Ok(None)),
        };
        MaterialCompanion {
            bytes: Ok(Some(bytes.clone())),
            origin: Some(origin.clone()),
        }
    }

    fn observe_material(
        &mut self,
        usage: MaterialCompanionUse,
        origin: Option<&MaterialReadOrigin>,
    ) {
        self.uses.push((usage, origin.cloned()));
    }
}

#[test]
fn a_cached_layer_retains_the_same_read_and_selected_submaterial() {
    let main = TestMtl::single(
        &TestMtlMaterial::new("main")
            .layer_with(&[("Path", "layers/shared"), ("Submtl", "chosen")])
            .layer_with(&[("Path", "layers/shared"), ("Submtl", "missing")]),
    );
    let layer = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        TestMtlMaterial::new("chosen"),
    ]);
    let mut reads = Reads {
        main: main.cryxml(),
        layer: layer.cryxml(),
        main_origin: MaterialReadOrigin::default(),
        layer_origin: MaterialReadOrigin::default(),
        layer_fetches: 0,
        uses: Vec::new(),
    };
    let cache = LayerCache::new();
    for _ in 0..2 {
        let status =
            MaterialStatus::load_with(Some("main"), &mut reads, MaterialReading::Drawn, &cache);
        assert!(status.library().is_some());
        assert_eq!(
            reads.uses,
            [
                (
                    MaterialCompanionUse::Library,
                    Some(reads.main_origin.clone())
                ),
                (
                    MaterialCompanionUse::Layer {
                        material: 0,
                        layer: 0,
                        source_material: Some(1)
                    },
                    Some(reads.layer_origin.clone())
                ),
                (
                    MaterialCompanionUse::Layer {
                        material: 0,
                        layer: 1,
                        source_material: None
                    },
                    Some(reads.layer_origin.clone())
                ),
            ]
        );
        reads.uses.clear();
    }
    assert_eq!(reads.layer_fetches, 1);
}
