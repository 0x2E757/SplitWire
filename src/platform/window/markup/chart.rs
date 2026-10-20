//! The traffic chart on a Grid (TaskbarMetrics' own renderer): the grid and the
//! labels in star rows and columns, the series as stretched Paths, so the chart fills any width
//! without knowing it. Receive, the upper half, is in TaskbarMetrics' chart blue (`total`); Send,
//! the lower half, in the profile's colour.

use super::Kit;
use crate::presentation::ChartView;
use crate::profile::Pin;

pub struct Chart;

impl Chart {
    pub fn markup(view: &ChartView, pin: Pin, height: u32) -> String {
        let n = pin.number();
        let line = |top: bool, color: &str| {
            format!(
                r#"<Border Height="1" VerticalAlignment="{}" Background="{color}"/>"#,
                if top { "Top" } else { "Bottom" }
            )
        };
        let rows = r#"<Grid.RowDefinitions><RowDefinition/><RowDefinition/><RowDefinition/><RowDefinition/></Grid.RowDefinitions>"#;
        let horizontal = format!(
            r#"<Grid>{rows}{}<Grid Grid.Row="1">{}</Grid><Grid Grid.Row="2">{}</Grid><Grid Grid.Row="3">{}{}</Grid></Grid>"#,
            line(true, "$grid$"),
            line(true, "$grid$"),
            line(true, "$border$"),
            line(true, "$grid$"),
            line(false, "$grid$")
        );
        let columns = r#"<Grid.ColumnDefinitions><ColumnDefinition Width="1*"/><ColumnDefinition Width="2*"/><ColumnDefinition Width="2*"/><ColumnDefinition Width="2*"/><ColumnDefinition Width="2*"/><ColumnDefinition Width="1*"/></Grid.ColumnDefinitions>"#;
        let vertical: String = (1..=5)
            .map(|c| {
                format!(r#"<Border Grid.Column="{c}" Width="1" HorizontalAlignment="Left" Background="$grid$"/>"#)
            })
            .collect();
        // TaskbarMetrics' left axis: right-aligned 8 before the plot. A label wider than the
        // 46 px pad runs on into the card's padding instead of being cut ("500 KB/s").
        let label = |text: &str, row: usize, top: bool| {
            format!(
                r#"<TextBlock Grid.Row="{row}" Text="{}" FontSize="11" Foreground="$text3$" TextTrimming="None" TextWrapping="NoWrap" HorizontalAlignment="Right" VerticalAlignment="{}" Margin="{}"/>"#,
                Kit::esc(text),
                if top { "Top" } else { "Bottom" },
                if top { "-40,-8,8,0" } else { "-40,0,8,-8" }
            )
        };
        let y_labels = format!(
            r#"<Grid Grid.Row="1">{rows}{}{}{}{}{}</Grid>"#,
            label(&view.scale_label, 0, true),
            label(&view.half_label, 1, true),
            label("0", 2, true),
            label(&view.half_label, 3, true),
            label(&view.scale_label, 3, false)
        );
        let x_labels: String = view
            .x_labels
            .iter()
            .enumerate()
            .map(|(i, text)| {
                format!(
                    r#"<TextBlock Grid.Column="{}" Grid.ColumnSpan="2" Text="{}" FontSize="11" Foreground="$text3$" HorizontalAlignment="Center" Margin="0,4,0,0"/>"#,
                    i * 2,
                    Kit::esc(text)
                )
            })
            .collect();
        let ten: String = (0..10).map(|_| r#"<ColumnDefinition/>"#).collect();
        let x_axis = format!(
            r#"<Grid Grid.Row="2" Grid.Column="1"><Grid.ColumnDefinitions>{ten}</Grid.ColumnDefinitions>{x_labels}</Grid><TextBlock Grid.Row="2" Grid.Column="1" Grid.ColumnSpan="2" Text="now" FontSize="11" FontWeight="SemiBold" Foreground="$text2$" HorizontalAlignment="Right" Margin="0,4,0,0"/>"#
        );
        let series = view
            .series
            .as_ref()
            .map(|s| {
                format!(
                    r#"<Path Data="{}" Stretch="Fill" Fill="$totalFill$"/><Path Data="{}" Stretch="Fill" Fill="$pin{n}f$"/><Path Data="{}" Stretch="Fill" Stroke="$total$" StrokeThickness="1.25"/><Path Data="{}" Stretch="Fill" Stroke="$pin{n}$" StrokeThickness="1.25"/>"#,
                    s.rx_fill, s.tx_fill, s.rx_line, s.tx_line
                )
            })
            .unwrap_or_default();
        let empty = view
            .empty
            .as_ref()
            .map(|(title, detail)| {
                format!(
                    r#"<StackPanel HorizontalAlignment="Center" VerticalAlignment="Center" Spacing="2">{}{}</StackPanel>"#,
                    Kit::text(title, 13, "$text2$", r#"FontWeight="SemiBold" HorizontalAlignment="Center""#),
                    Kit::text(detail, 11, "$text3$", r#"HorizontalAlignment="Center""#)
                )
            })
            .unwrap_or_default();
        let chip = |text: &str, top: bool| {
            format!(
                r#"<Border Background="$card$" Opacity="0.9" CornerRadius="4" Padding="6,1" Margin="6" HorizontalAlignment="Left" VerticalAlignment="{}">{}</Border>"#,
                if top { "Top" } else { "Bottom" },
                Kit::text(text, 11, "$text2$", r#"FontWeight="SemiBold""#)
            )
        };
        format!(
            r#"<Grid Height="{height}"><Grid.ColumnDefinitions><ColumnDefinition Width="46"/><ColumnDefinition/><ColumnDefinition Width="44"/></Grid.ColumnDefinitions><Grid.RowDefinitions><RowDefinition Height="28"/><RowDefinition/><RowDefinition Height="22"/></Grid.RowDefinitions>{y_labels}<Grid Grid.Row="1" Grid.Column="1">{horizontal}<Grid>{columns}{vertical}</Grid>{series}{empty}{}{}</Grid>{x_axis}</Grid>"#,
            chip("↓ Receive", true),
            chip("↑ Send", false)
        )
    }

    /// A ChartCard: the legend line with the scale note, then the chart. The legend's mark has the
    /// chart's two colours, Receive above Send.
    pub fn card(title: &str, note: &str, view: &ChartView, pin: Pin, height: u32) -> String {
        let n = pin.number();
        Kit::card(
            &format!(
                r#"<StackPanel Spacing="4"><StackPanel Orientation="Horizontal" Spacing="16" Tag="align:row"><StackPanel Orientation="Horizontal" Spacing="6"><StackPanel Spacing="2" VerticalAlignment="Center" Tag="align:block"><Border Width="16" Height="2" Background="$total$"/><Border Width="16" Height="2" Background="$pin{n}$"/></StackPanel>{}</StackPanel>{}</StackPanel>{}</StackPanel>"#,
                Kit::line(title, 12, "$text2$", ""),
                Kit::line(note, 12, "$text3$", ""),
                Self::markup(view, pin, height)
            ),
            "16,10,16,8",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::TrafficSample;
    use crate::presentation::{LocalTime, TrafficChart};

    #[test]
    fn the_chart_stretches_its_series_and_labels_its_axes() {
        let samples: Vec<TrafficSample> = (1..=300)
            .map(|seq| TrafficSample {
                seq,
                rx_bps: 40_000,
                tx_bps: 8_000,
            })
            .collect();
        let time = LocalTime::new(std::time::UNIX_EPOCH, 0);
        let markup = Chart::markup(
            &TrafficChart::of(&samples, &time),
            Pin::new(3).unwrap(),
            150,
        );
        assert!(markup.contains(r#"Stretch="Fill" Fill="$totalFill$""#));
        assert!(markup.contains(r#"Stretch="Fill" Stroke="$total$""#));
        assert!(markup.contains(r#"Stretch="Fill" Fill="$pin3f$""#));
        assert!(markup.contains(r#"Stretch="Fill" Stroke="$pin3$""#));
        assert!(markup.contains(r#"Text="50 KB/s""#));
        assert!(markup.contains(r#"Text="now""#));
        let empty = Chart::markup(
            &TrafficChart::empty("Handshaking…", "waiting", &time),
            Pin::new(1).unwrap(),
            150,
        );
        assert!(empty.contains("Handshaking…") && !empty.contains("<Path"));
    }
}
