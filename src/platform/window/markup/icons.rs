//! The window's own 16 px icons: TaskbarMetrics' set plus SplitWire's,
//! drawn by the same rules: a 16 × 16 box, round caps and joins, stroke 1.3 (1.6 at 12 px and
//! below), some parts filled.

/// `(name, [(path data, filled)])`.
const ICONS: &[(&str, &[(&str, bool)])] = &[
    (
        "gear",
        &[
            (
                "M5.93 3.56L6.19 1.24L9.81 1.24L10.07 3.56A4.9 4.9 0 0 1 10.81 3.99L12.95 3.05L14.76 6.19L12.88 7.57A4.9 4.9 0 0 1 12.88 8.43L14.76 9.81L12.95 12.95L10.81 12.01A4.9 4.9 0 0 1 10.07 12.44L9.81 14.76L6.19 14.76L5.93 12.44A4.9 4.9 0 0 1 5.19 12.01L3.05 12.95L1.24 9.81L3.12 8.43A4.9 4.9 0 0 1 3.12 7.57L1.24 6.19L3.05 3.05L5.19 3.99A4.9 4.9 0 0 1 5.93 3.56Z",
                false,
            ),
            ("M5.6 8a2.4 2.4 0 1 0 4.8 0a2.4 2.4 0 1 0-4.8 0z", false),
        ],
    ),
    (
        "search",
        &[
            ("M2.5 7A4.5 4.5 0 1 0 11.5 7A4.5 4.5 0 1 0 2.5 7Z", false),
            ("M10.4 10.4L14 14", false),
        ],
    ),
    (
        "info",
        &[
            ("M1.5 8A6.5 6.5 0 1 0 14.5 8A6.5 6.5 0 1 0 1.5 8Z", false),
            ("M8 7.2v4.3M8 4.6v0.2", false),
        ],
    ),
    (
        "clock",
        &[
            ("M1.5 8A6.5 6.5 0 1 0 14.5 8A6.5 6.5 0 1 0 1.5 8Z", false),
            ("M8 4.5V8l2.4 1.6", false),
        ],
    ),
    ("return", &[("M2 8h9M8 4.8L11.2 8L8 11.2M14 3v10", false)]),
    ("chev", &[("M4 6l4 4l4-4", false)]),
    ("chevu", &[("M4 10l4-4l4 4", false)]),
    ("chevr", &[("M6 4l4 4l-4 4", false)]),
    ("close", &[("M3.5 3.5l9 9M12.5 3.5l-9 9", false)]),
    ("min", &[("M3 8h10", false)]),
    (
        "max",
        &[(
            "M4.5 3.5H11.5A1 1 0 0 1 12.5 4.5V11.5A1 1 0 0 1 11.5 12.5H4.5A1 1 0 0 1 3.5 11.5V4.5A1 1 0 0 1 4.5 3.5Z",
            false,
        )],
    ),
    ("menu", &[("M2 4h12M2 8h12M2 12h12", false)]),
    ("x", &[("M4.5 4.5l7 7M11.5 4.5l-7 7", false)]),
    ("check", &[("M3 8.5l3 3l7-7", false)]),
    (
        "alert",
        &[
            ("M8 1.8L14.5 13.5H1.5Z", false),
            ("M8 6v3.6M8 11.5v0.2", false),
        ],
    ),
    (
        "pause",
        &[
            ("M1.5 8A6.5 6.5 0 1 0 14.5 8A6.5 6.5 0 1 0 1.5 8Z", false),
            ("M6.5 5.5v5M9.5 5.5v5", false),
        ],
    ),
    (
        "globe",
        &[
            ("M1.5 8A6.5 6.5 0 1 0 14.5 8A6.5 6.5 0 1 0 1.5 8Z", false),
            (
                "M1.5 8h13M8 1.5c2 2 2.8 4.2 2.8 6.5S10 12.5 8 14.5C6 12.5 5.2 10.3 5.2 8S6 3.5 8 1.5",
                false,
            ),
        ],
    ),
    (
        "bell",
        &[
            ("M4 11V7a4 4 0 0 1 8 0v4l1 1.5H3Z", false),
            ("M6.5 14h3", false),
        ],
    ),
    (
        "layout",
        &[
            (
                "M3 3H13A1.5 1.5 0 0 1 14.5 4.5V11.5A1.5 1.5 0 0 1 13 13H3A1.5 1.5 0 0 1 1.5 11.5V4.5A1.5 1.5 0 0 1 3 3Z",
                false,
            ),
            ("M1.5 7h13M6 7v6", false),
        ],
    ),
    ("chart", &[("M1.5 13.5h13M3 11l3-4l3 2.5l4.5-6", false)]),
    (
        "history",
        &[(
            "M2.5 8a5.5 5.5 0 1 0 1.6-3.9M2.5 2.5v2.8h2.8M8 5v3l2 1.2",
            false,
        )],
    ),
    (
        "shield",
        &[(
            "M8 1.5l5.5 2v4.2c0 3.2-2.3 5.6-5.5 6.8C4.8 13.3 2.5 10.9 2.5 7.7V3.5Z",
            false,
        )],
    ),
    (
        "dot",
        &[("M4.5 8A3.5 3.5 0 1 0 11.5 8A3.5 3.5 0 1 0 4.5 8Z", true)],
    ),
    ("arrowdown", &[("M8 2.5v11M4 9.5l4 4l4-4", false)]),
    ("arrowup", &[("M8 13.5v-11M4 6.5l4-4l4 4", false)]),
    ("pulse", &[("M1.5 8h3l1.5-4l3 8l1.5-4h4", false)]),
    (
        "eyeoff",
        &[(
            "M2 2l12 12M6.5 6.6a2 2 0 0 0 2.9 2.8M4.2 4.3C2.9 5.2 2 6.5 1.5 8c1.2 3 3.8 4.5 6.5 4.5c1.3 0 2.5-.3 3.6-1M8 3.5c2.7 0 5.3 1.5 6.5 4.5c-.4 1-1 1.9-1.7 2.6",
            false,
        )],
    ),
    (
        "reset",
        &[("M2.5 8a5.5 5.5 0 1 0 1.6-3.9M2.5 2.5v2.8h2.8", false)],
    ),
    ("power", &[("M5.2 3.6a5.5 5.5 0 1 0 5.6 0M8 1.5v6", false)]),
    (
        "routes",
        &[
            ("M8 14.5V9M8 9L3.5 4.5M8 9l4.5-4.5", false),
            ("M2.5 7.5V3.5h4M13.5 7.5V3.5h-4", false),
        ],
    ),
    (
        "list",
        &[
            ("M5.5 4h9M5.5 8h9M5.5 12h9", false),
            (
                "M1.6 4a.9.9 0 1 0 1.8 0a.9.9 0 1 0-1.8 0zM1.6 8a.9.9 0 1 0 1.8 0a.9.9 0 1 0-1.8 0zM1.6 12a.9.9 0 1 0 1.8 0a.9.9 0 1 0-1.8 0z",
                true,
            ),
        ],
    ),
    (
        "logs",
        &[
            ("M3.5 1.5h6l3 3v10h-9z", false),
            ("M9.5 1.5v3h3M5.5 8h5M5.5 10.5h5M5.5 13h3", false),
        ],
    ),
    (
        "folder",
        &[(
            "M1.5 4a1 1 0 0 1 1-1h3.6l1.5 1.5h5.9a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z",
            false,
        )],
    ),
    (
        "copy",
        &[
            (
                "M5.5 5.5h7a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1z",
                false,
            ),
            ("M2.5 10.5v-8a1 1 0 0 1 1-1h8", false),
        ],
    ),
    ("plus", &[("M8 2.5v11M2.5 8h11", false)]),
    (
        "trash",
        &[(
            "M2.5 4h11M6 4V2.5h4V4M4 4l.7 9.6a1 1 0 0 0 1 .9h4.6a1 1 0 0 0 1-.9L12 4",
            false,
        )],
    ),
    (
        "file",
        &[("M3.5 1.5h6l3 3v10h-9z", false), ("M9.5 1.5v3h3", false)],
    ),
    (
        "import",
        &[
            ("M3.5 1.5h6l3 3v10h-9z", false),
            ("M9.5 1.5v3h3M8 6.5v5M5.8 9.3L8 11.5l2.2-2.2", false),
        ],
    ),
    (
        "key",
        &[
            (
                "M10.5 1.5a4 4 0 1 1-3.7 5.6L1.5 12.4v2.1h2.5V13h1.5v-1.5H7l1.3-1.3A4 4 0 0 1 10.5 1.5z",
                false,
            ),
            ("M10.3 5a1 1 0 1 0 2 0a1 1 0 1 0-2 0z", true),
        ],
    ),
    ("edit", &[("M10.8 2.2l3 3L5.5 13.5l-3.5.5l.5-3.5z", false)]),
    ("filter", &[("M1.5 2.5h13l-5 6v5l-3 1.5v-6.5z", false)]),
    (
        "more",
        &[(
            "M2.4 8a.9.9 0 1 0 1.8 0a.9.9 0 1 0-1.8 0zM7.1 8a.9.9 0 1 0 1.8 0a.9.9 0 1 0-1.8 0zM11.8 8a.9.9 0 1 0 1.8 0a.9.9 0 1 0-1.8 0z",
            true,
        )],
    ),
    (
        "error",
        &[
            ("M1.5 8A6.5 6.5 0 1 0 14.5 8A6.5 6.5 0 1 0 1.5 8Z", false),
            ("M5.8 5.8l4.4 4.4M10.2 5.8l-4.4 4.4", false),
        ],
    ),
    (
        "ok",
        &[
            ("M1.5 8A6.5 6.5 0 1 0 14.5 8A6.5 6.5 0 1 0 1.5 8Z", false),
            ("M5 8.2l2.1 2.1L11 6.4", false),
        ],
    ),
    (
        "link",
        &[(
            "M6.5 9.5l3-3M7 4.5l1.3-1.3a2.8 2.8 0 0 1 4 4L11 8.5M9 11.5l-1.3 1.3a2.8 2.8 0 0 1-4-4L5 7.5",
            false,
        )],
    ),
    (
        "open",
        &[(
            "M9.5 2.5h4v4M13.5 2.5L7.5 8.5M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3",
            false,
        )],
    ),
    (
        "lock",
        &[
            ("M3.5 7.5h9v7h-9z", false),
            ("M5.5 7.5V5a2.5 2.5 0 0 1 5 0v2.5", false),
        ],
    ),
    (
        "server",
        &[
            ("M2.5 2.5h11v4.5h-11zM2.5 9h11v4.5h-11z", false),
            ("M4.5 4.75h.1M4.5 11.25h.1", false),
        ],
    ),
    (
        "tunnel",
        &[
            ("M1.5 13.5V8a6.5 6.5 0 0 1 13 0v5.5", false),
            ("M4.5 13.5V8a3.5 3.5 0 0 1 7 0v5.5", false),
        ],
    ),
    ("play", &[("M5 3l8 5l-8 5z", false)]),
    (
        "sync",
        &[(
            "M13.5 8a5.5 5.5 0 0 1-9.6 3.7M2.5 8a5.5 5.5 0 0 1 9.6-3.7M12.3 1.8v2.6H9.7M3.7 14.2v-2.6h2.6",
            false,
        )],
    ),
    (
        "down",
        &[("M8 2.5v8M4.5 7l3.5 3.5L11.5 7M3 13.5h10", false)],
    ),
];

pub struct Icons;

impl Icons {
    pub fn exists(name: &str) -> bool {
        ICONS.iter().any(|(key, _)| *key == name)
    }

    /// The icon at `size` DIP in `color` (a `$token$` or `#RRGGBB`).
    pub fn markup(name: &str, size: u32, color: &str) -> String {
        let stroke = if size <= 12 { 1.6 } else { 1.3 };
        let mut out = format!(
            r#"<Viewbox Width="{size}" Height="{size}" VerticalAlignment="Center" IsHitTestVisible="False"><Canvas Width="16" Height="16">"#
        );
        if let Some((_, parts)) = ICONS.iter().find(|(key, _)| *key == name) {
            for (data, filled) in *parts {
                if *filled {
                    out.push_str(&format!(r#"<Path Data="{data}" Fill="{color}"/>"#));
                } else {
                    out.push_str(&format!(
                        r#"<Path Data="{data}" Stroke="{color}" StrokeThickness="{stroke}" StrokeStartLineCap="Round" StrokeEndLineCap="Round" StrokeLineJoin="Round"/>"#
                    ));
                }
            }
        }
        out.push_str("</Canvas></Viewbox>");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_draw_strokes_and_fills() {
        let gear = Icons::markup("gear", 16, "$text$");
        assert!(gear.starts_with(r#"<Viewbox Width="16" Height="16""#));
        assert!(gear.contains(r#"StrokeThickness="1.3""#));
        assert!(Icons::markup("more", 12, "#000").contains(r##"Fill="#000""##));
        assert!(Icons::markup("x", 12, "#000").contains(r#"StrokeThickness="1.6""#));
        for name in [
            "shield", "routes", "list", "logs", "plus", "gear", "bell", "tunnel", "server",
            "import",
        ] {
            assert!(Icons::exists(name), "{name}");
        }
    }
}
