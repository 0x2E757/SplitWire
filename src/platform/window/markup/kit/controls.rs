//! Controls: buttons and links, the switch, the Segmented control, text fields and the editor.

use super::{Kit, Look};
use crate::platform::window::markup::Align;
use crate::platform::window::markup::region::{Event, Out};
use crate::presentation::{Command, InputId};

impl Kit {
    fn look(look: Look, enabled: bool) -> (&'static str, &'static str, &'static str, String) {
        if !enabled {
            return ("$subtle$", "$subtle$", "$disabled$", String::new());
        }
        match look {
            Look::Default => ("$ctrl$", "$ctrlBorder$", "$text$", String::new()),
            Look::Danger => ("$ctrl$", "$ctrlBorder$", "$danger$", String::new()),
            Look::Subtle => ("Transparent", "Transparent", "$text$", String::new()),
            Look::Primary => (
                "$accent$",
                "$accent$",
                "$onAccent$",
                r#"<Button.Resources><SolidColorBrush x:Key="ButtonBackgroundPointerOver" Color="$accentHover$"/><SolidColorBrush x:Key="ButtonBackgroundPressed" Color="$accentPress$"/><SolidColorBrush x:Key="ButtonBorderBrushPointerOver" Color="$accentHover$"/><SolidColorBrush x:Key="ButtonBorderBrushPressed" Color="$accentPress$"/><SolidColorBrush x:Key="ButtonForegroundPointerOver" Color="$onAccent$"/><SolidColorBrush x:Key="ButtonForegroundPressed" Color="$onAccent$"/></Button.Resources>"#.into(),
            ),
        }
    }

    /// A Button with an optional icon and label; `small` is the 26 px size.
    #[allow(clippy::too_many_arguments)]
    pub fn button(
        out: &mut Out,
        label: &str,
        icon: Option<&str>,
        look: Look,
        small: bool,
        enabled: bool,
        command: Command,
        tip: &str,
    ) -> String {
        let name = out.bind(Event::Click, command);
        Self::button_named(&name, label, icon, look, small, enabled, tip, "")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn button_named(
        name: &str,
        label: &str,
        icon: Option<&str>,
        look: Look,
        small: bool,
        enabled: bool,
        tip: &str,
        inner: &str,
    ) -> String {
        let (background, border, color, resources) = Self::look(look, enabled);
        let (height, padding, font, icon_size) = match (small, label.is_empty()) {
            (true, true) => ("26", "0", 12, 14),
            (true, false) => ("26", "8,0", 12, 14),
            (false, true) => ("32", "0", 13, 16),
            (false, false) => ("32", "12,0", 13, 16),
        };
        let width = if label.is_empty() {
            format!(r#" Width="{height}""#)
        } else {
            String::new()
        };
        let weight = if look == Look::Primary && enabled {
            r#" FontWeight="SemiBold""#
        } else {
            ""
        };
        let icon = icon
            .map(|i| Self::icon(i, icon_size, color))
            .unwrap_or_default();
        let label = if label.is_empty() {
            String::new()
        } else {
            format!(
                r#"<TextBlock Text="{}" FontSize="{font}" Foreground="{color}"{weight}{} VerticalAlignment="Center"/>"#,
                Self::esc(label),
                Align::line(font)
            )
        };
        let tip = if tip.is_empty() {
            String::new()
        } else {
            format!(r#" ToolTipService.ToolTip="{}""#, Self::esc(tip))
        };
        format!(
            r#"<Button x:Name="{name}" Height="{height}" MinHeight="{height}" Padding="{padding}"{width} Background="{background}" BorderBrush="{border}" Foreground="{color}" IsEnabled="{}"{tip}{row}>{resources}<StackPanel Orientation="Horizontal" Spacing="{}">{icon}{label}</StackPanel>{inner}</Button>"#,
            if enabled { "True" } else { "False" },
            if small { 6 } else { 8 },
            row = Align::ROW
        )
    }

    /// The × that removes a chip: an 18 px round button inside the chip's 24 px.
    pub fn chip_remove(out: &mut Out, command: Command) -> String {
        let name = out.bind(Event::Click, command);
        format!(
            r#"<Button x:Name="{name}" Width="18" Height="18" MinHeight="18" Padding="0" CornerRadius="9" Background="Transparent" BorderThickness="0" ToolTipService.ToolTip="Remove"{}>{}</Button>"#,
            Align::ROW,
            Self::icon("x", 12, "$text2$")
        )
    }

    /// A link: accent text without a frame.
    pub fn link(out: &mut Out, label: &str, command: Command) -> String {
        let name = out.bind(Event::Click, command);
        format!(
            r#"<Button x:Name="{name}" Background="Transparent" BorderThickness="0" Padding="0" MinHeight="0" Height="20" VerticalAlignment="Center"{}><Button.Resources><SolidColorBrush x:Key="ButtonBackgroundPointerOver" Color="Transparent"/><SolidColorBrush x:Key="ButtonBackgroundPressed" Color="Transparent"/></Button.Resources><TextBlock Text="{}" FontSize="12" Foreground="$accent$"{}/></Button>"#,
            Align::ROW,
            Self::esc(label),
            Align::line(12)
        )
    }

    /// A Connect switch or any setting switch; `label` goes to its left.
    pub fn toggle(
        out: &mut Out,
        on: bool,
        enabled: bool,
        command: Command,
        label: &str,
        tip: &str,
    ) -> String {
        let name = out.bind(Event::Toggled, command);
        let width = if label.is_empty() {
            r#" Width="40""#
        } else {
            ""
        };
        let tip = if tip.is_empty() {
            String::new()
        } else {
            format!(r#" ToolTipService.ToolTip="{}""#, Self::esc(tip))
        };
        format!(
            r#"<ToggleSwitch x:Name="{name}" IsOn="{}" IsEnabled="{}" OnContent="{l}" OffContent="{l}"{width} VerticalAlignment="Center"{tip}/>"#,
            if on { "True" } else { "False" },
            if enabled { "True" } else { "False" },
            l = Self::esc(label)
        )
    }

    /// The Segmented control: `(label, selected, command)`.
    pub fn segmented(out: &mut Out, items: Vec<(String, bool, Command)>) -> String {
        let buttons: String = items
            .into_iter()
            .map(|(label, selected, command)| {
                let name = out.bind(Event::Click, command);
                let (background, border, color, weight) = if selected {
                    ("$card$", "$border$", "$text$", "SemiBold")
                } else {
                    ("Transparent", "Transparent", "$text2$", "Normal")
                };
                format!(
                    r#"<Button x:Name="{name}" Height="24" MinHeight="24" Padding="10,0" CornerRadius="4" Background="{background}" BorderBrush="{border}"{}><TextBlock Text="{}" FontSize="12" FontWeight="{weight}" Foreground="{color}"{} VerticalAlignment="Center"/></Button>"#,
                    Align::ROW,
                    Self::esc(&label),
                    Align::line(12)
                )
            })
            .collect();
        format!(
            r#"<Border Background="$subtle$" BorderBrush="$divider$" BorderThickness="1" CornerRadius="6" Padding="2" VerticalAlignment="Center" HorizontalAlignment="Left"{}><StackPanel Orientation="Horizontal" Spacing="2">{buttons}</StackPanel></Border>"#,
            Align::ROW
        )
    }

    /// A text field: a frame with an optional icon and unit around a TextBox. Enter submits it.
    /// The frame turns red with `bad` as the text is typed, so it is a live part. The placeholder
    /// takes the font of the typed text, whose line is taller than a TextBlock's, so both sit on
    /// one baseline in the middle of the frame (the TextBox style's padding).
    #[allow(clippy::too_many_arguments)]
    pub fn field(
        out: &mut Out,
        id: InputId,
        value: &str,
        placeholder: &str,
        icon: Option<&str>,
        width: Option<u32>,
        unit: Option<&str>,
        bad: bool,
    ) -> String {
        let (name, token) = out.input(id.clone(), value);
        out.also(&name, Event::Enter, Command::Submit(id));
        let width = width
            .map(|w| format!(r#" Width="{w}""#))
            .unwrap_or_default();
        let frame = out.live("", |_| {
            format!(
                r#"<Grid><Border Background="$ctrl$" BorderBrush="{}" BorderThickness="1" CornerRadius="4"/><Border Height="1" VerticalAlignment="Bottom" Margin="3,0" Background="{}"{}/></Grid>"#,
                if bad { "$danger$" } else { "$ctrlBorder$" },
                if bad { "$danger$" } else { "$textBoxBottom$" },
                Align::FREE
            )
        });
        let icon = icon
            .map(|i| Self::icon(i, 14, "$text3$"))
            .unwrap_or_default();
        let unit = unit
            .map(|u| Self::line(u, 12, "$text3$", r#"Grid.Column="2""#))
            .unwrap_or_default();
        format!(
            r#"<Grid Height="32"{width} VerticalAlignment="Center"{}>{frame}<Grid Padding="10,0" ColumnSpacing="8"><Grid.ColumnDefinitions><ColumnDefinition Width="Auto"/><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions>{icon}<TextBox x:Name="{name}" Grid.Column="1" Text="{token}" PlaceholderText="{}" MinWidth="0" IsSpellCheckEnabled="False"><TextBox.Resources><Style TargetType="TextBlock"><Setter Property="FontFamily" Value="Segoe UI Variable"/></Style></TextBox.Resources></TextBox>{unit}</Grid></Grid>"#,
            Align::ROW,
            Self::esc(placeholder)
        )
    }

    /// A multi-line monospace editor with a line gutter. `AcceptsReturn` comes before `Text`: XAML
    /// sets attributes in order, and a TextBox without it keeps only the first line. The gutter
    /// follows the typed lines, so it is a live part.
    pub fn editor(
        out: &mut Out,
        id: InputId,
        value: &str,
        lines: usize,
        marks: &[(usize, &str)],
        height: Option<u32>,
    ) -> String {
        let (name, token) = out.input(id, value);
        let numbers: String = (1..=lines.max(1))
            .map(|n| {
                let color = marks
                    .iter()
                    .find(|(line, _)| *line == n)
                    .map_or("$text3$", |(_, color)| color);
                format!(r#"<Run Text="{n}" Foreground="{color}"/><LineBreak/>"#)
            })
            .collect();
        let gutter = out.live("", |_| {
            format!(
                r#"<TextBlock FontFamily="Cascadia Mono, Consolas" FontSize="12" TextAlignment="Right" Margin="0,4,10,0" TextTrimming="None">{numbers}</TextBlock>"#
            )
        });
        let height = height
            .map(|h| format!(r#" MinHeight="{h}""#))
            .unwrap_or_default();
        format!(
            r#"<Border Background="$ctrl$" BorderBrush="$ctrlBorder$" BorderThickness="1" CornerRadius="4"{height}><Grid Padding="0,6,0,6"><Grid.ColumnDefinitions><ColumnDefinition Width="40"/><ColumnDefinition/></Grid.ColumnDefinitions>{gutter}<TextBox x:Name="{name}" Grid.Column="1" AcceptsReturn="True" Text="{token}" TextWrapping="NoWrap" FontFamily="Cascadia Mono, Consolas" FontSize="12" Padding="0,4,8,4" VerticalAlignment="Stretch" IsSpellCheckEnabled="False" ScrollViewer.HorizontalScrollBarVisibility="Auto"/></Grid></Border>"#
        )
    }
}
