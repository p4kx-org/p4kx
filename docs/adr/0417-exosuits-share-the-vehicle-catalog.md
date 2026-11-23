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
The four ATLS variants require Actor and control components and one matching driver seat
selected by a physical Reference in the original default loadout. An explicit disabled
controller or a competing seat at the same port defeats the proof. The linked seat carries
a host attachment and a typed operator-mode Reference. The selected Record and component
locations contribute to both sourced classifications.

These original inputs support the roster classification. Its scope is authored player
control. Physical aliases keep their own Record positions, and unavailable control
evidence stays visible in the read coverage and sourced values.
