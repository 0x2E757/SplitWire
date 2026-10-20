use std::time::Duration;

/// Parses `90`, `20s`, `5m`, `2h`, `1d` or combinations such as `1h30m`.
pub struct DurationArg;

impl DurationArg {
    pub fn parse(text: &str) -> Result<Duration, String> {
        let bad = || format!("'{text}' is not a duration (e.g. 20s, 5m, 1h30m)");
        if let Ok(secs) = text.parse::<u64>() {
            return Ok(Duration::from_secs(secs));
        }
        let mut total = 0u64;
        let mut number = String::new();
        for c in text.chars() {
            if c.is_ascii_digit() {
                number.push(c);
                continue;
            }
            let unit = match c {
                's' => 1,
                'm' => 60,
                'h' => 3600,
                'd' => 86_400,
                _ => return Err(bad()),
            };
            let value: u64 = number.parse().map_err(|_| bad())?;
            total = value
                .checked_mul(unit)
                .and_then(|v| total.checked_add(v))
                .ok_or_else(bad)?;
            number.clear();
        }
        if !number.is_empty() || total == 0 {
            return Err(bad());
        }
        Ok(Duration::from_secs(total))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_units_and_combinations() {
        assert_eq!(DurationArg::parse("20s").unwrap(), Duration::from_secs(20));
        assert_eq!(DurationArg::parse("90").unwrap(), Duration::from_secs(90));
        assert_eq!(DurationArg::parse("5m").unwrap(), Duration::from_secs(300));
        assert_eq!(
            DurationArg::parse("1h30m").unwrap(),
            Duration::from_secs(5_400)
        );
    }

    #[test]
    fn rejects_nonsense() {
        for bad in ["", "s", "10x", "1h30", "0s", "-5s", "99999999999999999999d"] {
            assert!(DurationArg::parse(bad).is_err(), "{bad}");
        }
    }
}
