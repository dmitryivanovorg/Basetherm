# Solver versus ISO 13370 and ASHRAE-style hand methods, Surgut

Test: `crates/wasm-api/tests/iso_ashrae.rs` (ignored, about 6 minutes in release; command in the file header).
Climate: `refs/epw/surgut/RUS_KHM_Surgut.AP.238490_TMYx.2011-2025.epw` (climate.onebuilding.org, TMYx 2011-2025;
annual mean air -0.25 C, monthly means -19.6 (Jan) to +18.2 C (Jul), amplitude 18.9 K).

Building: interior 12 x 8 m, basement 2.4 m deep or slab on grade, concrete wall 0.25 m and floor 0.15 m (k 1.4),
films 8 and 6 W/(m2 K), insulation 100 mm (k 0.035) on the ground side of the walls and under the floor, interior 20 C
(rooms: 20/20 or 20/12 C, split at x = 6 m). Soil one layer, k 1.6 W/(m K), cv 2.8 MJ/(m3 K); with freezing also
k frozen 2.0, cv frozen 2.1, latent 98 MJ/m3 (0 C). Draft mesh, margin 15 m, 15 m below the floor, convergence 0.1 %
of the peak hourly loss. All numbers below are heat loss in W (positive here).

## Reference methods and their limits

- **ISO 13370, annual mean part**: `B' = A/(0.5 P)` on external dimensions; slab and basement floor U-values and
  basement wall U-value from the standard; `Phi = (A U_bf + z P U_bw)(theta_i - theta_e,mean)`. Annual mean air
  temperature is the only climate input; no freezing, no snow, no edge-insulation corrections. The periodic (monthly)
  part of the standard is **not** compared: its formulas could not be checked against the standard text here.
- **ASHRAE-style path-length method**: heat leaves a wall point at depth x, or a floor point at offset y from the wall,
  along a quarter circle of length `pi/2 r` in soil (Latta and Boileau concept). It was implemented from the concept,
  **not** from the ASHRAE Handbook tables (not available); treat it as an ASHRAE-type estimate, not as the Handbook's
  numbers. Used twice: with the annual mean air temperature (compare with the annual mean loss), and with the
  ASHRAE winter design ground temperature, annual mean minus amplitude = -19.2 C (compare with the coldest monthly mean).

## Results

| Case                                       | Solver, annual mean | ISO  | Path | Solver/ISO | Solver, coldest month | Design (path, -19.2 C) | Solver/design | Solver peak hour |
| ------------------------------------------ | ------------------- | ---- | ---- | ---------- | --------------------- | ---------------------- | ------------- | ---------------- |
| Basement, no insulation, no freezing       | 2660                | 2378 | 2372 | 1.12       | 4253                  | 4587                   | 0.93          | 4573             |
| Basement, no insulation, freezing          | 2879                | 2378 | 2372 | 1.21       | 4420                  | 4587                   | 0.96          | 4710             |
| Basement, insulated, no freezing           | 993                 | 784  | 770  | 1.27       | 1384                  | 1489                   | 0.93          | 1426             |
| Basement, insulated, freezing              | 1072                | 784  | 770  | 1.37       | 1351                  | 1489                   | 0.91          | 1411             |
| Slab, no insulation, no freezing           | 1340                | 1193 | 1351 | 1.12       | 2109                  | 2614                   | 0.81          | 2291             |
| Slab, insulated, freezing                  | 798                 | 411  | 402  | 1.94       | 1260                  | 778                    | 1.62          | 1530             |
| Two rooms 20/20, basement, plain           | 2660                | 2378 | 2372 | 1.12       | 4253                  | 4587                   | 0.93          | 4573             |
| Two rooms 20/12, basement, plain           | 2095                | 1908 | 1903 | 1.10       | 3689                  | 4118                   | 0.90          | 4010             |
| Two rooms 20/12, basement, insulated       | 783                 | 629  | 618  | 1.24       | 1174                  | 1337                   | 0.88          | 1216             |
| Two rooms 20/12, basement, plain, freezing | 2311                | 1908 | 1903 | 1.21       | 3804                  | 4118                   | 0.92          | 4152             |

Floor and wall split, one room, plain basement: solver floor 715 W and walls 1945 W, ISO 784 and 1594 W. Insulated:
solver 392 and 601 W, ISO 355 and 429 W.

Rooms at different temperatures (solver room loss against ISO applied to each room alone, with the whole building's
`B'`, so no exchange through the ground):

| Case | Warm room (20 C) solver / alone | Cool room (12 C) solver / alone |
|---|---|---|
| plain | 1443 / 1189 W (1.21) | 653 / 719 W (0.91) |
| insulated | 557 / 392 W (1.42) | 225 / 237 W (0.95) |
| plain, freezing | 1552 / 1189 W (1.31) | 759 / 719 W (1.06) |

## What the numbers say

- **Same temperature in both rooms:** the split building gives exactly the one-room result (2660 W), as it should
  (partitions are adiabatic, the ground sees one floor).
- **Different temperatures:** the whole-building loss follows the area-mean temperature (solver/ISO 1.10 against 1.12
  at equal temperatures), so the total is nearly linear. What changes is the distribution: the warm room loses 21-42 %
  more than it would alone, and the cool room 5-9 % less (no freezing), because heat flows sideways in the ground from
  under the warm room to under the cool one. The effect is largest for the insulated building, where the floor loss
  to the ground below is small and the lateral path matters more.
- **No insulation, no freezing:** the solver is 10-12 % above ISO 13370 on the annual mean, and 7 % below the
  ASHRAE-type winter design value in the coldest month (the design method is conservative by construction).
- **Freezing** adds about 8 % to the annual loss, plain or insulated. In the
  coldest month the frozen case is 4 % higher (plain) or 2 % lower (insulated) than the thawed one: frozen soil conducts
  better but its latent heat and the colder, longer-lasting frost front work against it. ISO and the path method have no
  freezing, so their numbers are the same for both.
- **Insulation** cuts the annual loss by 63 % (thawed) in the solver and by 67 % in ISO, so the solver gains less from
  insulation: its insulated cases sit 24-37 % above ISO.
- **Largest disagreement: insulated slab on grade, 1.94 times ISO** (1.62 times the design value). The model has an
  uninsulated slab edge at the ground surface and no snow cover; ISO has no edge term unless edge insulation is given. We
  did not isolate the cause (candidates: the slab edge, the missing snow cover in a -19 C month, the corner of the
  insulation). Run an edge-insulated slab and a version with the ground surface held at the air temperature before
  relying on this case.

## Caveats

- The solver's ground surface has solar gain and long-wave exchange and **no snow cover**; Surgut has months of snow
  that insulates the ground. ISO and the path method use the air temperature. Part of the differences is therefore
  a difference in the model of the ground surface, not an error of either method. (Snow is planned for v1.1.)
- Draft mesh only; the Standard mesh differs by 0.5-1.9 % on the BESTEST cases, and these cases were not rerun on it.
- No pass tolerance was set for these comparisons (the hand methods are not accepted references with a stated error).
  The BESTEST suite with the 5 % rule remains the external validation of the numerical method.
