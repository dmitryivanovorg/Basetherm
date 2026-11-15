//! Compares preconditioners on the real BESTEST systems (iterations and time).
//!   cargo test --release -p basetherm-validation-tools --test mg_explore -- --ignored --nocapture

use basetherm_bc::Side;
use basetherm_mg::{Coarsen, Settings, Smoother, C64};
use basetherm_periodic::{solve, Amplitudes, Params, PrecondKind};
use basetherm_validation_tools::bestest::{Case, Mesh, Model};
use std::time::Instant;

fn configs() -> Vec<(&'static str, PrecondKind)> {
    let mg = |coarsen, smoother| {
        PrecondKind::Multigrid(Settings {
            coarsen,
            smoother,
            coarse_limit: 400,
        })
    };
    vec![
        ("z-line      ", PrecondKind::ZLine),
        ("MG all+z    ", mg(Coarsen::All, Smoother::Z)),
        ("MG all+alt  ", mg(Coarsen::All, Smoother::Alternating)),
        ("MG horiz+z  ", mg(Coarsen::Horizontal, Smoother::Z)),
        (
            "MG horiz+alt",
            mg(Coarsen::Horizontal, Smoother::Alternating),
        ),
    ]
}

#[test]
#[ignore]
fn compare_preconditioners() {
    for (name, mesh_name, mesh) in [
        ("GC30b", "std", Mesh::standard()),
        ("GC30b", "fine", Mesh::fine()),
    ] {
        let case = Case::by_name(name).unwrap();
        let model = Model::build(&case, &mesh, 10.0).unwrap();
        let mut forcing = Amplitudes::zeros(&model.grid);
        for &f in &model.exterior_faces {
            forcing.set(Side::ZLo, f, C64::new(5.0, -3.0));
        }
        println!("{name} {mesh_name}: {} cells", model.grid.cell_count());
        let mut reference: Option<f64> = None;
        for (label, kind) in configs() {
            let params = Params {
                precond: kind,
                ..Params::annual()
            };
            let t = Instant::now();
            let sol = solve(
                &model.grid,
                &model.k,
                &model.cv,
                &model.bc,
                &forcing,
                params,
            )
            .unwrap();
            let secs = t.elapsed().as_secs_f64();
            let (mean, _) = sol.boundary_flux(Side::ZLo);
            let q: f64 = model.slab_faces.iter().map(|&f| mean[f]).sum();
            let drift = reference.map_or(0.0, |r| (q - r).abs() / r.abs());
            reference.get_or_insert(q);
            println!(
                "  {label}: mean {:>4} its, harmonic {:>4} its, {secs:>6.1} s (flux differs from z-line by {drift:.1e})",
                sol.mean_outcome.iterations, sol.harmonic_outcome.iterations
            );
        }
    }
}
