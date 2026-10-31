### Fixed

- UI screens: the constant of an integer comparison in a screen (`seatdashboard/currentmode`
  equal to 4, say) was read as zero, so the powered MFD drew every mode of its self-status page at
  once, with two large light-blue blocks at the lower corners. A runtime image such as the ship
  diagram no longer fills its box with a colour, a notification of the power page is no longer
  drawn at rest, and the arrows of the tab bar take their colour.
