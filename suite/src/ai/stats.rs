//! The data analyser: statistics the suite computes itself from a table (a CSV or TSV file, or a
//! GridCraft sheet). Only these numbers go to the AI, which explains them and never calculates.

/// A table of text cells: a header row and data rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl Table {
    /// The first row is the header row; columns without a name get "Column N".
    pub fn from_rows(mut rows: Vec<Vec<String>>) -> Table {
        rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
        if rows.is_empty() {
            return Table::default();
        }
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        let mut headers = rows.remove(0);
        headers.resize(width, String::new());
        for (i, h) in headers.iter_mut().enumerate() {
            *h = h.trim().to_string();
            if h.is_empty() {
                *h = format!("Column {}", i + 1);
            }
        }
        for r in &mut rows {
            r.resize(width, String::new());
        }
        Table { headers, rows }
    }
}

/// Splits CSV (or, with `delim` '\t', TSV) text into rows: quoted fields may hold the delimiter,
/// line breaks and doubled quotes.
pub fn parse_delimited(text: &str, delim: char) -> Vec<Vec<String>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.is_empty() {
            quoted = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// The delimiter a file most likely uses: tab for .tsv/.tab, else whichever of tab, semicolon
/// and comma the first line has most of.
pub fn guess_delimiter(name: &str, text: &str) -> char {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tsv") || lower.ends_with(".tab") {
        return '\t';
    }
    let first = text.lines().next().unwrap_or("");
    let count = |c: char| first.matches(c).count();
    [('\t', count('\t')), (';', count(';')), (',', count(','))].into_iter().max_by_key(|(_, n)| *n).filter(|(_, n)| *n > 0).map(|(c, _)| c).unwrap_or(',')
}

const MISSING: [&str; 9] = ["", "na", "n/a", "nan", "null", "none", "-", ".", "#n/a"];

pub fn is_missing(cell: &str) -> bool {
    MISSING.contains(&cell.trim().to_ascii_lowercase().as_str())
}

/// A number in a cell: plain, with thousands separators (1,234.5), or a percentage (12.5%).
pub fn number(cell: &str) -> Option<f64> {
    let t = cell.trim();
    if let Ok(v) = t.parse::<f64>() {
        return v.is_finite().then_some(v);
    }
    let (t, scale) = match t.strip_suffix('%') {
        Some(rest) => (rest.trim(), 0.01),
        None => (t, 1.0),
    };
    let plain: String = if t.contains(',') && t.split(',').skip(1).all(|g| g.len() >= 3 && g[..3].chars().all(|c| c.is_ascii_digit())) {
        t.replace(',', "")
    } else {
        t.to_string()
    };
    plain.parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v * scale)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Numeric {
    pub mean: f64,
    /// Sample standard deviation (n − 1); None with one value.
    pub sd: Option<f64>,
    pub median: f64,
    pub q1: f64,
    pub q3: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    /// Cells with a value.
    pub n: usize,
    pub missing: usize,
    pub numeric: Option<Numeric>,
    /// For a text column: how many different values, and the most common ones with counts.
    pub distinct: usize,
    pub top: Vec<(String, usize)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub level: String,
    pub n: usize,
    pub mean: f64,
    pub sd: Option<f64>,
}

/// A numeric column summarised for each value of a text column with a few values.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub by: String,
    pub value: String,
    pub groups: Vec<Group>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Correlation {
    pub a: String,
    pub b: String,
    /// Pearson's r over the rows where both have a number.
    pub r: f64,
    pub n: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub rows: usize,
    pub columns: Vec<Column>,
    pub comparisons: Vec<Comparison>,
    pub correlations: Vec<Correlation>,
}

/// A column is numeric when at least 80% of its values are numbers.
const NUMERIC_SHARE: f64 = 0.8;
/// Text columns with 2 to this many values group the numeric columns.
const MAX_GROUPS: usize = 8;
const MAX_COMPARISONS: usize = 12;
const MAX_CORRELATIONS: usize = 10;

pub fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len() as f64
}

pub fn sd(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let m = mean(v);
    Some((v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (v.len() - 1) as f64).sqrt())
}

/// The `p` quantile (0..=1) of sorted values, interpolating between neighbours (as Excel's
/// QUARTILE.INC and R's default).
pub fn quantile(sorted: &[f64], p: f64) -> f64 {
    let h = (sorted.len() - 1) as f64 * p;
    let lo = h.floor() as usize;
    let hi = h.ceil() as usize;
    sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
}

fn numeric(values: &[f64]) -> Numeric {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Numeric {
        mean: mean(values),
        sd: sd(values),
        median: quantile(&sorted, 0.5),
        q1: quantile(&sorted, 0.25),
        q3: quantile(&sorted, 0.75),
        min: sorted[0],
        max: sorted[sorted.len() - 1],
    }
}

pub fn pearson(x: &[f64], y: &[f64]) -> Option<f64> {
    if x.len() < 3 {
        return None;
    }
    let (mx, my) = (mean(x), mean(y));
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (a, b) in x.iter().zip(y) {
        sxy += (a - mx) * (b - my);
        sxx += (a - mx).powi(2);
        syy += (b - my).powi(2);
    }
    (sxx > 0.0 && syy > 0.0).then(|| sxy / (sxx * syy).sqrt())
}

pub fn summarize(table: &Table) -> Summary {
    let cols = table.headers.len();
    fn cell(r: &[String], c: usize) -> &str {
        r.get(c).map(String::as_str).unwrap_or("")
    }
    let mut columns = Vec::new();
    // Per column: each row's number (None: missing or not a number), when the column is numeric.
    let mut nums: Vec<Option<Vec<Option<f64>>>> = Vec::new();
    for c in 0..cols {
        let present: Vec<&str> = table.rows.iter().map(|r| cell(r, c)).filter(|v| !is_missing(v)).collect();
        let parsed: Vec<f64> = present.iter().filter_map(|v| number(v)).collect();
        let is_numeric = !present.is_empty() && parsed.len() as f64 >= present.len() as f64 * NUMERIC_SHARE;
        let mut counts: Vec<(String, usize)> = Vec::new();
        if !is_numeric {
            let mut map: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
            for v in &present {
                *map.entry(v.trim()).or_default() += 1;
            }
            counts = map.into_iter().map(|(k, n)| (k.to_string(), n)).collect();
            counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        }
        columns.push(Column {
            name: table.headers[c].clone(),
            n: present.len(),
            missing: table.rows.len() - present.len(),
            numeric: (is_numeric && !parsed.is_empty()).then(|| numeric(&parsed)),
            distinct: counts.len(),
            top: counts.into_iter().take(5).collect(),
        });
        nums.push(is_numeric.then(|| table.rows.iter().map(|r| Some(cell(r, c)).filter(|v| !is_missing(v)).and_then(number)).collect()));
    }

    let mut comparisons = Vec::new();
    for (g, gcol) in columns.iter().enumerate() {
        if gcol.numeric.is_some() || gcol.distinct < 2 || gcol.distinct > MAX_GROUPS {
            continue;
        }
        let mut levels: Vec<String> = table.rows.iter().map(|r| cell(r, g).trim().to_string()).filter(|v| !is_missing(v)).collect();
        levels.sort();
        levels.dedup();
        for (v, values) in nums.iter().enumerate() {
            let Some(values) = values else { continue };
            let mut groups = Vec::new();
            for level in &levels {
                let xs: Vec<f64> = table.rows.iter().zip(values).filter(|(r, _)| cell(r, g).trim() == level).filter_map(|(_, x)| *x).collect();
                if !xs.is_empty() {
                    groups.push(Group { level: level.clone(), n: xs.len(), mean: mean(&xs), sd: sd(&xs) });
                }
            }
            if groups.len() >= 2 && comparisons.len() < MAX_COMPARISONS {
                comparisons.push(Comparison { by: gcol.name.clone(), value: columns[v].name.clone(), groups });
            }
        }
    }

    let mut correlations = Vec::new();
    for a in 0..cols {
        for b in a + 1..cols {
            let (Some(xa), Some(xb)) = (&nums[a], &nums[b]) else { continue };
            let (x, y): (Vec<f64>, Vec<f64>) = xa.iter().zip(xb).filter_map(|(p, q)| Some(((*p)?, (*q)?))).unzip();
            if let Some(r) = pearson(&x, &y) {
                correlations.push(Correlation { a: columns[a].name.clone(), b: columns[b].name.clone(), r, n: x.len() });
            }
        }
    }
    correlations.sort_by(|p, q| q.r.abs().total_cmp(&p.r.abs()));
    correlations.truncate(MAX_CORRELATIONS);

    Summary { rows: table.rows.len(), columns, comparisons, correlations }
}

/// A number for people and for the AI: up to 4 significant digits after the point.
pub fn fmt(v: f64) -> String {
    if v == 0.0 {
        return "0".into();
    }
    let digits = if v.abs() >= 1000.0 { 1 } else if v.abs() >= 1.0 { 3 } else { 4 };
    let s = format!("{v:.digits$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

fn fmt_sd(sd: Option<f64>) -> String {
    sd.map(fmt).unwrap_or_else(|| "–".into())
}

impl Summary {
    /// The statistics as compact text for the AI (no data rows).
    pub fn to_prompt(&self) -> String {
        let mut out = format!("Rows: {}\nColumns:\n", self.rows);
        for c in &self.columns {
            match &c.numeric {
                Some(x) => out.push_str(&format!(
                    "- {} (numeric): n={}, missing={}, mean={} ± SD {}, median={} (IQR {}–{}), min={}, max={}\n",
                    c.name, c.n, c.missing, fmt(x.mean), fmt_sd(x.sd), fmt(x.median), fmt(x.q1), fmt(x.q3), fmt(x.min), fmt(x.max)
                )),
                None => {
                    let top: Vec<String> = c.top.iter().map(|(v, n)| format!("{} ({n})", short(v, 40))).collect();
                    out.push_str(&format!("- {} (text): n={}, missing={}, {} different values; most common: {}\n", c.name, c.n, c.missing, c.distinct, top.join(", ")));
                }
            }
        }
        if !self.comparisons.is_empty() {
            out.push_str("Group comparisons (n, mean ± SD):\n");
            for cmp in &self.comparisons {
                let g: Vec<String> = cmp.groups.iter().map(|g| format!("{}: n={}, {} ± {}", short(&g.level, 40), g.n, fmt(g.mean), fmt_sd(g.sd))).collect();
                out.push_str(&format!("- {} by {}: {}\n", cmp.value, cmp.by, g.join("; ")));
            }
        }
        if !self.correlations.is_empty() {
            out.push_str("Correlations (Pearson r, pairwise complete rows):\n");
            for c in &self.correlations {
                out.push_str(&format!("- {} vs {}: r={} (n={})\n", c.a, c.b, fmt(c.r), c.n));
            }
        }
        out
    }
}

fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() } else { format!("{}…", s.chars().take(max).collect::<String>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn parses_quoted_csv() {
        let rows = parse_delimited("a,b\n\"x, y\",\"he said \"\"hi\"\"\"\r\n1,\"two\nlines\"\n", ',');
        assert_eq!(rows, vec![vec!["a", "b"], vec!["x, y", "he said \"hi\""], vec!["1", "two\nlines"]]);
    }

    #[test]
    fn guesses_delimiters() {
        assert_eq!(guess_delimiter("x.tsv", "a,b"), '\t');
        assert_eq!(guess_delimiter("x.csv", "a;b;c"), ';');
        assert_eq!(guess_delimiter("x.csv", "a,b"), ',');
    }

    #[test]
    fn numbers_and_missing() {
        assert_eq!(number("1,234.5"), Some(1234.5));
        assert_eq!(number("12.5%"), Some(0.125));
        assert_eq!(number(" 3 "), Some(3.0));
        assert_eq!(number("1,2"), None);
        assert!(is_missing(" NA ") && is_missing("") && !is_missing("0"));
    }

    #[test]
    fn known_statistics() {
        // 2, 4, 4, 4, 5, 5, 7, 9: mean 5, sample SD sqrt(32/7), median 4.5, Q1 4, Q3 5.5.
        let t = Table::from_rows(vec![
            vec!["v".into()],
            vec!["2".into()],
            vec!["4".into()],
            vec!["4".into()],
            vec!["4".into()],
            vec!["5".into()],
            vec!["5".into()],
            vec!["7".into()],
            vec!["9".into()],
            vec!["NA".into()],
        ]);
        let s = summarize(&t);
        let c = &s.columns[0];
        assert_eq!((c.n, c.missing), (8, 1));
        let x = c.numeric.as_ref().unwrap();
        assert!(close(x.mean, 5.0));
        assert!(close(x.sd.unwrap(), (32.0f64 / 7.0).sqrt()));
        assert!(close(x.median, 4.5) && close(x.q1, 4.0) && close(x.q3, 5.5));
        assert!(close(x.min, 2.0) && close(x.max, 9.0));
    }

    #[test]
    fn groups_and_correlation() {
        let mut rows = vec![vec!["group".to_string(), "x".into(), "y".into()]];
        for (g, x) in [("a", 1.0), ("a", 2.0), ("a", 3.0), ("b", 10.0), ("b", 12.0)] {
            rows.push(vec![g.into(), x.to_string(), (2.0 * x + 1.0).to_string()]);
        }
        let s = summarize(&Table::from_rows(rows));
        assert_eq!(s.rows, 5);
        assert!(s.columns[0].numeric.is_none());
        assert_eq!(s.columns[0].distinct, 2);
        let cmp = s.comparisons.iter().find(|c| c.by == "group" && c.value == "x").unwrap();
        assert_eq!(cmp.groups[0].level, "a");
        assert_eq!(cmp.groups[0].n, 3);
        assert!(close(cmp.groups[0].mean, 2.0));
        assert!(close(cmp.groups[1].mean, 11.0));
        assert!(close(s.correlations[0].r, 1.0));
        assert_eq!(s.correlations[0].n, 5);
        let text = s.to_prompt();
        assert!(text.contains("x (numeric): n=5, missing=0, mean=5.6"));
        assert!(text.contains("x by group: a: n=3, 2 ± 1; b: n=2, 11 ± 1.414"));
        assert!(text.contains("x vs y: r=1 (n=5)"));
    }

    #[test]
    fn formats_numbers() {
        assert_eq!(fmt(5.0), "5");
        assert_eq!(fmt(1234.56), "1234.6");
        assert_eq!(fmt(0.123456), "0.1235");
        assert_eq!(fmt(-0.00001), "0");
    }
}
