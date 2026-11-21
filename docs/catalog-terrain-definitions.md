# Planet terrain definitions

The schema-v3 `definitions` object exposes six physical loose Archive Entry families. Each table reports `availability` independently of selected row count. The ordinary catalog filter selects physical Entry paths. Read and projection failures appear in export health and leave readable siblings available for diagnostic publication.

| Table | Entry grammar | Authored data |
| --- | --- | --- |
| `geologySets` | `Data/Libs/Planets/geology_sets/**/*.geology` | Scattered and cliff asset GUID arrays, independent size probabilities and asset counts, cliff probability and texture-set GUID. |
| `geologyTextureSets` | `Data/Libs/Planets/geology_sets/**/*.geotextureset` | Ground and biplanar layers, roughness, aggregate bleed, substrate spread, scree and gradient parameters. |
| `soils` | `Data/Libs/Planets/soil_sets/**/*.soil` | Texture-set GUID, beach, stagnant-water, trash and acid flags, and ocean-secondary-colour weight. |
| `soilTextureSets` | `Data/Libs/Planets/soil_sets/**/*.soiltextureset` | Ground and medium layers, independent medium weights and height scales, sand, scree and roughness. |
| `shadingClasses` | `Data/Libs/Planets/shading_classes/**/*.shadingclass` | Ground layer, original lighting-model code, roughness, ambient occlusion, UV scale, view-angle masking, normal strength and parallax height. |
| `ecosystems` | `Data/Libs/EcoSystems/**/*.eco` | Protected original label, audio trigger, eccentricity, audio array counts and all five authored texture or mask paths. |

Every physical row retains original Entry identity, canonical GUID and the authored GUID spelling. The five terrain set and class formats have no separate name in the verified sample. The protected ecosystem name retains its source spelling and does not use the localisation registry. Every known nested field and array occurrence carries the source-use references and immediate encryption marker of its owning original JSON token. Additional original fields retain sourced names and exact tokens in `additionalFields`. Numeric JSON source values preserve the complete original number spelling, including exponents and trailing zeros. Known integral tokens use the original integer spelling without defining units or assigning enum text.

A ground layer retains `textureName`, `surfaceTypeName`, `surfaceGroupName`, `gloss` and `bedrock` from `bIsBedrock`. Biplanar and medium layers share the texture-only projection. Arrays retain their original order. Medium layers, `mediumLayerWeights` and `mediumLayerHeightScales` preserve independently authored positions and lengths. A populated ecosystem audio array keeps its original sourced count and receives an `unsupportedValue` diagnostic until its payload shape has evidence. The documented fields are `audioRTPCArray`, `audioSwitchArray`, `audioTrigger`, `fAudioTerrainEccentricity`, `ecoSystemClimateTexture`, `ecoSystemDataTexture`, `ecoSystemNormalTexture`, `elevationTexture` and `ObjectPresetSpawnMask`.

## Declared references

A geology set's `links` preserves separate original slots for `scatteredAssets` and `cliffAssets`, and independently resolves `geologyTextureSetGUID` to `geologyTextureSets`. A soil definition links its authored `soilTextureSetGUID` to `soilTextureSets`. Climate asset `appearance.shadingLinks` keeps `default`, `close`, `mid` and `far` independent. They resolve the original `shadingClassGUID`, `shadingClassCloseGUID`, `shadingClassMidGUID` and `shadingClassFarGUID` to `shadingClasses`. Each physical target candidate retains its declared GUID and Entry evidence, even when the target has been filtered out. The source selector and target fields carry independent origins.

A target lookup reports nil, malformed, missing, unavailable, wrong-family, multiple-candidate and filtered states separately. Wrong-family identification is bounded to the actual surveyed terrain definition families. A failed candidate-family identity read makes the relationship unavailable to classify uniquely. Original evidence from other observed candidates remains visible. The relationship origin flags uncertainty when its negative payload evidence cannot be retained at that source boundary.

Texture and mask links use exact Archive-directory lookup and the permitted original `Data/` spelling. The stored .tif, .dds, .r16 or .r8 path remains intact. Directory evidence never guesses another extension or reads texture payload bytes. Layer texture-link ordinals identify the original slots. This graph describes authored definitions and selectors. It has no proved per-planet distribution binding.
