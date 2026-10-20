//! `SplitWire.Window.exe`: the window, in system XAML in a XAML island through the
//! hand-written WinRT ABI (TaskbarMetrics' approach), no third-party libraries and no WebView, and
//! the tray icon, in one process per session sharing one session and one transport. The view
//! models are `presentation`'s; this module renders them, maps events to commands and runs the
//! frame loop.

mod alignment;
mod app;
mod app_icon;
mod arguments;
mod bound;
mod dropped;
mod error_info;
mod events;
mod file_picker;
mod files;
mod focus;
mod hidden_host;
mod island;
mod layout;
mod link;
pub mod markup;
mod menu_previews;
mod native_window;
mod nav_drag;
mod pictures;
mod png;
mod renderer;
mod single_instance;
mod snapshot;
mod startup;
mod system;
mod tray;
mod typing;
mod verify;
mod winrt;
mod xaml;

pub use arguments::Arguments;

use app::WindowApp;
use island::{Island, XamlRuntime};
use native_window::NativeWindow;
use pictures::PagePictures;
use single_instance::{Claim, SingleInstance};
use system::ParentConsole;
use verify::PageVerifier;

use crate::platform::com::{Apartment, ComResult, E_FAIL};
use crate::platform::library::AppLibrary;

pub struct WindowProgram;

impl WindowProgram {
    /// Runs the window or a check; the process exit code.
    pub fn run(arguments: Arguments) -> i32 {
        // Nothing this process loads later looks in the current folder or the PATH.
        let _ = AppLibrary::restrict_default_search();
        if arguments.help || arguments.problem.is_some() {
            ParentConsole::attach();
            if let Some(problem) = &arguments.problem {
                eprintln!("{problem}");
            }
            println!("{}", Arguments::USAGE);
            return if arguments.help { 0 } else { 2 };
        }
        if arguments.verify_pages {
            ParentConsole::attach();
            return match PageVerifier::run() {
                Ok(totals) => {
                    println!(
                        "Pages verified: {} states, {} regions loaded, {} bound elements found.",
                        totals.states, totals.regions, totals.elements
                    );
                    println!(
                        "Alignment verified: {} rows, every mark within a pixel of its line; {} \
                         profile menus; {} headers clickable all over.",
                        totals.rows, totals.menus, totals.covers
                    );
                    println!(
                        "Typing verified: {} keys typed into {} fields, none of them reloaded.",
                        totals.typed.keys, totals.typed.fields
                    );
                    println!(
                        "Tray verified: {} icons and {} app icon sizes loaded, {} menus with {} \
                         items built.",
                        totals.tray_icons, totals.app_icons, totals.tray_menus, totals.tray_items
                    );
                    0
                }
                Err(problem) => {
                    eprintln!("{problem}");
                    1
                }
            };
        }
        if let Some(folder) = &arguments.render_pages {
            ParentConsole::attach();
            return match PagePictures::run(folder) {
                Ok(count) => {
                    println!("Pages rendered: {count} pictures in {}.", folder.display());
                    0
                }
                Err(problem) => {
                    eprintln!("{problem}");
                    1
                }
            };
        }
        match Self::window(&arguments) {
            Ok(()) => 0,
            Err(hr) => {
                let folder = system::Folders::local();
                let _ = std::fs::create_dir_all(&folder);
                let _ = std::fs::write(
                    folder.join("SplitWire.Window.error.log"),
                    format!("The window failed: 0x{:08X}\n", hr as u32),
                );
                1
            }
        }
    }

    fn window(arguments: &Arguments) -> ComResult<()> {
        let name = SingleInstance::name(arguments.demo.is_some(), arguments.pipe.as_ref());
        let instance = match SingleInstance::claim(&name) {
            Ok(Claim::First(instance)) => instance,
            // A sign-in start finds the tray there already; any other start shows its window.
            Ok(Claim::Running(other)) => {
                if !arguments.tray {
                    let _ = other.raise();
                }
                return Ok(());
            }
            Err(_) => return Err(E_FAIL),
        };
        let _apartment = Apartment::single_threaded()?;
        let _xaml = XamlRuntime::start()?;
        let (store, settings) = WindowApp::settings(arguments);
        let title = format!("SplitWire v{}", env!("CARGO_PKG_VERSION"));
        let window = NativeWindow::new(&title, settings.placement)?;
        let island = Island::attach(window.raw())?;
        window.attach(&island);
        let mut app = WindowApp::new(arguments, instance, store, settings, &window, &island)?;
        app.start(&window, !arguments.tray || settings.open_window_at_sign_in);
        let result = window.run(&island, || app.tick(&window, &island));
        app.finish();
        drop(app);
        drop(island);
        drop(window);
        NativeWindow::drain();
        result
    }
}
