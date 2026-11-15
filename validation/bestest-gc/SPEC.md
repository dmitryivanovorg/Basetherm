# BESTEST ground-coupling validation spec (draft)

Source: IEA BESTEST ground-coupling cases, NREL/TP-550-43388 (Neymark & Judkoff, 2008). Original files are in `refs/bestest-gc/` (not part of the build). Reference numbers are in `reference.csv`, transcribed from the report's Part III summary tables (pp. 220–223). This file is the contract for the tests; change it only by agreement.

## Reference and pass rule

- **Reference = the three verified numerical models only** (TRNSYS/TESS, FLUENT/PAAET, MATLAB/DIT). Their results agree within 0.3–4.2%. The envelopes of the other programs (EnergyPlus, ISO 13370, BASESIMP and others) span 7–24% and are not used.
- **Pass (5% rule):** the result is within 5% of the mean of the three, **or** inside their min–max envelope. `reference.csv` gives the combined pass band.
- **Phase shift of the peak:** pass inside the envelope ±24 h.
- **GC10a:** compare with the analytic value (2433 W). Tolerance 1% (agreed; the verified models are within 0.3%).
- Sign: the report gives losses as positive. Our convention is loss negative, so tests flip the sign.

## What the cases are

Idealised uninsulated slab on grade, constant properties (k = 1.9 W/mK, ρ = 1490 kg/m³, c = 1800 J/kgK; slab and foundation wall = soil), no radiation, no freezing, no solar. Ti = 30 °C, mean outdoor 10 °C. Walls are adiabatic and touch but do not penetrate the ground. A 0.24 m wide ring at the slab perimeter is adiabatic (ground surface under the wall).

| Case | Type | Slab (m) | h_int / h_ext (W/m²K) | Depth E / far field F (m) | k | Weather |
|---|---|---|---|---|---|---|
| GC10a | steady, analytic | 12×12 | Dirichlet Ti / To, linear ramp over the 0.24 m ring | infinite (verified models used 40/40) | 1.9 | none |
| GC30a | steady | 12×12 | Dirichlet | 30 / 20 | 1.9 | GCSS-W40 |
| GC30b | steady | 12×12 | 100 / 100 | 15 / 15 | 1.9 | GCSS-W20 |
| GC60b | steady | 12×12 | 7.95 / 100 | 15 / 15 | 1.9 | GCSS-W20 |
| GC65b | steady | 12×12 | 7.95 / 11.95 | 15 / 15 | 1.9 | GCSS-W01 |
| GC40a | annual | 12×12 | Dirichlet, exterior = hourly To,a | 30 / 20 | 1.9 | GCSP-W40 |
| GC40b | annual | 12×12 | 100 / 100 | 15 / 15 | 1.9 | GCSP-W20 |
| GC45b | annual | 36×4 | 100 / 100 | 15 / 15 | 1.9 | GCSP-W20 |
| GC50b | annual | 80×80 | 100 / 100 | 15 / 15 | 1.9 | GCSP-W20 |
| GC55b | annual | 12×12 | 100 / 100 | 2 / 15 | 1.9 | GCSP-W20 |
| GC70b | annual | 12×12 | 7.95 / 11.95 | 15 / 15 | 1.9 | GCSP-W01 |
| GC80b | annual | 12×12 | 100 / 100 | 15 / 15 | 0.5 | GCSP-W20 |
| GC30c, 40c, 45c, 55c, 80c | included (BASESIMP-style boundaries) | as b | 7.95 / direct Ta | 15 (5 for 55c) / 8 | 1.9 (0.85 for 80c) | as b |

- Deep boundary: fixed 10 °C at depth E. Far-field vertical planes: adiabatic at F from the slab edge.
- Outdoor temperature: hourly dry-bulb from the TM2 file. The annual cases follow `To,a = Tday − 2·cos((hour−4)/24·2π)`, `Tday = 10 − 6·cos((day−15)/365·2π)`, range 2–18 °C. Hour 1 = 0:00–1:00; 365-day year.
- Output: floor conduction through the interior slab surface (B×L). Steady cases give the last hour (W). Annual cases give the last full year total (kWh), the peak hour (W) and its time, and the phase shift from the coldest hour.
- Run criterion: year-to-year change of annual conduction ≤ 0.1% (the spec's value). Mesh refinement should change results by ≤ 0.5%.
- Initial condition for the b-cases: the whole domain at 10 °C, interior stepped to 30 °C. Run to steady or steady-periodic state.

## Solver requirements this implies (approved)

These are test configurations, not changes to the production physics:
1. **Selectable boundary conditions.** Dirichlet surfaces. Robin surface with constant `h` and a time series ambient temperature, and no solar or long-wave. Adiabatic far field and fixed-temperature bottom at depth E, as alternatives to the production 1D-coupled far field. Adiabatic ring at the wall footprint. Linear temperature ramp over the ring (GC10a only).
2. **Slab as soil-property cells.** The slab is one thin layer of the same material as the soil.
3. **Direct periodic solve.** The harmonic cases are linear and constant-property, so they also serve as the consistency test between the direct annual-harmonic solve and the time-stepped state. The direct solve covers only the annual component; the diurnal ±2 °C part comes from the hourly year run.
4. **Test tooling, not in solver crates:** a TM2 reader (dry-bulb only), and the Delsante et al. (1983) analytic function (report pp. 24–25; the PDF text is garbled, so implement from the formula image and confirm that the 12×12 case gives 2433 W).

## Gaps BESTEST does not cover

Freezing (minimum temperature is 2 °C), solar and long-wave surface exchange, multilayer walls, horizontal insulation, and basement walls. These stay with the analytic half-space and Stefan tests, an energy-closure test for the surface balance, and 1D/2D/3D and symmetry-reduced consistency tests.

## Open points

- The phase-shift table is titled "Time from Coldest Hour (Jan 15, Hour 4)", but the minimum temperature first occurs on 8 Jan, hour 4. Settle the reference time against the README (`refs/bestest-gc/data/README-GC-InDepth.doc`) before testing.
- The harmonic reference values are for ≤0.1% steady-periodic convergence; verified models ran 3–10 years.
