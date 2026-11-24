# Exosuits share the vehicle catalog

Decided for #1182. The schema-v3 `/ships` dataset is the vehicle roster. It includes supported
player-controllable exosuits with `vehicleKind.value` set to `exosuit` and
`playability.value` set to `player`. The Catalog and SDK share the same admission decision
and physical Record identity.

Admission requires original evidence for the exosuit owner and its player control. When the
source supplies no direct playability flag, a curated rule may classify an individually
verified original identity. The rule retains the original selectors and their provenance.
Authored values keep the meaning of each original field and its sourced state.

The curated identity matches both the direct Actor Record path and the authored class name.
The supported ATLS owners require Actor and control components and one matching driver seat
selected by a physical Reference in the original default loadout. An explicit disabled
controller or a competing seat at the same port defeats the proof. The linked seat carries
a host attachment and a typed operator-mode Reference. The selected Record and component
locations contribute to both sourced classifications.

IKTI Akuma and IKTI Rad have their own original Actor owners and use the IKTI driver seat.
The three GEO Collector grades have individual Actor owners, each with authored control
components and a physical link to the GEO driver seat. Each insurance declaration names
the GEO as the insured entity. The original declaration is retained in the insurance
cell. Ship variant comparison follows vehicle implementation membership.
The selected attachment's `AttachDef.Localization.Name` supplies the public name when
the vehicle name is absent. The independent authored attachment manufacturer reference
keeps its physical target and sourced state.

These original inputs support the roster classification. Its scope is authored player
control. Physical aliases keep their own Record positions, and unavailable control
evidence stays visible in the read coverage and sourced values.
