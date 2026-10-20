//! Settings: Profiles, All profiles (`splitwire.conf`), This app, About.

use super::{PageContext, PageMarkup};
use crate::platform::window::markup::region::Out;
use crate::platform::window::markup::{Kit, Look, Tone};
use crate::presentation::pages::{Badge, ExpanderView, FieldView, SettingsPageView};
use crate::presentation::{Command, InputId, Page, ThemeChoice};

fn badge(badge: Option<Badge>) -> String {
    match badge {
        Some(Badge::Problem) => Kit::badge("Problem", Tone::Error),
        Some(Badge::Changed) => Kit::badge("Changed", Tone::Accent),
        None => String::new(),
    }
}

fn expander(out: &mut Out, view: &ExpanderView, lead: &str, body: &str) -> String {
    Kit::expander(
        out,
        &view.id,
        lead,
        &view.title,
        &view.description,
        &badge(view.badge),
        &view.summary,
        view.open,
        view.badge == Some(Badge::Problem),
        body,
    )
}

#[allow(clippy::too_many_arguments)]
fn field_row(
    out: &mut Out,
    field: &FieldView,
    label: &str,
    description: &str,
    width: u32,
    unit: Option<&str>,
    first: bool,
    after: &str,
) -> String {
    // The fix follows the typed value.
    let mut control = out.live(r#"VerticalAlignment="Center""#, |part| {
        field
            .fix
            .as_ref()
            .map(|fix| {
                let button = Kit::button(
                    part,
                    &format!("Use {fix}"),
                    None,
                    Look::Default,
                    true,
                    true,
                    Command::SettingUse(field.field.clone(), fix.clone()),
                    "",
                );
                format!(r#"<Grid Margin="0,0,8,0">{button}</Grid>"#)
            })
            .unwrap_or_default()
    });
    control.push_str(&Kit::field(
        out,
        InputId::Setting(field.field.clone()),
        &field.value,
        &field.placeholder,
        None,
        Some(width),
        unit,
        field.problem.is_some(),
    ));
    control.push_str(after);
    // Without spacing: the live parts around the field may be empty.
    let control = format!(r#"<StackPanel Orientation="Horizontal">{control}</StackPanel>"#);
    Kit::xrow(
        out,
        label,
        description,
        &control,
        field.changed,
        field.problem.is_some(),
        field.problem.as_deref(),
        first,
    )
}

fn group_title(title: &str, caption: &str) -> String {
    Kit::titled(
        title,
        14,
        caption,
        "$text3$",
        r#" HorizontalAlignment="Left" Margin="0,14,0,6""#,
    )
}

pub fn page(context: &PageContext, view: &SettingsPageView, key: String) -> PageMarkup {
    let mut out = Out::default();
    let header = context.header(
        &mut out,
        "Settings",
        Some("Global settings and every profile’s overrides on one page"),
        "",
    );
    let header = out.finish("Header", header);

    let message = view
        .message
        .as_ref()
        .map(|bar| Kit::infobar(Tone::Error, bar, ""))
        .unwrap_or_default();
    let message = Out::default().finish("Message", message);

    let mut out = Out::default();
    let profiles: String = view
        .profiles
        .iter()
        .map(|p| {
            let boot = Kit::toggle(&mut out, p.boot.0, true, Command::SettingToggle(crate::presentation::SettingsEdit::field(p.upstream.field.scope.clone(), "connect_at_boot")), "", "");
            // The link follows the typed MTU's problem.
            let open_link = out.live(r#"VerticalAlignment="Center""#, |part| {
                if p.mtu.problem.is_some() {
                    let link = Kit::link(part, &format!("Open {}", p.name), Command::Go(Page::Profile(p.name.clone())));
                    format!(r#"<Grid Margin="8,0,0,0">{link}</Grid>"#)
                } else {
                    String::new()
                }
            });
            let open_page = Kit::button(&mut out, "Open page", None, Look::Default, true, true, Command::Go(Page::Profile(p.name.clone())), "");
            let replace = Kit::button(&mut out, "Replace .conf…", Some("import"), Look::Default, true, true, Command::PickFiles { replace: Some(p.name.clone()) }, "");
            let remove = Kit::button(&mut out, "Remove", None, Look::Danger, true, true, Command::AskRemove(p.name.clone()), "");
            let body = format!(
                "{}{}{}{}",
                Kit::xrow(&mut out, "Connect at boot", "If this profile was connected when the service stopped, connect it again when the service starts, before anyone signs in", &boot, p.boot.1, false, None, true),
                field_row(&mut out, &p.upstream, "Upstream DNS", "Servers for this profile’s listed names, asked through its adapter. Empty: the profile’s DNS. 1–4 IPv4[:port]", 220, None, false, ""),
                field_row(&mut out, &p.mtu, "MTU", "Empty: the profile’s MTU; 1280–1500", 140, None, false, &open_link),
                Kit::xrow(&mut out, "Profile", &p.about, &format!("{open_page}{replace}{remove}"), false, false, None, false),
            );
            expander(&mut out, &p.expander, &Kit::swatch(p.colour), &body)
        })
        .collect();
    let import = Kit::button(
        &mut out,
        "Import .conf…",
        Some("import"),
        Look::Default,
        false,
        true,
        Command::Go(Page::Add),
        "",
    );
    let add = Kit::scard(
        &mut out,
        &Kit::icon("plus", 16, "$text2$"),
        "Add a profile",
        "Import one or several WireGuard .conf files",
        None,
        &import,
        false,
    );
    let profiles = out.finish(
        "Profiles",
        format!(
            r#"<StackPanel Spacing="4">{}{profiles}{add}</StackPanel>"#,
            group_title("Profiles", &view.profiles_caption)
        ),
    );

    let mut out = Out::default();
    let chips: String = view
        .bypass
        .iter()
        .map(|name| {
            let remove = Kit::chip_remove(&mut out, Command::BypassRemove(name.clone()));
            format!(
                r#"<Border Height="24" CornerRadius="12" BorderBrush="$border$" BorderThickness="1" Background="$card$" Padding="10,0,3,0" VerticalAlignment="Center" Tag="align:row"><StackPanel Orientation="Horizontal" Spacing="4">{}{remove}</StackPanel></Border>"#,
                Kit::line(name, 12, "$text2$", "")
            )
        })
        .collect();
    let bypass_field = Kit::field(
        &mut out,
        InputId::BypassNew,
        &view.bypass_new,
        "Add a name",
        None,
        Some(160),
        None,
        view.bypass_problem.is_some(),
    );
    let bypass_add = Kit::button(
        &mut out,
        "Add",
        Some("plus"),
        Look::Default,
        true,
        true,
        Command::BypassAdd,
        "",
    );
    let dns_body = format!(
        "{}{}{}",
        Kit::xrow(
            &mut out,
            "DNS proxy",
            "Where Windows sends listed names (NRPT rules point here). Set in splitwire.conf only",
            &Kit::line(&format!("{} · UDP and TCP", view.listen), 13, "$text2$", ""),
            false,
            false,
            None,
            true
        ),
        field_row(
            &mut out,
            &view.fallback,
            "Fallback DNS",
            "Resolves every profile’s endpoint and the bypass names. Empty: the DNS of the default-route interface. 0–4 IPv4",
            240,
            None,
            false,
            ""
        ),
        Kit::xrow(
            &mut out,
            "Bypass names",
            "Resolved through the fallback DNS and never routed, under any profile. Every profile’s endpoint host is included implicitly",
            &format!(
                r#"<StackPanel Orientation="Horizontal" Spacing="6" Tag="align:row">{chips}{bypass_field}{bypass_add}</StackPanel>"#
            ),
            view.bypass_changed,
            false,
            view.bypass_problem.as_deref(),
            false
        )
    );
    let dns = expander(
        &mut out,
        &view.dns,
        &Kit::icon("globe", 16, "$text2$"),
        &dns_body,
    );
    let cache_body = format!(
        "{}{}",
        field_row(
            &mut out,
            &view.max_ttl,
            "Maximum TTL",
            "Cap on cached answers, for every profile; 30–86400 s",
            110,
            Some("s"),
            true,
            ""
        ),
        field_row(
            &mut out,
            &view.idle_expiry,
            "Idle expiry of learned routes",
            "A learned route not looked up for this long is removed; 600–604800 s (86400 = 24 h)",
            110,
            Some("s"),
            false,
            ""
        )
    );
    let cache = expander(
        &mut out,
        &view.cache,
        &Kit::icon("history", 16, "$text2$"),
        &cache_body,
    );
    let all = out.finish(
        "All",
        format!(
            r#"<StackPanel Spacing="4">{}{dns}{cache}</StackPanel>"#,
            group_title("All profiles", "splitwire.conf")
        ),
    );

    let mut out = Out::default();
    let start = Kit::toggle(
        &mut out,
        view.start_at_sign_in,
        true,
        Command::StartAtSignIn(!view.start_at_sign_in),
        "",
        "",
    );
    let open = Kit::toggle(
        &mut out,
        view.open_window_at_sign_in,
        true,
        Command::OpenWindowAtSignIn(!view.open_window_at_sign_in),
        "",
        "",
    );
    let theme = Kit::segmented(
        &mut out,
        ThemeChoice::ALL
            .into_iter()
            .map(|t| (t.label().to_owned(), t == view.theme, Command::Theme(t)))
            .collect(),
    );
    let window_body = format!(
        "{}{}{}",
        Kit::xrow(
            &mut out,
            "Start SplitWire at sign-in",
            "The tray icon starts with Windows; the window opens on demand",
            &start,
            false,
            false,
            None,
            true
        ),
        Kit::xrow(
            &mut out,
            "Open the window at sign-in",
            "Otherwise only the tray icon appears",
            &open,
            false,
            false,
            None,
            false
        ),
        Kit::xrow(
            &mut out,
            "Window theme",
            "“System” follows the Windows app theme",
            &theme,
            false,
            false,
            None,
            false
        )
    );
    let window = expander(
        &mut out,
        &view.window,
        &Kit::icon("layout", 16, "$text2$"),
        &window_body,
    );
    let about_body = format!(
        "{}{}{}",
        Kit::xrow(
            &mut out,
            "wireguard-nt",
            "The WireGuard driver, loaded from the program folder (SHA-256 checked)",
            &Kit::line(&view.wgnt, 13, "$text2$", ""),
            false,
            false,
            None,
            true
        ),
        Kit::xrow(
            &mut out,
            "Service",
            "SplitWire.exe service, LocalSystem",
            &Kit::line(&view.service_since, 13, "$text2$", ""),
            false,
            false,
            None,
            false
        ),
        Kit::xrow(
            &mut out,
            "Data folder",
            "Settings, profiles (encrypted for this machine), lists and splitwire.log; administrators only",
            &format!(
                r#"<TextBlock Text="{}" FontFamily="Cascadia Mono, Consolas" FontSize="12" Foreground="$text2$"{} VerticalAlignment="Center" IsTextSelectionEnabled="True"/>"#,
                Kit::esc(&view.data_folder),
                crate::platform::window::markup::Align::line(12)
            ),
            false,
            false,
            None,
            false
        )
    );
    let about = expander(
        &mut out,
        &view.about,
        &Kit::icon("info", 16, "$text2$"),
        &about_body,
    );
    let app = out.finish(
        "App",
        format!(
            r#"<StackPanel Spacing="4">{}{window}{about}</StackPanel>"#,
            group_title(
                "This app",
                "applies at once · stored for your account, no Apply"
            )
        ),
    );
    PageMarkup {
        key,
        regions: vec![header, message, profiles, all, app],
        grow: None,
    }
}
