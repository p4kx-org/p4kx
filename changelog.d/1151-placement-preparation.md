### Changed

- Release parsed world placement data after its last use and reuse the first linked row buffers while preparing the catalog.
- Stage container and location instance rows during the world walk so subsequent projections do not retain either full table in memory.
