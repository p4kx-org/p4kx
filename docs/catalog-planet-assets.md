# PlanetTech climate and geology asset definitions

`definitions.climateAssets` and `definitions.geologyAssets` in catalog schema v3
contain original loose JSON Entries under `Data/Libs/Planets/`. Each carries a
physical Archive identity, canonical UUID, original decoded spelling and
authored source states. Numeric fields preserve the exact JSON number token.
Ordered arrays preserve each element's individual source evidence.
An unprojected object member retains its exact JSON tokens in
`additionalFields`. If a typed array contains an invalid element, its typed
slots retain their states and the original member token remains available.

## Climate assets

The envelope keeps authored elevation, rainfall, temperature, bedrock, soil
depth, aggregate size, slope and luminosity fields, exclusion and biome masks.
Growth keeps dominance, sparsity, vitality, seasonal variation, separate
history-mode and factor fields and ordered `growthStageBuckets`.
Appearance keeps the original independent `shadingClassGUID`,
`shadingClassCloseGUID`, `shadingClassMidGUID` and `shadingClassFarGUID`.
Each field preserves explicit zero, missing and invalid source states.

The `subAssets` array retains original declaration order and the complete known
placement, surface, alignment, geometry, asset-size, opacity, fallback/promotion,
growth-bucket, shadow/collision and material data. The exact Entity link uses
`entityClass` and physical `EntityClassDefinition` candidates.
`harvestableResource` is a declared planet resource identifier.
`harvestableSetupRecord` identifies a physical `HarvestableSetup` Record by
GUID. The separate starmap EntXML binding identifies a
`SubHarvestableMultiConfigRecord`. A checked PTU source sample found the two
target declarations distinct. Missing, filtered, unreadable or wrong-family
candidates keep the authored field and do not interrupt definition projection.

## Geology assets

Geometry, placement and substrate sections retain the original declared
values, including the Boolean material/alignment flags and distinct aggregate,
bedrock and soil-depth parameters. `SunShadowMode` retains its authored name or
legacy number token in distinct variants. `Mineable_cfg_path` and
`Mineable_Material_path` remain exact Archive-relative fields. The checked PTU
source declares geometry and material paths; directory lookup follows the
original path spelling and established `Data/` root handling.

## Body assignments

The checked climate, geology, creature, resource, ecosystem and shading
families provide authored definitions and selected graph edges. A planet's
Child, HCD and EntXML documents describe existing placement routes and
separate Context Preset or setup declarations. The examined routes contain
no exact climate setup target. The catalog requires an explicit declared
body association for placement or distribution claims.
