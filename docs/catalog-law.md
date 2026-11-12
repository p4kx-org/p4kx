# Catalog law definitions

The catalog v3 tables below contain physical DataForge definitions. Each carries the
[canonical Record header](catalog-record-headers.md). Selection uses the exact Record type at
any path. Rows are ordered by GUID and physical Record index. Table availability follows the
source declaration, while completeness and export health account for filtering and source
failures.

| Catalog table | Original Record type | Direct fields |
| --- | --- | --- |
| `definitions.infractions` | `InfractionDefinition` | `name` and `description` are Locale. `triggers` is an ordered Enum array of `EInfractionTrigger`. `defaultParameters` is an owned `InfractionParameters` Class. |
| `definitions.infractionSets` | `InfractionSet` | `infractions` is an ordered Class array of `Infraction`. |
| `definitions.jurisdictions` | `Jurisdiction` | The original scope label, name, asset path, law and prison settings, and physical and owned links described below. |

Each `Infraction` use contains one `definition` Reference to `InfractionDefinition` and one
owned `parameterOverrides` Class of `InfractionParameters`. The same infraction GUID can
occur more than once with different parameters. Defaults live on the definition, while each
override stays at its original position in a set or jurisdiction. The parameter Class has no
independent Record identity.

| `InfractionParameters` original fields | Original storage and unit evidence |
| --- | --- |
| `isFelony`, `graceAllowance`, `displayGraceTime`, `felonyMerits`, `ignoreIfAgainstPartyMember`, `hideCrimeNotification`, `hideCrimeInJournal` | Int32. The original signed integers include zero. |
| `graceAllowanceCooldown`, `gracePeriod`, `graceCooloffScale`, `earlyPaymentPeriod`, `lifetime`, `coolOffTime`, `pressChargesNotificationTime` | Single. The declarations supply the original bits and no unit metadata. |
| `escalatedPaymentFineMultiplier` | Single multiplier. |
| `removeTimeSeconds` | Single in seconds. |
| `graceWarnings` | Ordered Locale array. Each element has its own original source receipt. |

| `Jurisdiction` original fields | Original storage and unit evidence |
| --- | --- |
| `subsumptionJurisdiction`, `logoPath` | String scope label and asset path. |
| `name` | Locale. |
| `parentJurisdiction`, `journalEntry` | One Reference each to `Jurisdiction` and `JournalEntry`, respectively. |
| `respectsParentJurisdictionLaws`, `isPrison` | Bool. |
| `infractions`, `controlledSubstanceClasses`, `impoundingDefinitions` | Ordered Class arrays of `Infraction`, `ControlledSubstanceClass`, and `ImpoundingDefinition`, respectively. |
| `infractionSets`, `prohibitedGoods`, `prohibitedResources` | Ordered Reference arrays to `InfractionSet`, `EntityClassDefinition`, and `ResourceType`, respectively. |
| `maxStolenGoodsPossessionSCU` | Single in SCU, exported as `maxStolenGoodsPossessionScu`. |
| `baseFine`, `earlyPaymentPeriod` | Int32 and Single, respectively. The declarations supply the authored magnitudes without unit metadata. |

A `ControlledSubstanceClass` preserves the ordered `commodities` Reference array to
`EntityClassDefinition`, the ordered `resources` Reference array to `ResourceType`, and the
Single `maxPossessionSCU` in SCU, exported as `maxPossessionScu`.

An `ImpoundingDefinition` preserves `name` (Locale), `trigger` (Enum of `EImpoundingTrigger`),
`impoundingTimeSeconds` (Single in seconds), `impoundingFineUEC` (Int32 in UEC, exported as
`impoundingFineUec`), and `ignoreIfAgainstPartyMember` (Bool).

Original enum option labels and their order are retained. Single values keep their exact
IEEE-754 bits, including signed zero and nonfinite values. Unknown original fields and owned
shapes use the owner-local `authoredFields` arena. The source declarations above contain no
separately typed condition tree. An authored condition found in an additional member stays
in the arena with its original structure, order and evidence.

Each typed cell, array element, Reference identity and retained source node carries its own
source evidence. Non-present states distinguish null, absent, unreadable, invalid and
unsupported data. The original Reference GUID survives a missing or incompatible target.
Filter exclusions are reported separately in export health. Parent links can be cyclic and
remain direct links. Navigation and other Record references join the same physical
`definitions.jurisdictions` rows by GUID and source identity. The separate Faction definition
holds Faction legal authority.

`description` and `graceWarnings` reference `texts.gameTexts` and carry `x-gameText`.
The short localized names reference `texts.publicNames`. Text protection and source encryption
are independent.

These tables describe authored game data. The consumer sees each configured occurrence and
link directly, with no computed enforcement, inherited default, active jurisdiction,
player criminality or runtime penalty.
