//! Calibration tables: interpolated lookups and parsing tables pasted out of FlashPro Manager (web/src/tables.js).

use serde::{Deserialize, Serialize};

pub fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    hi.min(lo.max(x))
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Table {
    pub rows: Vec<f64>,
    pub cols: Vec<f64>,
    pub data: Vec<Vec<f64>>,
}

/// Index and blend fraction of x within an ascending (or descending) axis, clamped at the ends.
fn locate(axis: &[f64], x: f64) -> (usize, f64) {
    let n = axis.len();
    if n == 1 {
        return (0, 0.0);
    }
    let asc = axis[n - 1] >= axis[0];
    let v = if asc { x } else { -x };
    let a = |i: usize| if asc { axis[i] } else { -axis[i] };
    if v <= a(0) {
        return (0, 0.0);
    }
    if v >= a(n - 1) {
        return (n - 2, 1.0);
    }
    let mut i = 0;
    while i < n - 2 && v > a(i + 1) {
        i += 1;
    }
    (i, (v - a(i)) / (a(i + 1) - a(i)))
}

pub fn lookup1(axis: &[f64], values: &[f64], x: f64) -> f64 {
    if axis.len() == 1 {
        return values[0];
    }
    let (i, f) = locate(axis, x);
    values[i] + (values[i + 1] - values[i]) * f
}

pub fn lookup2(t: &Table, row: f64, col: f64) -> f64 {
    let (ri, rf) = if t.rows.len() > 1 { locate(&t.rows, row) } else { (0, 0.0) };
    let (ci, cf) = if t.cols.len() > 1 { locate(&t.cols, col) } else { (0, 0.0) };
    let r1 = (ri + 1).min(t.rows.len() - 1);
    let c1 = (ci + 1).min(t.cols.len() - 1);
    let d = &t.data;
    let top = d[ri][ci] + (d[ri][c1] - d[ri][ci]) * cf;
    let bot = d[r1][ci] + (d[r1][c1] - d[r1][ci]) * cf;
    top + (bot - top) * rf
}

/// Nearest breakpoint cell for a row/col value, for highlighting like a tuning UI.
pub fn active_cell(t: &Table, row: f64, col: f64) -> (usize, usize) {
    let near = |axis: &[f64], x: f64| {
        let mut best = 0;
        for (i, v) in axis.iter().enumerate() {
            if (v - x).abs() < (axis[best] - x).abs() {
                best = i;
            }
        }
        best
    };
    (near(&t.rows, row), near(&t.cols, col))
}

fn split_line(line: &str) -> Vec<String> {
    let parts: Vec<&str> = if line.contains('\t') {
        line.split('\t').collect()
    } else if line.contains(',') {
        line.split(',').collect()
    } else {
        line.split_whitespace().collect()
    };
    parts
        .iter()
        .map(|s| {
            let mut t = s.replace(['%', '°'], "");
            for unit in ["kpa", "rpm", "ms", "KPA", "kPa", "RPM", "Rpm", "MS", "Ms"] {
                t = t.replace(unit, "");
            }
            t.trim().to_string()
        })
        .collect()
}

fn is_num(s: &str) -> bool {
    !s.is_empty() && s.parse::<f64>().is_ok() && s.chars().all(|c| c.is_ascii_digit() || "+-.eE".contains(c))
}

/// Parse pasted text (axis row on top, axis column on the left) into a table.
pub fn parse_table(text: &str) -> Result<Table, String> {
    let lines: Vec<String> = text
        .replace('\r', "")
        .split('\n')
        .map(|l| if l.contains('\t') { l.trim_end().to_string() } else { l.trim().to_string() })
        .filter(|l| !l.trim().is_empty())
        .collect();
    if lines.len() < 2 {
        return Err("Paste needs at least a header row and one data row.".into());
    }
    let grid: Vec<Vec<String>> = lines.iter().map(|l| split_line(l)).collect();
    let header = &grid[0];
    let data_width = grid[1].len();
    let header_cells: Vec<&String> = if header.len() == data_width { header.iter().skip(1).collect() } else { header.iter().collect() };
    let cols: Vec<f64> = header_cells.iter().filter(|s| !s.is_empty()).map(|s| s.parse::<f64>().unwrap_or(f64::NAN)).collect();
    if cols.is_empty() || cols.iter().any(|v| v.is_nan()) {
        return Err("First row should be the column axis (e.g. MAP in kPa).".into());
    }
    let mut rows = Vec::new();
    let mut data = Vec::new();
    for (i, cells) in grid.iter().skip(1).enumerate() {
        let nums: Vec<&String> = cells.iter().filter(|s| !s.is_empty()).collect();
        if !nums.iter().all(|s| is_num(s)) {
            return Err(format!("Row {} has something that isn't a number: \"{}\"", i + 2, lines[i + 1]));
        }
        if nums.len() != cols.len() + 1 {
            return Err(format!("Row {} has {} values but the header has {} columns.", i + 2, nums.len() as i64 - 1, cols.len()));
        }
        rows.push(nums[0].parse().unwrap());
        data.push(nums[1..].iter().map(|s| s.parse().unwrap()).collect());
    }
    Ok(Table { rows, cols, data })
}

/// What a fuel table holds, from its values: lambda target, AFR target, or injector pulse width (ms).
pub fn guess_fuel_units(t: &Table) -> &'static str {
    let all = t.data.iter().flatten();
    let lo = all.clone().cloned().fold(f64::INFINITY, f64::min);
    let hi = all.cloned().fold(f64::NEG_INFINITY, f64::max);
    if lo >= 0.5 && hi <= 1.3 {
        "lambda"
    } else if lo >= 9.0 && hi <= 20.0 {
        "afr"
    } else {
        "ms"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_flashpro_paste() {
        let t = parse_table("\t30\t60\t100\n1000\t1.0\t0.95\t0.9\n2000\t1.0\t0.9\t0.85\n").unwrap();
        assert_eq!(t.cols, vec![30.0, 60.0, 100.0]);
        assert_eq!(t.rows, vec![1000.0, 2000.0]);
        assert!((lookup2(&t, 1500.0, 80.0) - 0.9).abs() < 1e-9);
        assert_eq!(guess_fuel_units(&t), "lambda");
        assert!(parse_table("x\n1\t2").is_err());
    }
}
