
  <h3>What it does</h3>
  <p>
    Basetherm simulates the heat flow between a building's basement (or slab on grade), the surrounding ground and the
    outdoor environment over a whole year, in three dimensions, with hourly weather data. It reports the heat loss of each room through
    its floor and walls, the annual thermal loads, the peak loss and gain, and the temperature field in the ground. Everything
    runs in your browser using resources of your computer. Simulation inputs and outputs are not uploaded to the server.
  </p>

<h3>Why use it</h3>
  <p>
Underground heat exchange is complex. Unlike the above-ground portion of a building—where heat loss and gain can be calculated as steady-state heat transfer between indoor and outdoor air—heat transfer between a basement and ambient air occurs through the thermal mass of the ground, making the problem highly transient.

In fact, ground thermal conditions evolve over many years, influenced by annual  climatic conditions, basement air temperatures, and the thermal properties of the ground domain. Because of these factors, manual calculation methods (such as those outlined in ISO or ASHRAE standards) can yield results that differ significantly from dynamic simulation methods, leading to the over- or undersizing of mechanical systems.

The timings of the peak heat loss and gain in underground spaces can be significantly shifted compared to above-ground zones, affecting the sizing of the central plant equipment. Finally, dynamic simulation allows you to evaluate different insulation strategies and estimate how heat transfer between the underground spaces with different temperatures impact the results for both spaces.
  </p>

  <h3>Methods in brief</h3>
  <ul>
    <li><b>Equation.</b> Three-dimensional heat conduction in enthalpy form, so freezing and thawing of soil moisture are included.</li>
    <li><b>Discretisation.</b> Finite volumes on a rectilinear grid. Walls, floors and insulation are included in the grid; fine cells are assigned at the ground surface, around the building and in the constructions, coarser cells further from the building. Implicit Euler scheme, solved with ADI method, so large steps are stable.</li>
    <li><b>Initial state.</b> A direct periodic solution (mean field plus annual harmonic) using conjugate gradients with multigrid, followed by a daily-step warm-up.</li>
    <li><b>Ground surface.</b> Heat balance from hourly air temperature, wind, solar radiation and long-wave exchange, simplified into a convective boundary.</li>
    <li><b>Far field.</b> Sides and bottom are coupled to a one-dimensional undisturbed-ground column with the same layers and climate. Can be switched to adiabatic.</li>
    <li><b>Soil properties.</b> From a soil type and degree of moisture saturation (Johansen's method as given by Farouki, 1981), or entered manually.</li>
    <li><b>Speed-ups.</b> Mirror symmetry is used automatically when the building and its rooms are symmetric. Multi-processor simulations are supported. Daily time steps for the warm-up period and periodic solution for the initial state.</li>
  </ul>

  <h3>Validation</h3>
  <p>The solver is checked against analytic solutions and published benchmarks:</p>
  <ul>
    <li><b>BESTEST in-ground cases</b> (the 11 annual and 6 steady ground-coupling cases): on the Standard mesh the annual totals are within about 2% of the mean of the reference programs, and within 0.2% on the Fine mesh. The steady case with an analytic solution (Delsante) is 0.7% below it. Hourly peaks agree within about 2% and the timing of the yearly peak is inside, or within a few hours of, the range of the reference programs.</li>
    <li><b>Freezing:</b> the Stefan problem (Neumann solution) within 0.4%; enthalpy is conserved to round-off.</li>
    <li><b>Mesh convergence:</b> Standard versus Fine differs by 0.5–1.9%.</li>
    <li>Quarter-domain symmetry runs agree with full-domain runs within 0.1%.</li>
  </ul>

  <h3>Main limitations</h3>
  <ul>
    <li>Building outline must be rectilinear (edges along x and y). Curved or diagonal walls are not supported.</li>
    <li>Moisture transport and groundwater flow, are not modelled.</li>
    <li>Partitions between rooms are adiabatic: rooms exchange heat only through the ground. Indoor temperatures are prescribed, not computed from a building energy balance.</li>
    <li>The ground surface has no snow cover, vegetation or shading by the building.</li>
    <li>The mesh is limited to 4 million cells.</li>
