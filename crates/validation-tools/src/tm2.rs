//! Minimal TMY2 (.TM2) reader: dry-bulb temperature only (columns 68-71, tenths of a degree C).

/// Dry-bulb temperatures in C, one per data line (the first line is the station header).
pub fn dry_bulb(text: &str) -> Result<Vec<f64>, String> {
    text.lines()
        .skip(1)
        .filter(|l| !l.trim().is_empty())
        .enumerate()
        .map(|(i, l)| {
            let field = l
                .get(67..71)
                .ok_or_else(|| format!("line {}: too short", i + 2))?;
            field
                .trim()
                .parse::<i32>()
                .map(|v| f64::from(v) / 10.0)
                .map_err(|_| format!("line {}: bad dry-bulb field {field:?}", i + 2))
        })
        .collect()
}

pub fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(db: &str) -> String {
        // 67 filler columns, then the 4-character dry-bulb field, then flags.
        format!("{}{db}A7", "x".repeat(67))
    }

    #[test]
    fn parses_signed_tenths() {
        let text = format!("header\n{}\n{}\n", line("0028"), line("-056"));
        assert_eq!(dry_bulb(&text).unwrap(), vec![2.8, -5.6]);
    }

    #[test]
    fn reports_short_lines() {
        assert!(dry_bulb("header\nshort\n").is_err());
    }

    #[test]
    fn mean_of_values() {
        assert_eq!(mean(&[1.0, 2.0, 6.0]), 3.0);
    }
}
