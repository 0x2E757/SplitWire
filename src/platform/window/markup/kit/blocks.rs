//! Blocks: the header, stats, InfoBars, cards, expanders and their rows, the apply bar, empty
//! states and table rows.

use super::{Kit, Look, Tone};
use crate::platform::window::markup::Align;
use crate::platform::window::markup::region::{Event, Out};
use crate::presentation::pages::{InfoBar, Stat};
use crate::presentation::{ApplyBarView, Command, Dot};

impl Kit {
    /// The page header: h1 28/600, the subtitle on its baseline, the end controls and the bell.
    /// The title is trimmed to its capitals, so it sits in the middle of the header like the
    /// controls; the subtitle is trimmed to its baseline, so both end on the same line.
    pub fn header(
        title: &str,
        subtitle: Option<&str>,
        end: &str,
        bell: &str,
        compact: bool,
    ) -> String {
        let subtitle = subtitle.filter(|_| !compact).unwrap_or_default();
        Self::header_with(title, subtitle, "", end, bell)
    }

    /// A header with `middle` centred after the title and its subtitle (the Logs page's
    /// Segmented).
    pub fn header_with(title: &str, subtitle: &str, middle: &str, end: &str, bell: &str) -> String {
        format!(
            r#"<Grid MinHeight="36" ColumnSpacing="12"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Grid VerticalAlignment="Center"{}>{}</Grid><Grid Grid.Column="1" VerticalAlignment="Center">{middle}</Grid><StackPanel Grid.Column="2" Orientation="Horizontal" Spacing="8" VerticalAlignment="Center">{end}{bell}</StackPanel></Grid>"#,
            Align::ROW,
            Align::BLOCK,
            Self::titled(title, 28, subtitle, "$text2$", "")
        )
    }

    /// A semibold title and a 12 px caption in `color` on its baseline (the UI kit's header). The
    /// title is trimmed to its capitals and the caption to its baseline, both at the bottom: the
    /// baselines meet whatever the sizes, and the title's capitals are the middle of the line.
    pub fn titled(title: &str, size: u32, caption: &str, color: &str, extra: &str) -> String {
        let caption = if caption.is_empty() {
            String::new()
        } else {
            format!(
                r#"<TextBlock Text="{}" FontSize="12" Foreground="{color}" VerticalAlignment="Bottom" TextLineBounds="TrimToBaseline" Margin="12,0,0,0"/>"#,
                Self::esc(caption)
            )
        };
        format!(
            r#"<StackPanel Orientation="Horizontal" VerticalAlignment="Center"{}{extra}><TextBlock Text="{}" FontSize="{size}" FontWeight="SemiBold" VerticalAlignment="Bottom" TextLineBounds="Tight"/>{caption}</StackPanel>"#,
            Align::BASELINE,
            Self::esc(title)
        )
    }

    pub fn stat(stat: &Stat, small: bool) -> String {
        format!(
            r#"<StackPanel MinWidth="0">{}{}{}</StackPanel>"#,
            Self::text(&stat.label, 12, "$text2$", ""),
            Self::text(
                &stat.value,
                if small { 18 } else { 20 },
                "$text$",
                r#"FontWeight="SemiBold""#
            ),
            Self::text(&stat.caption, 11, "$text3$", r#"MinHeight="15""#)
        )
    }

    /// The stats row: blocks 36 apart, wrapping when narrow.
    pub fn stats(stats: &[Stat], compact: bool) -> String {
        let blocks: String = stats.iter().map(|s| Self::stat(s, compact)).collect();
        format!(
            r#"<VariableSizedWrapGrid Orientation="Horizontal" ItemWidth="{}">{blocks}</VariableSizedWrapGrid>"#,
            if compact { 150 } else { 190 }
        )
    }

    /// A card's stats in one row of equal columns (TaskbarMetrics' stats row, spread over the
    /// card's width): three lines, each on one baseline: the labels, the values and the captions;
    /// an empty caption still takes its line.
    pub fn spread_stats(stats: &[Stat], compact: bool) -> String {
        let gap = if compact { 28 } else { 36 };
        let value = if compact { 18 } else { 20 };
        let columns: String = stats.iter().map(|_| "<ColumnDefinition/>").collect();
        let line = |cell: &dyn Fn(&Stat, String) -> String| {
            let cells: String = stats
                .iter()
                .enumerate()
                .map(|(column, stat)| {
                    cell(
                        stat,
                        format!(r#"Grid.Column="{column}" TextTrimming="CharacterEllipsis""#),
                    )
                })
                .collect();
            format!(
                r#"<Grid ColumnSpacing="{gap}"{}><Grid.ColumnDefinitions>{columns}</Grid.ColumnDefinitions>{cells}</Grid>"#,
                Align::BASELINE
            )
        };
        format!(
            "<StackPanel>{}{}{}</StackPanel>",
            line(&|s, at| Self::text(&s.label, 12, "$text2$", &at)),
            line(&|s, at| Self::text(
                &s.value,
                value,
                "$text$",
                &format!(r#"FontWeight="SemiBold" {at}"#)
            )),
            line(&|s, at| Self::text(
                &s.caption,
                11,
                "$text3$",
                &format!(r#"MinHeight="15" {at}"#)
            ))
        )
    }

    pub fn infobar(tone: Tone, bar: &InfoBar, actions: &str) -> String {
        let (background, border, icon) = match tone {
            Tone::Error => (
                "$errBg$",
                "$errBorder$",
                Self::icon("error", 16, "$danger$"),
            ),
            Tone::Warning => (
                "$warnBg$",
                "$warnBorder$",
                Self::icon("alert", 16, "$warn$"),
            ),
            Tone::Accent => (
                "$accentSoft$",
                "$accentBorder$",
                Self::icon("info", 16, "$accent$"),
            ),
            Tone::Neutral => ("$subtle$", "$divider$", Self::icon("info", 16, "$text2$")),
        };
        format!(
            r#"<Border Background="{background}" BorderBrush="{border}" BorderThickness="1" CornerRadius="6" Padding="16,11,12,11"><Grid ColumnSpacing="12" RowSpacing="2"><Grid.ColumnDefinitions><ColumnDefinition Width="16"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Grid.RowDefinitions><RowDefinition Height="Auto"/><RowDefinition Height="Auto"/></Grid.RowDefinitions>{}<Grid Grid.Row="1" Grid.Column="1">{}</Grid><StackPanel Grid.Column="2" Grid.RowSpan="2" Orientation="Horizontal" Spacing="8" VerticalAlignment="Center">{actions}</StackPanel></Grid></Border>"#,
            Self::first_line(&icon, &bar.title, 13, 12),
            Self::text(&bar.message, 13, "$text2$", r#"TextWrapping="Wrap""#)
        )
    }

    /// A 16 px icon before a bold title that may wrap, the icon in the middle of the title's first
    /// line; spanning the first two columns of a grid whose first column is 16 wide, so the
    /// lines under the title line up with it.
    pub fn first_line(icon: &str, title: &str, size: u32, spacing: u32) -> String {
        format!(
            r#"<Grid Grid.ColumnSpan="2" ColumnSpacing="{spacing}" VerticalAlignment="Top"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/></Grid.ColumnDefinitions><Grid Height="{}" VerticalAlignment="Top">{icon}</Grid><Grid Grid.Column="1">{}</Grid></Grid>"#,
            Align::ROW,
            Align::line_height(size),
            Self::line(
                title,
                size,
                "$text$",
                r#" FontWeight="SemiBold" TextWrapping="Wrap""#
            )
        )
    }

    /// An alert row; compact rows are one tinted line.
    pub fn card(content: &str, padding: &str) -> String {
        format!(
            r#"<Border Background="$card$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8" Padding="{padding}">{content}</Border>"#
        )
    }

    /// A settings card: icon or lead, title, description, an error, the control on the right. The
    /// frame and the texts follow the control's field as it is typed, so they are live parts.
    #[allow(clippy::too_many_arguments)]
    pub fn scard(
        out: &mut Out,
        lead: &str,
        title: &str,
        description: &str,
        error: Option<&str>,
        control: &str,
        bad: bool,
    ) -> String {
        let frame = Self::frame(out, bad);
        let texts = out.live(
            &format!(
                r#"Grid.Column="1" VerticalAlignment="Center"{}"#,
                Align::BLOCK
            ),
            |_| {
                format!(
                    r#"<StackPanel Spacing="2">{}{}{}</StackPanel>"#,
                    Self::text(title, 13, "$text$", ""),
                    Self::description(description),
                    Self::error(error)
                )
            },
        );
        format!(
            r#"<Grid MinHeight="64">{frame}<Grid Margin="1" Padding="16,10" ColumnSpacing="16"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{lead}{texts}<StackPanel Grid.Column="2" Orientation="Horizontal" Spacing="8" VerticalAlignment="Center">{control}</StackPanel></Grid></Grid>"#,
            Align::ROW
        )
    }

    /// A card's frame behind its content, red when `bad`: a live part.
    fn frame(out: &mut Out, bad: bool) -> String {
        out.live("", |_| {
            format!(
                r#"<Border Background="$card$" BorderBrush="{}" BorderThickness="1" CornerRadius="6"/>"#,
                if bad { "$errBorder$" } else { "$border$" }
            )
        })
    }

    /// A card's or row's description; none when empty, so the title alone is centred.
    fn description(text: &str) -> String {
        if text.is_empty() {
            String::new()
        } else {
            Self::text(text, 12, "$text2$", r#"TextWrapping="Wrap""#)
        }
    }

    fn error(error: Option<&str>) -> String {
        error
            .map(|e| Self::text(e, 12, "$danger$", r#"TextWrapping="Wrap""#))
            .unwrap_or_default()
    }

    /// Our own expander (WinUI's is unavailable): a header Button with a chevron, and its body.
    /// The frame and the header's badge and summary follow the body's fields as they are typed,
    /// so they are live parts.
    #[allow(clippy::too_many_arguments)]
    pub fn expander(
        out: &mut Out,
        id: &str,
        lead: &str,
        title: &str,
        description: &str,
        badge: &str,
        summary: &str,
        open: bool,
        bad: bool,
        body: &str,
    ) -> String {
        let name = out.bind(Event::Click, Command::Toggle(id.into()));
        let frame = Self::frame(out, bad);
        let head = out.live("", |_| {
            format!(
                r#"<Grid ColumnSpacing="16"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><Grid Width="16" VerticalAlignment="Center">{lead}</Grid><StackPanel Grid.Column="1" Spacing="2" VerticalAlignment="Center"{}>{}{}</StackPanel><Grid Grid.Column="2">{badge}</Grid><Grid Grid.Column="3">{}</Grid><Grid Grid.Column="4">{}</Grid></Grid>"#,
                Align::ROW,
                Align::BLOCK,
                Self::text(title, 13, "$text$", ""),
                Self::description(description),
                Self::line(summary, 12, "$text2$", ""),
                Self::icon(if open { "chevu" } else { "chev" }, 14, "$text2$")
            )
        });
        let body = if open {
            format!(
                r#"<Border BorderBrush="$divider$" BorderThickness="0,1,0,0"><StackPanel>{body}</StackPanel></Border>"#
            )
        } else {
            String::new()
        };
        format!(
            r#"<Grid>{frame}<StackPanel Margin="1"><Button x:Name="{name}" HorizontalAlignment="Stretch" HorizontalContentAlignment="Stretch" Background="Transparent" BorderThickness="0" MinHeight="64" Padding="16,10" CornerRadius="6">{head}</Button>{body}</StackPanel></Grid>"#
        )
    }

    /// A row of an expander's body. The stripe and the texts beside the control follow its field
    /// as it is typed, so they are a live part.
    #[allow(clippy::too_many_arguments)]
    pub fn xrow(
        out: &mut Out,
        label: &str,
        description: &str,
        control: &str,
        changed: bool,
        bad: bool,
        error: Option<&str>,
        first: bool,
    ) -> String {
        let label = Self::text(label, 13, "$text$", "");
        Self::xrow_with(
            out,
            &label,
            description,
            control,
            changed,
            bad,
            error,
            first,
        )
    }

    /// A row whose label is markup of its own (an icon before it, another font).
    #[allow(clippy::too_many_arguments)]
    pub fn xrow_with(
        out: &mut Out,
        label: &str,
        description: &str,
        control: &str,
        changed: bool,
        bad: bool,
        error: Option<&str>,
        first: bool,
    ) -> String {
        let stripe = if bad {
            r#"<Border Width="3" HorizontalAlignment="Left" Background="$danger$"/>"#
        } else if changed {
            r#"<Border Width="3" HorizontalAlignment="Left" Background="$accent$"/>"#
        } else {
            ""
        };
        let texts = out.live(Align::BLOCK, |_| {
            format!(
                r#"<Grid>{stripe}<StackPanel Spacing="2" VerticalAlignment="Center" Margin="48,6,0,6">{label}{}{}</StackPanel></Grid>"#,
                Self::description(description),
                Self::error(error)
            )
        });
        format!(
            r#"<Grid BorderBrush="$divider$" BorderThickness="0,{},0,0"><Grid MinHeight="48" Padding="0,0,16,0" ColumnSpacing="12"{}><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{texts}<StackPanel Grid.Column="1" Orientation="Horizontal" Spacing="8" VerticalAlignment="Center" Margin="0,6">{control}</StackPanel></Grid></Grid>"#,
            if first { 0 } else { 1 },
            Align::ROW
        )
    }

    pub fn apply_bar(
        out: &mut Out,
        bar: &ApplyBarView,
        revert: Command,
        apply: Command,
        label: &str,
    ) -> String {
        let revert = Self::button(out, "Revert", None, Look::Default, false, true, revert, "");
        let apply = Self::button(
            out,
            label,
            None,
            Look::Primary,
            false,
            bar.can_apply,
            apply,
            "",
        );
        format!(
            r#"<Border Background="$layer$" BorderBrush="$divider$" BorderThickness="0,1,0,0" Padding="24,12"><Grid ColumnSpacing="8"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{revert}</Grid><Grid Grid.Column="3">{apply}</Grid></Grid></Border>"#,
            Align::ROW,
            Self::dot(if bar.bad { Dot::Error } else { Dot::Busy }),
            Self::line(&bar.text, 12, "$text$", r#" TextWrapping="Wrap""#)
        )
    }

    /// An empty or special state: a circle with an icon, a title and a quiet text.
    pub fn empty(icon: Option<(&str, &str)>, title: &str, description: &str) -> String {
        let circle = icon
            .map(|(name, color)| {
                format!(
                    r#"<Border Width="48" Height="48" CornerRadius="24" Background="$subtle$" HorizontalAlignment="Center">{}</Border>"#,
                    Self::icon(name, 22, color)
                )
            })
            .unwrap_or_default();
        let title = if title.is_empty() {
            String::new()
        } else {
            Self::text(
                title,
                16,
                "$text$",
                r#"FontWeight="SemiBold" HorizontalAlignment="Center""#,
            )
        };
        format!(
            r#"<StackPanel Spacing="8" Padding="24,32" HorizontalAlignment="Center">{circle}{title}{}</StackPanel>"#,
            Self::text(
                description,
                13,
                "$text2$",
                r#"TextWrapping="Wrap" TextAlignment="Center" MaxWidth="520" HorizontalAlignment="Center""#
            )
        )
    }

    /// A table row: cells in columns of `widths` (`140`, `*`, `1.6*`).
    pub fn row(widths: &[&str], cells: &[String], background: &str, padding: &str) -> String {
        let columns: String = widths
            .iter()
            .map(|w| format!(r#"<ColumnDefinition Width="{w}"/>"#))
            .collect();
        let cells: String = cells
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                format!(r#"<Grid Grid.Column="{i}" VerticalAlignment="Center">{cell}</Grid>"#)
            })
            .collect();
        format!(
            r#"<Grid MinHeight="32" Padding="{padding}" ColumnSpacing="12" CornerRadius="4" Background="{background}"{}><Grid.ColumnDefinitions>{columns}</Grid.ColumnDefinitions>{cells}</Grid>"#,
            Align::ROW
        )
    }

    /// A table's header row.
    pub fn table_header(widths: &[&str], labels: &[&str], padding: &str) -> String {
        let cells: Vec<String> = labels
            .iter()
            .map(|l| Self::line(l, 12, "$text3$", ""))
            .collect();
        format!(
            r#"<Border BorderBrush="$divider$" BorderThickness="0,0,0,1" Height="30">{}</Border>"#,
            Self::row(widths, &cells, "Transparent", padding)
                .replace(r#"MinHeight="32""#, r#"MinHeight="29""#)
        )
    }

    /// A table's footer: an info icon and a line.
    pub fn table_footer(text: &str, end: &str) -> String {
        format!(
            r#"<Border BorderBrush="$divider$" BorderThickness="0,1,0,0" MinHeight="34" Padding="20,0"><Grid ColumnSpacing="8"{}><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{}<Grid Grid.Column="1">{}</Grid><Grid Grid.Column="2">{}</Grid></Grid></Border>"#,
            Align::ROW,
            Self::icon("info", 14, "$text2$"),
            Self::line(text, 12, "$text2$", ""),
            Self::line(end, 12, "$text3$", "")
        )
    }
}
