### Fixed

- Model export and the Blender add-on apply explicitly selected HardSurface palette finish to
  sampled smoothness together with the authored layer multiplier. Layered GLB roughness retains
  the contributing material, palette and texture origins; alpha minification preserves the
  scalar smoothness channel without changing RGB or normal processing.
- Selected HardSurface finishes read explicit Gloss from red independently of DDNA alpha;
  the generic unselected RGB-Gloss convention stays unchanged.
