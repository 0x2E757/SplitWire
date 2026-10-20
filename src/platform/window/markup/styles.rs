//! The styles every page relies on, as TaskbarMetrics' window: a Button template without the
//! pressed tilt, the 40 × 20 switch of the artboards, text and field brushes. Flyouts live outside
//! the page tree, so their content carries the same styles in its own resources.

pub struct Styles;

impl Styles {
    /// The Button template: the system look's states without the tilt that blurs text.
    const PLAIN_BUTTON: &'static str = r#"<ControlTemplate x:Key="PlainButton" TargetType="Button"><ContentPresenter x:Name="ContentPresenter" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" CornerRadius="{TemplateBinding CornerRadius}" Padding="{TemplateBinding Padding}" Foreground="{TemplateBinding Foreground}" Content="{TemplateBinding Content}" ContentTemplate="{TemplateBinding ContentTemplate}" HorizontalContentAlignment="{TemplateBinding HorizontalContentAlignment}" VerticalContentAlignment="{TemplateBinding VerticalContentAlignment}"><VisualStateManager.VisualStateGroups><VisualStateGroup x:Name="CommonStates"><VisualState x:Name="Normal"/><VisualState x:Name="PointerOver"><VisualState.Setters><Setter Target="ContentPresenter.Background" Value="{ThemeResource ButtonBackgroundPointerOver}"/><Setter Target="ContentPresenter.BorderBrush" Value="{ThemeResource ButtonBorderBrushPointerOver}"/><Setter Target="ContentPresenter.Foreground" Value="{ThemeResource ButtonForegroundPointerOver}"/></VisualState.Setters></VisualState><VisualState x:Name="Pressed"><VisualState.Setters><Setter Target="ContentPresenter.Background" Value="{ThemeResource ButtonBackgroundPressed}"/><Setter Target="ContentPresenter.BorderBrush" Value="{ThemeResource ButtonBorderBrushPressed}"/><Setter Target="ContentPresenter.Foreground" Value="{ThemeResource ButtonForegroundPressed}"/></VisualState.Setters></VisualState><VisualState x:Name="Disabled"><VisualState.Setters><Setter Target="ContentPresenter.Background" Value="{ThemeResource ButtonBackgroundDisabled}"/><Setter Target="ContentPresenter.BorderBrush" Value="{ThemeResource ButtonBorderBrushDisabled}"/><Setter Target="ContentPresenter.Foreground" Value="{ThemeResource ButtonForegroundDisabled}"/></VisualState.Setters></VisualState></VisualStateGroup></VisualStateManager.VisualStateGroups></ContentPresenter></ControlTemplate>"#;

    /// The switch: track 40 × 20, knob 10 off and 12 on, its label 12 before it, in the font and
    /// line of the other texts beside it (a ContentPresenter's text gets no implicit style).
    const TOGGLE_SWITCH: &'static str = r#"<Style TargetType="ToggleSwitch"><Setter Property="MinWidth" Value="0"/><Setter Property="Template"><Setter.Value><ControlTemplate TargetType="ToggleSwitch"><Grid Background="Transparent" Tag="align:row"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition Width="Auto"/></Grid.ColumnDefinitions><VisualStateManager.VisualStateGroups><VisualStateGroup x:Name="CommonStates"><VisualState x:Name="Normal"/><VisualState x:Name="PointerOver"/><VisualState x:Name="Pressed"/><VisualState x:Name="Disabled"><VisualState.Setters><Setter Target="Track.BorderBrush" Value="$disabled$"/><Setter Target="OffKnob.Fill" Value="$disabled$"/><Setter Target="OnKnob.Fill" Value="$card$"/><Setter Target="OffLabel.Foreground" Value="$disabled$"/><Setter Target="OnLabel.Foreground" Value="$disabled$"/></VisualState.Setters></VisualState></VisualStateGroup><VisualStateGroup x:Name="ToggleStates"><VisualState x:Name="Dragging"/><VisualState x:Name="Off"/><VisualState x:Name="On"><VisualState.Setters><Setter Target="Track.Background" Value="$accent$"/><Setter Target="Track.BorderBrush" Value="$accent$"/><Setter Target="OffKnob.Visibility" Value="Collapsed"/><Setter Target="OnKnob.Visibility" Value="Visible"/></VisualState.Setters></VisualState></VisualStateGroup><VisualStateGroup x:Name="ContentStates"><VisualState x:Name="OffContent"/><VisualState x:Name="OnContent"><VisualState.Setters><Setter Target="OffLabel.Opacity" Value="0"/><Setter Target="OnLabel.Opacity" Value="1"/></VisualState.Setters></VisualState></VisualStateGroup></VisualStateManager.VisualStateGroups><Border x:Name="Track" Grid.Column="1" Width="40" Height="20" CornerRadius="10" BorderThickness="1" BorderBrush="$text2$" Background="Transparent" VerticalAlignment="Center"><Grid><Ellipse x:Name="OffKnob" Width="10" Height="10" Margin="4,0,0,0" HorizontalAlignment="Left" Fill="$text2$"/><Ellipse x:Name="OnKnob" Width="12" Height="12" Margin="23,0,0,0" HorizontalAlignment="Left" Fill="$onAccent$" Visibility="Collapsed"/></Grid></Border><ContentPresenter x:Name="OffLabel" Content="{TemplateBinding OffContent}" Foreground="$text$" FontFamily="Segoe UI Variable Text" FontSize="13" LineHeight="18" LineStackingStrategy="BlockLineHeight" Margin="0,0,12,0" HorizontalAlignment="Right" VerticalAlignment="Center"/><ContentPresenter x:Name="OnLabel" Content="{TemplateBinding OnContent}" Foreground="$text$" FontFamily="Segoe UI Variable Text" FontSize="13" LineHeight="18" LineStackingStrategy="BlockLineHeight" Margin="0,0,12,0" HorizontalAlignment="Right" VerticalAlignment="Center" Opacity="0"/><Thumb x:Name="SwitchThumb" Grid.ColumnSpan="2" IsTabStop="False"><Thumb.Template><ControlTemplate TargetType="Thumb"><Rectangle Fill="Transparent"/></ControlTemplate></Thumb.Template></Thumb></Grid></ControlTemplate></Setter.Value></Setter></Style>"#;

    /// Implicit styles and the brushes the system templates read. A TextBox has no 32 px floor
    /// (`TextControlThemeMinHeight`) and 2 px under its line, whose baseline is lower than a
    /// TextBlock's: centred in a field's frame, its capitals are in the frame's middle.
    const COMMON: &'static str = r#"<Style TargetType="TextBlock"><Setter Property="Foreground" Value="$text$"/><Setter Property="FontFamily" Value="Segoe UI Variable Text"/><Setter Property="FontSize" Value="13"/><Setter Property="TextTrimming" Value="CharacterEllipsis"/></Style>
<Style TargetType="Button"><Setter Property="Template" Value="{StaticResource PlainButton}"/><Setter Property="CornerRadius" Value="4"/><Setter Property="MinHeight" Value="32"/><Setter Property="Padding" Value="12,0"/><Setter Property="FontSize" Value="13"/><Setter Property="Background" Value="$ctrl$"/><Setter Property="Foreground" Value="$text$"/><Setter Property="BorderBrush" Value="$ctrlBorder$"/><Setter Property="BorderThickness" Value="1"/><Setter Property="VerticalAlignment" Value="Center"/><Setter Property="HorizontalContentAlignment" Value="Center"/><Setter Property="VerticalContentAlignment" Value="Center"/></Style>
<Style TargetType="TextBox"><Setter Property="FontSize" Value="13"/><Setter Property="FontFamily" Value="Segoe UI Variable"/><Setter Property="MinHeight" Value="0"/><Setter Property="Padding" Value="0,0,0,2"/><Setter Property="BorderThickness" Value="0"/><Setter Property="Background" Value="Transparent"/><Setter Property="VerticalAlignment" Value="Center"/></Style>
<SolidColorBrush x:Key="ButtonBackgroundPointerOver" Color="$ctrlHover$"/>
<SolidColorBrush x:Key="ButtonBackgroundPressed" Color="$ctrlPress$"/>
<SolidColorBrush x:Key="ButtonBackgroundDisabled" Color="$subtle$"/>
<SolidColorBrush x:Key="ButtonForegroundPointerOver" Color="$text$"/>
<SolidColorBrush x:Key="ButtonForegroundPressed" Color="$text2$"/>
<SolidColorBrush x:Key="ButtonForegroundDisabled" Color="$disabled$"/>
<SolidColorBrush x:Key="ButtonBorderBrushPointerOver" Color="$ctrlBorder$"/>
<SolidColorBrush x:Key="ButtonBorderBrushPressed" Color="$ctrlBorder$"/>
<SolidColorBrush x:Key="ButtonBorderBrushDisabled" Color="$subtle$"/>
<x:Double x:Key="TextControlThemeMinHeight">0</x:Double>
<SolidColorBrush x:Key="TextControlBackground" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBackgroundPointerOver" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBackgroundFocused" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBackgroundDisabled" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBorderBrush" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBorderBrushPointerOver" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBorderBrushFocused" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlBorderBrushDisabled" Color="Transparent"/>
<SolidColorBrush x:Key="TextControlForeground" Color="$text$"/>
<SolidColorBrush x:Key="TextControlForegroundPointerOver" Color="$text$"/>
<SolidColorBrush x:Key="TextControlForegroundFocused" Color="$text$"/>
<SolidColorBrush x:Key="TextControlForegroundDisabled" Color="$disabled$"/>
<SolidColorBrush x:Key="TextControlPlaceholderForeground" Color="$text3$"/>
<SolidColorBrush x:Key="TextControlPlaceholderForegroundPointerOver" Color="$text3$"/>
<SolidColorBrush x:Key="TextControlPlaceholderForegroundFocused" Color="$text3$"/>
<SolidColorBrush x:Key="TextControlPlaceholderForegroundDisabled" Color="$disabled$"/>
<SolidColorBrush x:Key="TextControlButtonForeground" Color="$text2$"/>
<SolidColorBrush x:Key="TextControlSelectionHighlightColor" Color="$accent$"/>
<SolidColorBrush x:Key="ToolTipBackground" Color="$card$"/>
<SolidColorBrush x:Key="ToolTipForeground" Color="$text$"/>
<SolidColorBrush x:Key="ToolTipBorderBrush" Color="$border$"/>
<SolidColorBrush x:Key="ScrollBarThumbFill" Color="$text3$"/>"#;

    /// The window root's resources.
    pub fn resources() -> String {
        format!(
            "{}{}{}",
            Self::PLAIN_BUTTON,
            Self::COMMON,
            Self::TOGGLE_SWITCH
        )
    }

    /// The resources a flyout's content repeats.
    pub fn flyout_resources() -> String {
        format!("{}{}", Self::PLAIN_BUTTON, Self::COMMON)
    }

    /// The presenter style of a Flyout: a card with the window's colours.
    pub fn flyout_presenter(padding: u32, width: Option<u32>) -> String {
        let width = width
            .map(|w| format!(r#"<Setter Property="MinWidth" Value="{w}"/><Setter Property="MaxWidth" Value="{w}"/>"#))
            .unwrap_or_default();
        format!(
            r#"<Flyout.FlyoutPresenterStyle><Style TargetType="FlyoutPresenter"><Setter Property="RequestedTheme" Value="$theme$"/><Setter Property="Background" Value="$card$"/><Setter Property="BorderBrush" Value="$border$"/><Setter Property="BorderThickness" Value="1"/><Setter Property="Padding" Value="{padding}"/><Setter Property="CornerRadius" Value="8"/>{width}<Setter Property="ScrollViewer.VerticalScrollBarVisibility" Value="Auto"/></Style></Flyout.FlyoutPresenterStyle>"#
        )
    }

    /// The presenter style of a MenuFlyout: a card at least `width` wide.
    pub fn menu_presenter(width: u32) -> String {
        format!(
            r#"<MenuFlyout.MenuFlyoutPresenterStyle><Style TargetType="MenuFlyoutPresenter"><Setter Property="RequestedTheme" Value="$theme$"/><Setter Property="Background" Value="$card$"/><Setter Property="BorderBrush" Value="$border$"/><Setter Property="BorderThickness" Value="1"/><Setter Property="CornerRadius" Value="8"/><Setter Property="Padding" Value="4"/><Setter Property="MinWidth" Value="{width}"/></Style></MenuFlyout.MenuFlyoutPresenterStyle>"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_define_the_template_before_its_use() {
        let resources = Styles::resources();
        let template = resources.find("x:Key=\"PlainButton\"").unwrap();
        let use_ = resources.find("{StaticResource PlainButton}").unwrap();
        assert!(template < use_);
        assert!(Styles::flyout_resources().contains("TargetType=\"TextBlock\""));
        assert!(Styles::flyout_presenter(4, Some(390)).contains("Value=\"390\""));
    }
}
