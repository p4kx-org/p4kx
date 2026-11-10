# Harvestable locations come from the preset in the .soc of a system container

Decided for issue #320. Evidence: LIVE 4.10, ScDataDumper `SocpakMappingGenerator`.

## Decision

- The catalog exports the mining Records as five kinds of schema version 2 (`mineableCompositions`,
  `miningGlobalParams`, `harvestableProviderPresets`, `subHarvestableMultiConfigs`,
  `harvestableLocations`). Version 2 is not released, so no bump. Facts only: numbers, GUIDs and
  Record names; the one text is the `depositName` of a composition, a game text.
- A provider preset follows each harvestable to the entity and to the composition and mining
  parameters of its `MineableParams`, so a consumer reads which rocks a place can spawn without
  the `HarvestablePreset` Records, which are not a kind.
- A location is a container below `ObjectContainers/PU/system` whose first `.soc` holds the GUID of
  a kept provider preset after the string `preset`. The `.soc` is scanned as a stream, because a
  container holds a `.soc` of over 100 MB; the scan stops at 256 MiB and a container that hits
  the limit is warned about, never taken for one without a preset. The starmap object is the
  nearest one at or above the Child element that reaches the container in the walk of the system
  containers, a mining claim marker giving way to the object above it. A marker is a starmap
  object by its `description` key `@MiningClaims_Desc_001` (all 188 `PrivateMiningPoint_*`
  Records of LIVE 4.10 and no other), a field and not the class name. A container no Child
  reaches is listed with no object.
- A `HarvestablePreset` or entity Record that is missing or cannot be built is a warning with
  count and details; the link fields of its harvestables stay `null`.
- The catalog does not pin a container to an object by hand as ScDataDumper does for four
  containers, and it does not guess a place from a name: a preset no container names (the
  surface presets of the planets) has no location; its `objectPresetPaths` name the body.

## Amendment for issue #810

Evidence: LIVE 4.10, the `ReferenceIndex` of the DataForge, the entries of the Archive.

- The 30 presets without a location stay without one. No Record of the DataForge refers to a
  provider preset, the Archive holds no planet container (no `.socpak` or `.soc` of a planet or
  moon names a preset), and the `objectPresetPaths` name a body only in the folder
  `libs/objectpresets/pu/system/<system>/<body>/` of the Hurston presets; those of Pyro and of the
  other Stanton bodies sit in `libs/objectpresets/` with no body. The folder text equals a body
  only by its spelling (`ariel` for the moon Arial), so a mapping would be a guess from a name, as
  the decision above rules out. `HPP_Pyro_AkiroCluster` is named by a container no Child reaches
  and has a location with no object.
- Each container is still opened a second time for its `.soc`. On LIVE 4.10 the pass over 196,461
  containers takes 1.6 s of a 20.8 s build, and the walk reads the XML of a container, never the
  `.soc`; reading the `.soc` in the walk would couple the starmap walk to the harvest data for
  under 8 % of the build, so it is not done.
- A location with no object says why when an object is recorded above its container: it is not
  in the starmap objects, mining claim markers form a cycle above, or a marker has no object
  above it. The warning `harvest_locations_without_object` counts them with a line per reason.
  A container with no object recorded above it, either because no Child reaches it or because it
  is reached with no object above (a system root), is the normal case and deliberately silent.

## Consequences

- 19 of 49 presets of LIVE 4.10 have a location. A later Build that names more containers in a
  `.soc` raises the number without a change of the catalog.
