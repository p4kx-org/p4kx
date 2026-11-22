# Exosuits share the vehicle catalog

Decided for #1182. The schema-v3 `/ships` dataset is the vehicle roster. It includes supported
player-controllable exosuits with `vehicleKind.value` set to `exosuit` and
`playability.value` set to `player`. The Catalog and SDK share the same admission decision
and physical Record identity.

Admission requires original evidence for the exosuit owner and its player control. When the
source supplies no direct playability flag, a curated rule may classify an individually
verified original identity. The rule retains the original selectors and their provenance.
Authored values keep the meaning of each original field and its sourced state.
