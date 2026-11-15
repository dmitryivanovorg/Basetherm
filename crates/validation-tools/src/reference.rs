//! BESTEST reference values (verified numerical models) from `validation/bestest-gc/reference.csv`.

const CSV: &str = include_str!("../../../validation/bestest-gc/reference.csv");

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub case: String,
    pub quantity: String,
    pub unit: String,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
    /// Combined pass band (5% of mean, or the models' envelope, whichever is wider);
    /// `None` for the analytic row.
    pub pass: Option<(f64, f64)>,
}

pub fn rows() -> Vec<Row> {
    CSV.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            Row {
                case: c[0].to_string(),
                quantity: c[1].to_string(),
                unit: c[2].to_string(),
                mean: c[6].parse().expect("mean"),
                min: c[7].parse().expect("min"),
                max: c[8].parse().expect("max"),
                pass: c[9].parse().ok().zip(c[10].parse().ok()),
            }
        })
        .collect()
}

pub fn lookup(case: &str, quantity: &str) -> Option<Row> {
    rows()
        .into_iter()
        .find(|r| r.case == case && r.quantity == quantity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_all_rows() {
        assert_eq!(rows().len(), 39);
    }

    #[test]
    fn looks_up_by_case_and_quantity() {
        let r = lookup("GC30b", "floor_conduction_steady").unwrap();
        assert!((r.mean - 2535.7).abs() < 0.05);
        let (lo, hi) = r.pass.unwrap();
        assert!(lo < r.min && hi > r.max);
        assert!(lookup("GC10a", "floor_conduction_steady_analytic")
            .unwrap()
            .pass
            .is_none());
        assert!(lookup("GC99", "x").is_none());
    }
}
