# Cargo loading and shop commodity defaults

Catalog v3 has two independent tables for automatic cargo handling. Each row
represents one physical source record in this build. Use `recordIndex.value`
to distinguish rows and `guid.value` for the original record identity. Both
tables follow the Record filter and share the canonical Record header.

| Catalog table and row fields | Original record type | Source value |
| --- | --- | --- |
| `configurations.cargoLoadingGlobals.rows[].autoLoadingBaseLoadingTime`, `autoLoadingBaseUnloadingTime` | `GlobalCargoLoadingParams` | Float32 base loading and unloading times |
| `configurations.cargoLoadingGlobals.rows[].cargoDeckLoadingTimePerSCU` | `GlobalCargoLoadingParams` | Float32 cargo deck time per SCU |
| `configurations.cargoLoadingGlobals.rows[].autoLoadingBoxSizeLoadingTime`, `autoLoadingBoxSizeUnloadingTime` | `GlobalCargoLoadingParams` | `SAutoLoadingBoxSizeLoadingTime` with Float32 times by box size |
| `configurations.shopCommodityGlobals.rows[].autoLoadingBasePrice` | `GlobalShopCommodityParams` | Signed Int32 base price |
| `configurations.shopCommodityGlobals.rows[].autoLoadingBoxSizePrices` | `GlobalShopCommodityParams` | `SAutoLoadingBoxSizePrices` with signed Int32 prices by box size |

The sizing blocks have a source state. When present, each contains
`value.sourceType`, `value.byScu` and `value.additionalFields`. The three
`byScu` objects use the same decimal string keys, each mapped to an authored
member by name. For example, the 1/8 SCU loading-time cell is
`autoLoadingBoxSizeLoadingTime.value.byScu["0.125"]`.

| SCU key | Original member |
| --- | --- |
| `"0.125"` | `one_eighthSCU` |
| `"0.25"` | `one_quarterSCU` |
| `"0.5"` | `one_halfSCU` |
| `"1"` | `oneSCU` |
| `"2"` | `twoSCU` |
| `"4"` | `fourSCU` |
| `"8"` | `eightSCU` |
| `"16"` | `sixteenSCU` |
| `"24"` | `twentyFourSCU` |
| `"32"` | `thirtyTwoSCU` |

Each scalar and each box-size cell has its own source state, provenance and
`containsEncryptedSource` flag. Undeclared members have state `absent`.
Float32 cells preserve exact `rawBits` and the nullable `finite` projection.
Price cells preserve the signed value. The outer rows and the sizing blocks
retain other authored members in `additionalFields`.

The DataForge declarations specify storage types and leave the time and
currency units open. Basetool's route model treats the base and per-box loading
and unloading times as seconds and the fees as aUEC. The member name
`cargoDeckLoadingTimePerSCU` identifies SCU as the denominator. Its time unit
is unknown. [Downstream route calculation decision](https://github.com/krt-profit/basetool/issues/3456#issuecomment-6899268537),
[loading-time and fee research](https://github.com/krt-profit/basetool/issues/3453#issuecomment-6898678143).

The two tables report availability independently of how many rows the filter
selects. The catalog preserves the stored timing and fee inputs. Consumers
select the applicable Record and calculate estimates for their specific cargo
operation.
