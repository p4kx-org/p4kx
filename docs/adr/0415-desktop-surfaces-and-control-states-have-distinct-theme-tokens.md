# Desktop surfaces and control states have distinct theme tokens

The desktop uses dedicated surfaces for the canvas, panels, bars, inset
areas and tiles. Shared widgets apply the role that matches their content.
Boxes, tiles, controls and inset elements use 10, 8, 6 and 4 point radii
respectively. The panel inset keeps rectangular child content inside the
rounded outline, including in the smallest supported window.

Tables use a neutral hover surface and a distinct selection treatment.
Selection and keyboard focus are expressed through the row or control
appearance alongside the accessible widget state. A light focus ring stays
visible inside a tinted selection. A table row offers its
existing action across the row, while its checkbox and expander retain
their own targets.

The logo tile uses the dedicated brand colour. Window surface changes can
therefore use the canvas role while the logo geometry and colour continue
to follow the brand specification.

The [desktop specification](../design/desktop-spec.md), its
[machine-readable metrics](../design/p4kx-desktop-spec.json) and the
checked-in theme fixture describe the same P4KX theme. Review colours,
radii and interaction states in renders at the minimum and default
window sizes as required by ADR-0074.
