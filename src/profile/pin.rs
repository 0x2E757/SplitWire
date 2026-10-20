use std::fmt;

/// A profile's colour: one of TaskbarMetrics' six pin colours, 1–6. Colours repeat beyond six
/// profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pin(u8);

impl Pin {
    pub const COUNT: u8 = 6;

    pub fn new(number: u8) -> Option<Self> {
        (1..=Self::COUNT).contains(&number).then_some(Self(number))
    }

    pub fn parse(text: &str) -> Option<Self> {
        text.trim().parse().ok().and_then(Self::new)
    }

    pub fn number(self) -> u8 {
        self.0
    }

    /// The first pin nobody uses; when all are used, the first of the least used.
    pub fn first_unused(used: impl IntoIterator<Item = Pin>) -> Self {
        let mut counts = [0usize; Self::COUNT as usize];
        for pin in used {
            counts[usize::from(pin.0 - 1)] += 1;
        }
        let fewest = counts.iter().min().copied().unwrap_or_default();
        let index = counts
            .iter()
            .position(|count| *count == fewest)
            .unwrap_or(0);
        Self(index as u8 + 1)
    }
}

impl fmt::Display for Pin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pins(numbers: &[u8]) -> Vec<Pin> {
        numbers.iter().map(|n| Pin::new(*n).unwrap()).collect()
    }

    #[test]
    fn only_one_to_six() {
        assert!(Pin::new(0).is_none());
        assert!(Pin::new(7).is_none());
        assert_eq!(Pin::parse(" 3 "), Pin::new(3));
        assert!(Pin::parse("x").is_none());
    }

    #[test]
    fn proposes_the_first_unused_then_the_least_used() {
        assert_eq!(Pin::first_unused([]).number(), 1);
        assert_eq!(Pin::first_unused(pins(&[1, 2, 4])).number(), 3);
        assert_eq!(Pin::first_unused(pins(&[1, 2, 3, 4, 5, 6])).number(), 1);
        assert_eq!(
            Pin::first_unused(pins(&[1, 2, 3, 4, 5, 6, 1, 2])).number(),
            3
        );
    }
}
