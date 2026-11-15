//! BESTEST steady-state floor conduction on the Standard mesh (SPEC.md pass rule).

mod common;
use basetherm_validation_tools::bestest::{run_mean, Case, Mesh};
use basetherm_validation_tools::delsante;
use common::assert_passes;

fn loss(name: &str, mesh: &Mesh) -> f64 {
    let case = Case::by_name(name).unwrap();
    -run_mean(&case, mesh, 10.0).unwrap().floor_w
}

#[test]
fn gc10a_matches_the_analytic_solution_within_one_percent() {
    let q = loss("GC10a", &Mesh::standard());
    let exact = delsante::floor_heat_flow(1.9, 30.0, 10.0, 12.0, 12.0, 0.24);
    let dev = 100.0 * (q - exact) / exact;
    println!("GC10a: {q:.1} W vs analytic {exact:.1} W ({dev:+.2}%)");
    assert!(dev.abs() < 1.0, "{dev:+.2}%");
}

#[test]
fn comparative_steady_cases_pass() {
    for name in ["GC30a", "GC30b", "GC60b", "GC65b", "GC30c"] {
        assert_passes(
            name,
            "floor_conduction_steady",
            loss(name, &Mesh::standard()),
        );
    }
}

#[test]
#[ignore = "slow (about a minute): Standard versus Fine mesh"]
fn standard_mesh_is_within_three_percent_of_fine() {
    for name in ["GC10a", "GC30a", "GC30b", "GC60b", "GC65b", "GC30c"] {
        let (s, f) = (loss(name, &Mesh::standard()), loss(name, &Mesh::fine()));
        let dev = 100.0 * (s - f) / f;
        println!("{name}: standard {s:.1}, fine {f:.1} ({dev:+.2}%)");
        assert!(dev.abs() < 3.0, "{name}: {dev:+.2}%");
    }
}
