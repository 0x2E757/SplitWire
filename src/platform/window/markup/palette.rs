//! TaskbarMetrics' design tokens (`docs/design/tokens.json`, main window) for both themes. Markup
//! is written with `$token$` placeholders; `Palette::apply` fills them for one theme.

use crate::profile::Pin;

/// `(token, light, dark)`.
const COLORS: &[(&str, &str, &str)] = &[
    ("bg", "#EEF0F3", "#191A1C"),
    ("layer", "#F8F9FA", "#202124"),
    ("card", "#FFFFFF", "#28292C"),
    ("border", "#E1E3E6", "#36383C"),
    ("divider", "#ECEEF0", "#2F3134"),
    ("text", "#1B1B1B", "#FFFFFF"),
    ("text2", "#4E5258", "#D2D4D7"),
    ("text3", "#62666C", "#A7AAAF"),
    ("textMuted", "#868C95", "#7B8087"),
    ("accent", "#005FB8", "#60CDFF"),
    ("onAccent", "#FFFFFF", "#00263A"),
    ("accentSoft", "#E3EEF9", "#1B3444"),
    // The line of TaskbarMetrics' charts; SplitWire's charts draw Receive in it.
    ("total", "#0F6CBD", "#4CC2FF"),
    ("grid", "#EDEEF0", "#2F3134"),
    ("rowHover", "#EFF1F4", "#303236"),
    ("ctrl", "#FFFFFF", "#2E3033"),
    ("ctrlBorder", "#D3D6DA", "#44464A"),
    ("ctrlHover", "#F6F7F8", "#35373B"),
    ("ctrlPress", "#F0F1F3", "#2A2C2F"),
    ("subtle", "#F1F2F4", "#26272A"),
    ("navSel", "#E4E7EB", "#2C2E31"),
    ("navHover", "#E8EAEE", "#222326"),
    ("navPress", "#E0E3E7", "#2A2C2F"),
    ("warn", "#8A5100", "#FCE100"),
    ("warnBg", "#FFF4CE", "#3A3217"),
    ("warnBorder", "#F2DC9B", "#5A4D1E"),
    ("danger", "#C42B1C", "#FF99A4"),
    ("disabled", "#A7AAAE", "#6B6E72"),
    ("textBoxBottom", "#8A8D91", "#44464A"),
    ("statusDot", "#C27C0E", "#FCE100"),
];

const PINS: [[&str; 6]; 2] = [
    [
        "#C25400", "#7A4FD0", "#00806F", "#C2185B", "#8C6A00", "#5E7A00",
    ],
    [
        "#FF9E50", "#B39DFF", "#45D0B5", "#FF7BB5", "#E6C34A", "#A8D86A",
    ],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
}

impl Palette {
    pub const BOTH: [Self; 2] = [Self { dark: false }, Self { dark: true }];

    pub fn color(self, token: &str) -> &'static str {
        COLORS
            .iter()
            .find(|(key, _, _)| *key == token)
            .map_or(
                "Transparent",
                |(_, light, dark)| if self.dark { dark } else { light },
            )
    }

    pub fn pin(self, pin: Pin) -> &'static str {
        PINS[usize::from(self.dark)][usize::from(pin.number() - 1) % 6]
    }

    /// `#RRGGBB` with an opacity as `#AARRGGBB`.
    pub fn alpha(color: &str, opacity: f64) -> String {
        let rgb = color.trim_start_matches('#');
        format!(
            "#{:02X}{}",
            (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
            &rgb[rgb.len().saturating_sub(6)..]
        )
    }

    /// `over` mixed into `base` by `share` (CSS `color-mix`), as `#RRGGBB`.
    pub fn mix(over: &str, base: &str, share: f64) -> String {
        let channel = |hex: &str, i: usize| {
            u8::from_str_radix(&hex.trim_start_matches('#')[i * 2..i * 2 + 2], 16).unwrap_or(0)
                as f64
        };
        let mixed: Vec<String> = (0..3)
            .map(|i| {
                let value = channel(over, i) * share + channel(base, i) * (1.0 - share);
                format!("{:02X}", value.round() as u8)
            })
            .collect();
        format!("#{}", mixed.concat())
    }

    /// Tokens derived from the others.
    fn derived(self) -> Vec<(String, String)> {
        let c = |t| self.color(t);
        let mut derived = vec![
            ("accentHover".into(), Self::alpha(c("accent"), 0.9)),
            ("accentPress".into(), Self::alpha(c("accent"), 0.8)),
            ("accentRing".into(), Self::alpha(c("accent"), 0.22)),
            ("accentBorder".into(), Self::alpha(c("accent"), 0.35)),
            // `alpha.totalFill` of the tokens.
            (
                "totalFill".into(),
                Self::alpha(c("total"), if self.dark { 0.13 } else { 0.1 }),
            ),
            ("busyRing".into(), Self::alpha(c("statusDot"), 0.22)),
            ("dangerRing".into(), Self::alpha(c("danger"), 0.22)),
            // The kit has no error tints; they are derived from danger over the card.
            ("errBg".into(), Self::mix(c("danger"), c("card"), 0.09)),
            ("errBorder".into(), Self::mix(c("danger"), c("card"), 0.35)),
            (
                "scrim".into(),
                if self.dark {
                    "#8C000000".into()
                } else {
                    "#4D000000".into()
                },
            ),
            (
                "theme".into(),
                if self.dark {
                    "Dark".into()
                } else {
                    "Light".into()
                },
            ),
        ];
        for number in 1..=Pin::COUNT {
            let pin = Pin::new(number).expect("a pin");
            derived.push((format!("pin{number}"), self.pin(pin).into()));
            derived.push((format!("pin{number}a"), Self::alpha(self.pin(pin), 0.35)));
            derived.push((format!("pin{number}f"), Self::alpha(self.pin(pin), 0.2)));
        }
        derived
    }

    /// The markup with every `$token$` replaced and the theme set.
    pub fn apply(self, markup: &str) -> String {
        let mut out = String::with_capacity(markup.len());
        let derived = self.derived();
        let mut rest = markup;
        while let Some(start) = rest.find('$') {
            out.push_str(&rest[..start]);
            let tail = &rest[start + 1..];
            let Some(end) = tail.find('$') else {
                out.push_str(&rest[start..]);
                rest = "";
                break;
            };
            let token = &tail[..end];
            let value = COLORS
                .iter()
                .find(|(key, _, _)| *key == token)
                .map(|(_, light, dark)| (if self.dark { *dark } else { *light }).to_owned())
                .or_else(|| {
                    derived
                        .iter()
                        .find(|(k, _)| k == token)
                        .map(|(_, v)| v.clone())
                });
            match value {
                Some(value) => {
                    out.push_str(&value);
                    rest = &tail[end + 1..];
                }
                None => {
                    out.push('$');
                    rest = tail;
                }
            }
        }
        out.push_str(rest);
        out.replace(
            "RequestedTheme=\"Default\"",
            if self.dark {
                "RequestedTheme=\"Dark\""
            } else {
                "RequestedTheme=\"Light\""
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_fill_for_each_theme() {
        let markup = r#"<A RequestedTheme="Default" B="$text$" C="$pin3a$" D="$errBg$" E="$x$"/>"#;
        let light = Palette { dark: false }.apply(markup);
        assert_eq!(
            light,
            r##"<A RequestedTheme="Light" B="#1B1B1B" C="#5900806F" D="#FAECEB" E="$x$"/>"##
        );
        let dark = Palette { dark: true }.apply(markup);
        assert!(dark.contains(r##"B="#FFFFFF""##) && dark.contains("RequestedTheme=\"Dark\""));
        assert_eq!(Palette::alpha("#005FB8", 0.35), "#59005FB8");
        let chart = "$total$ $totalFill$";
        assert_eq!(Palette { dark: false }.apply(chart), "#0F6CBD #1A0F6CBD");
        assert_eq!(Palette { dark: true }.apply(chart), "#4CC2FF #214CC2FF");
        assert_eq!(Palette::mix("#FFFFFF", "#000000", 0.5), "#808080");
    }
}
