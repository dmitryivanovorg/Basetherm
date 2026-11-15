# basetherm-basement

The 3D basement model: geometry spec (footprint, levels, rooms, wall and floor layers, horizontal insulation, layered soil), mesh builder, cell classification, interior faces per room, and the full-year driver with per-room heat flows.

Heat flow sign: loss is negative, gain positive. The footprint and the rooms are rectilinear polygons (`geom.rs`); wall and floor layers can be limited (`Layer.limit`). Each room has its own interior temperature; partitions between rooms are adiabatic. Keep files under about 300 lines. SI units only.
