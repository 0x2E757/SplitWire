//! Numbers, sizes, rates and spans as the window writes them: `6 240`, `1.24 GB`,
//! `38 KB/s`, `2 h 14 min`, `1 min 18 s ago`.

pub struct Format;

impl Format {
    /// Thousands separated by a space: `6 240`.
    pub fn count(value: u64) -> String {
        let digits = value.to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3);
        for (index, digit) in digits.chars().enumerate() {
            if index > 0 && (digits.len() - index).is_multiple_of(3) {
                out.push(' ');
            }
            out.push(digit);
        }
        out
    }

    /// A total: `1.24 GB`, `318.6 MB`, `512 B`.
    pub fn bytes(value: u64) -> String {
        let (scaled, unit) = Self::scale(value as f64);
        if unit == "B" {
            return format!("{value} B");
        }
        let decimals = if scaled < 10.0 { 2 } else { 1 };
        format!("{scaled:.decimals$} {unit}")
    }

    /// A rate in bytes per second: `38 KB/s`, `6.1 KB/s`.
    pub fn rate(bytes_per_second: u64) -> String {
        let (scaled, unit) = Self::scale(bytes_per_second as f64);
        let decimals = usize::from(scaled < 10.0 && unit != "B");
        format!("{scaled:.decimals$} {unit}/s")
    }

    fn scale(value: f64) -> (f64, &'static str) {
        let mut scaled = value;
        for unit in ["B", "KB", "MB", "GB"] {
            if scaled < 1000.0 {
                return (scaled, unit);
            }
            scaled /= 1000.0;
        }
        (scaled, "TB")
    }

    /// A coarse span: `42 s`, `47 min`, `2 h 14 min`, `3 d 2 h`.
    pub fn span(secs: u64) -> String {
        match secs {
            0..60 => format!("{secs} s"),
            60..3600 => format!("{} min", secs / 60),
            3600..86_400 => format!("{} h {} min", secs / 3600, secs % 3600 / 60),
            _ => format!("{} d {} h", secs / 86_400, secs % 86_400 / 3600),
        }
    }

    /// A span with seconds under ten minutes: `1 min 18 s`.
    pub fn span_fine(secs: u64) -> String {
        if (60..600).contains(&secs) && !secs.is_multiple_of(60) {
            format!("{} min {} s", secs / 60, secs % 60)
        } else {
            Self::span(secs)
        }
    }

    /// `42 s ago`, `1 min ago`.
    pub fn ago(secs: u64) -> String {
        format!("{} ago", Self::span(secs))
    }

    /// `in 23 h 59 min`.
    pub fn within(secs: u64) -> String {
        format!("in {}", Self::span(secs))
    }

    /// `1 entry`, `2 entries`.
    pub fn plural(count: usize, singular: &str, plural: &str) -> String {
        format!("{count} {}", if count == 1 { singular } else { plural })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_group_thousands_with_a_space() {
        assert_eq!(Format::count(0), "0");
        assert_eq!(Format::count(418), "418");
        assert_eq!(Format::count(6240), "6 240");
        assert_eq!(Format::count(1_234_567), "1 234 567");
    }

    #[test]
    fn sizes_and_rates_read_as_the_window_writes_them() {
        assert_eq!(Format::bytes(1_240_000_000), "1.24 GB");
        assert_eq!(Format::bytes(318_600_000), "318.6 MB");
        assert_eq!(Format::bytes(86_400_000), "86.4 MB");
        assert_eq!(Format::bytes(512), "512 B");
        assert_eq!(Format::rate(38_000), "38 KB/s");
        assert_eq!(Format::rate(6_100), "6.1 KB/s");
        assert_eq!(Format::rate(900), "900 B/s");
    }

    #[test]
    fn spans_are_coarse_or_fine() {
        assert_eq!(Format::span(42), "42 s");
        assert_eq!(Format::span(47 * 60 + 5), "47 min");
        assert_eq!(Format::span(2 * 3600 + 14 * 60), "2 h 14 min");
        assert_eq!(Format::span_fine(78), "1 min 18 s");
        assert_eq!(Format::span_fine(120), "2 min");
        assert_eq!(Format::ago(12), "12 s ago");
        assert_eq!(Format::within(23 * 3600 + 59 * 60), "in 23 h 59 min");
        assert_eq!(Format::plural(1, "entry", "entries"), "1 entry");
        assert_eq!(Format::plural(4, "entry", "entries"), "4 entries");
    }
}
