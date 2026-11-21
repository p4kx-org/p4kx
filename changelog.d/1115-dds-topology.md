### Fixed

- DDS arrays and volumes retain readable source topology beyond fixed device count limits.
  Aggregate image counts and image-plan indices remain exact without 32-bit saturation, and
  direct image-plan tail access avoids constructing preceding images.
