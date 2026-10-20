//! The dialogs: Remove… and Edit as text, drawn over the window with a scrim like a
//! ContentDialog.

use crate::platform::window::markup::region::{Event, Out};
use crate::platform::window::markup::{Kit, Look};
use crate::presentation::session::DialogView;
use crate::presentation::{Command, FrameView, InputId};

pub struct Dialogs;

impl Dialogs {
    fn frame(out: &mut Out, width: u32, body: &str, foot: &str) -> String {
        let dismiss = out.bind(Event::Click, Command::CloseDialog);
        format!(
            r#"<Grid><Button x:Name="{dismiss}" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Background="$scrim$" BorderThickness="0" CornerRadius="0"><Button.Resources><SolidColorBrush x:Key="ButtonBackgroundPointerOver" Color="$scrim$"/><SolidColorBrush x:Key="ButtonBackgroundPressed" Color="$scrim$"/></Button.Resources></Button><Border Width="{width}" HorizontalAlignment="Center" VerticalAlignment="Center" Background="$layer$" BorderBrush="$border$" BorderThickness="1" CornerRadius="8"><StackPanel><StackPanel Padding="24" Spacing="12">{body}</StackPanel><Border Background="$subtle$" BorderBrush="$divider$" BorderThickness="0,1,0,0" Padding="24" CornerRadius="0,0,8,8"><Grid ColumnSpacing="8"><Grid.ColumnDefinitions><ColumnDefinition/><ColumnDefinition/></Grid.ColumnDefinitions>{foot}</Grid></Border></StackPanel></Border></Grid>"#
        )
    }

    fn wide(button: String) -> String {
        button.replacen("<Button ", r#"<Button HorizontalAlignment="Stretch" "#, 1)
    }

    pub fn markup(out: &mut Out, frame: &FrameView) -> String {
        match &frame.dialog {
            None => String::new(),
            Some(DialogView::Remove(name)) => {
                let remove = Self::wide(Kit::button(
                    out,
                    "Remove",
                    None,
                    Look::Primary,
                    false,
                    true,
                    Command::Remove(name.clone()),
                    "",
                ));
                let cancel = Self::wide(Kit::button(
                    out,
                    "Cancel",
                    None,
                    Look::Default,
                    false,
                    true,
                    Command::CloseDialog,
                    "",
                ));
                Self::frame(
                    out,
                    548,
                    &format!(
                        "{}{}",
                        Kit::text(
                            &format!("Remove {name}?"),
                            20,
                            "$text$",
                            r#"FontWeight="SemiBold""#
                        ),
                        Kit::text(
                            &format!(
                                "{name} is disconnected and its folder is deleted: keys, list and settings. To use it again, import its .conf."
                            ),
                            13,
                            "$text2$",
                            r#"TextWrapping="Wrap""#
                        )
                    ),
                    &format!(r#"{remove}<Grid Grid.Column="1">{cancel}</Grid>"#),
                )
            }
            Some(DialogView::ListText { profile, error }) => {
                let text = frame.input(&InputId::ListDialog);
                let lines = text.lines().count() + 1;
                let editor = Kit::editor(out, InputId::ListDialog, &text, lines, &[], Some(220));
                let save = Self::wide(Kit::button(
                    out,
                    "Save",
                    None,
                    Look::Primary,
                    false,
                    true,
                    Command::SaveListDialog,
                    "",
                ));
                let cancel = Self::wide(Kit::button(
                    out,
                    "Cancel",
                    None,
                    Look::Default,
                    false,
                    true,
                    Command::CloseDialog,
                    "",
                ));
                let error = error
                    .as_ref()
                    .map(|e| Kit::text(e, 12, "$danger$", r#"TextWrapping="Wrap""#))
                    .unwrap_or_default();
                Self::frame(
                    out,
                    620,
                    &format!(
                        r#"{}{}<ScrollViewer MaxHeight="420" VerticalScrollBarVisibility="Auto">{editor}</ScrollViewer>{error}"#,
                        Kit::text(
                            &format!("Edit {profile}’s list as text"),
                            20,
                            "$text$",
                            r#"FontWeight="SemiBold""#
                        ),
                        Kit::text(
                            &format!(
                                "profiles\\{profile}\\routes.txt · one domain, IPv4 address or network per line · # comments. The save is checked against every other profile’s list and refused whole on an exact duplicate."
                            ),
                            12,
                            "$text2$",
                            r#"TextWrapping="Wrap""#
                        )
                    ),
                    &format!(r#"{save}<Grid Grid.Column="1">{cancel}</Grid>"#),
                )
            }
        }
    }
}
