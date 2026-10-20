//! The art compared with the pixels the owner approved: SVGs snapped per size, rendered by Edge at
//! 16 × or more and area-averaged down. Every premultiplied channel stays within `TOLERANCE`.

use super::{AppArt, Taskbar, TrayArt};

const TOLERANCE: i32 = 10;
/// Each state's colour on a light and on a dark taskbar.
const INKS: [(&str, u32, u32); 4] = [
    ("connected", 0x1B1B1B, 0xFFFFFF),
    ("warning", 0xC27C0E, 0xFCE100),
    ("error", 0xC42B1C, 0xFF6B5E),
    ("off", 0x8A8A8A, 0x8A8A8A),
];

/// `0xAARRGGBB` as premultiplied red, green, blue and alpha.
fn premultiplied(argb: u32) -> [i32; 4] {
    let alpha = argb >> 24;
    let channel = |shift: u32| (((argb >> shift) & 0xFF) * alpha + 127) / 255;
    [channel(16), channel(8), channel(0), alpha].map(|v| v as i32)
}

fn assert_near(actual: u32, approved: u32, what: impl Fn() -> String) {
    let (actual_channels, approved_channels) = (premultiplied(actual), premultiplied(approved));
    let near = actual_channels
        .iter()
        .zip(approved_channels)
        .all(|(a, b)| (a - b).abs() <= TOLERANCE);
    assert!(near, "{}: {actual:08X}, approved {approved:08X}", what());
}

/// A row of hex numbers `digits` wide.
fn numbers(row: &str, digits: usize) -> impl Iterator<Item = u32> + '_ {
    (0..row.len())
        .step_by(digits)
        .map(move |i| u32::from_str_radix(&row[i..i + digits], 16).expect("hex"))
}

/// `pixels` of a `size` square against `approved` rows of `RRGGBBAA`, sampled from `(first,
/// first)` every `step` pixels.
fn assert_app(size: u32, approved: &[&str], first: usize, step: usize) {
    let pixels = AppArt::pixels(size);
    for (row, line) in approved.iter().enumerate() {
        for (column, rgba) in numbers(line, 8).enumerate() {
            let (x, y) = (first + column * step, first + row * step);
            assert_near(pixels[y * size as usize + x], rgba.rotate_right(8), || {
                format!("the app icon at {size} px, ({x}, {y})")
            });
        }
    }
}

#[test]
fn the_tray_icons_match_the_approved_pixels() {
    assert_eq!(TrayArt::STATES, INKS.map(|(state, _, _)| state));
    let approved: [(u32, &[&str]); 4] = [
        (16, &TRAY_16),
        (20, &TRAY_20),
        (24, &TRAY_24),
        (32, &TRAY_32),
    ];
    for (state, light, dark) in INKS {
        for (taskbar, ink) in [(Taskbar::Light, light), (Taskbar::Dark, dark)] {
            for (size, rows) in approved {
                let pixels = TrayArt::pixels(state, taskbar, size);
                for (y, row) in rows.iter().enumerate() {
                    for (x, alpha) in numbers(row, 2).enumerate() {
                        assert_near(pixels[y * size as usize + x], alpha << 24 | ink, || {
                            format!("{state} on a {taskbar:?} taskbar at {size} px, ({x}, {y})")
                        });
                    }
                }
            }
        }
    }
}

#[test]
fn the_app_icon_matches_the_approved_pixels() {
    assert_app(16, &APP_16, 0, 1);
    assert_app(32, &APP_32, 0, 1);
    assert_app(256, &APP_256, 4, 8);
    let sums = AppArt::pixels(256)
        .iter()
        .fold([0u64; 4], |mut sums, &pixel| {
            let [r, g, b, a] = premultiplied(pixel);
            for (sum, value) in sums.iter_mut().zip([a, r, g, b]) {
                *sum += value as u64;
            }
            sums
        });
    // Half a level per pixel on average.
    for (sum, approved) in sums.into_iter().zip(APP_256_SUMS) {
        assert!(
            sum.abs_diff(approved) <= 256 * 256 / 2,
            "{sums:?}, approved {APP_256_SUMS:?}"
        );
    }
}

/// The tray arrow's alpha at 16 px.
const TRAY_16: [&str; 16] = [
    "00000000000000000000000000000000",
    "00000000808000000000000000000000",
    "00000080ffff80000000000000000000",
    "000080ffffffff800000000000000000",
    "0080ffffffffffff8000003e00000000",
    "00000000ffff00000000009f1c000000",
    "00000000ffff0000000000a68a070000",
    "00000000ffff054a85a1a6a6a6680000",
    "00000000ffff8aa6a6a6a6a6a6680000",
    "00000000ffffa68c330700a68a070000",
    "00000000ffff8b0a0000009f1c000000",
    "00000000ffff33000000003e00000000",
    "00000000ffff07000000000000000000",
    "00000000ffff00000000000000000000",
    "00000000ffff00000000000000000000",
    "00000000000000000000000000000000",
];

/// The tray arrow's alpha at 20 px.
const TRAY_20: [&str; 20] = [
    "0000000000000000000000000000000000000000",
    "0000000000666600000000000000000000000000",
    "0000000039f9f939000000000000000000000000",
    "0000001ae6ffffe5190000000000000000000000",
    "000006c6ffffffffc60600000000000000000000",
    "000099ffffffffffff9900000000000000000000",
    "0000000000ffff00000000000000530000000000",
    "0000000000ffff00000000000000a65300000000",
    "0000000000ffff00000000000000a6a653000000",
    "0000000000ffff0011588aa2a6a6a6a6a6530000",
    "0000000000ffff3b9ea6a6a6a6a6a6a6a6530000",
    "0000000000ffffa5a67128060000a6a653000000",
    "0000000000ffffa64b0000000000a65300000000",
    "0000000000ffff72000000000000530000000000",
    "0000000000ffff28000000000000000000000000",
    "0000000000ffff06000000000000000000000000",
    "0000000000ffff00000000000000000000000000",
    "0000000000ffff00000000000000000000000000",
    "0000000000ffff00000000000000000000000000",
    "0000000000000000000000000000000000000000",
];

/// The tray arrow's alpha at 24 px.
const TRAY_24: [&str; 24] = [
    "000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000000000000000000",
    "00000000000020bf20000000000000000000000000000000",
    "000000000020dfffdf200000000000000000000000000000",
    "0000000020dfffffffdf2000000000000000000000000000",
    "00000020dfffffffffffdf20000000000000000000000000",
    "000020dfffffffffffffffdf200000001500000000000000",
    "000000000000ffffff000000000000009115000000000000",
    "000000000000ffffff00000000000000a691150000000000",
    "000000000000ffffff00000000000000a6a6911500000000",
    "000000000000ffffff00001554829ca5a6a6a69115000000",
    "000000000000ffffff075ea3a6a6a6a6a6a6a6a67c000000",
    "000000000000ffffff87a6a6a6a6a6a6a6a6a69115000000",
    "000000000000ffffffa6a6a67b360e01a6a6911500000000",
    "000000000000ffffffa6a34000000000a691150000000000",
    "000000000000ffffffa64000000000009115000000000000",
    "000000000000ffffff7b0000000000001500000000000000",
    "000000000000ffffff360000000000000000000000000000",
    "000000000000ffffff0e0000000000000000000000000000",
    "000000000000ffffff010000000000000000000000000000",
    "000000000000ffffff000000000000000000000000000000",
    "000000000000ffffff000000000000000000000000000000",
    "000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000000000000000000",
];

/// The tray arrow's alpha at 32 px.
const TRAY_32: [&str; 32] = [
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000007070000000000000000000000000000000000000000000",
    "000000000000000052fdfd520000000000000000000000000000000000000000",
    "0000000000000039f6fffff63900000000000000000000000000000000000000",
    "00000000000024eaffffffffeb24000000000000000000000000000000000000",
    "000000000014daffffffffffffdb140000000000000000000000000000000000",
    "0000000009c6ffffffffffffffffc60900000000000000000000000000000000",
    "00000002adffffffffffffffffffffad02000000000000000000000000000000",
    "00000090ffffffffffffffffffffffff90000000000047000000000000000000",
    "0000000000000000ffffffff00000000000000000000a4310000000000000000",
    "0000000000000000ffffffff00000000000000000000a69e1f00000000000000",
    "0000000000000000ffffffff00000000000000000000a6a69412000000000000",
    "0000000000000000ffffffff00000000000000000000a6a6a686080000000000",
    "0000000000000000ffffffff000000154e7a96a4a6a6a6a6a6a6750200000000",
    "0000000000000000ffffffff00156da4a6a6a6a6a6a6a6a6a6a6a65f00000000",
    "0000000000000000ffffffff3c9ea6a6a6a6a6a6a6a6a6a6a6a6a65f00000000",
    "0000000000000000ffffffffa6a6a6a6a6a6a6a6a6a6a6a6a6a6750200000000",
    "0000000000000000ffffffffa6a6a6a6864418030000a6a6a686080000000000",
    "0000000000000000ffffffffa6a6a042010000000000a6a69412000000000000",
    "0000000000000000ffffffffa6a02800000000000000a69e1f00000000000000",
    "0000000000000000ffffffffa6420000000000000000a4310000000000000000",
    "0000000000000000ffffffff8601000000000000000047000000000000000000",
    "0000000000000000ffffffff4400000000000000000000000000000000000000",
    "0000000000000000ffffffff1900000000000000000000000000000000000000",
    "0000000000000000ffffffff0400000000000000000000000000000000000000",
    "0000000000000000ffffffff0000000000000000000000000000000000000000",
    "0000000000000000ffffffff0000000000000000000000000000000000000000",
    "0000000000000000ffffffff0000000000000000000000000000000000000000",
    "0000000000000000ffffffff0000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "0000000000000000000000000000000000000000000000000000000000000000",
];

/// The app icon at 16 px, `RRGGBBAA`.
const APP_16: [&str; 16] = [
    "00000000fc99305afc9632d6fb9233fdfb8e36fffa8a38fff9863afff9833cfff87f3efff77b40fff67742fff57345fff46e46fdf46b48d6f066495a00000000",
    "fc99305afc9631fffc9234fffb8e36fffa8a38fff9863afff9833cfff87f3efff77b40fff67742fff57345fff46f47fff46b49fff3674bfff2634dfff0604f5a",
    "fc9632d6fc9234fffb8e36fffa8a38fff9863afff9833cfff87f3efff77b40fff67742fff57345fff46f47fff46b49fff3674bfff2634dfff15f4ffff05b51d6",
    "fb9233fdfb8e36fffa8a38fff9863afffa9254fffddfcffff88b58fff67742fff57345fff46f47fff46b49fff3674bfff2634dfff15f4ffff05b51ffef5753fd",
    "fb8e36fffa8a38fff9863afffa9254fffeefe7fffffffffffeeee7fff6845cfff46f47fff46b49fff4735afff2634dfff15f4ffff05b51fff05753ffef5356ff",
    "fa8a38fff9863afffa9254fffeefe7fffffffffffffffffffffffffffeede8fff57d60fff3674bfff9bcb2fff26c5dfff05b51fff05753ffef5356ffee4f58ff",
    "f9863afff9833cfff87f3efff77b40fff67742fffffffffff46f47fff46b49fff3674bfff2634dfffac7c1fff8b8b4fff16561ffef5356ffee4f58ffed4b5aff",
    "f9833cfff87f3efff77b40fff67742fff57345fffffffffff46b49fff4755bfff7a294fff9bdb6fff9c5c2fff9c4c3fff6a7a8ffee4f58ffed4b5affec475cff",
    "f87f3efff77b40fff67742fff57345fff46f47fffffffffff5846dfffac0b7fff59187fff1685ffff9c4c3fff8b5b6ffef5d65ffed4b5affec475cffec445eff",
    "f77b40fff67742fff57345fff46f47fff46b49fffffffffffac0b7fff37467fff05b51fff05753fff8b5b6ffef5d65ffed4b5affec475cffec445effeb4060ff",
    "f67742fff57345fff46f47fff46b49fff3674bfffffffffff69186fff05b51fff05753ffef5356ffef5d65ffed4b5affec475cffec445effeb4060ffea3c62ff",
    "f57345fff46f47fff46b49fff3674bfff2634dfffffffffff2685ffff05753ffef5356ffee4f58ffed4b5affec475cffec445effeb4060ffea3c62ffe93864ff",
    "f46e46fdf46b49fff3674bfff2634dfff15f4ffffffffffff05854ffef5356ffee4f58ffed4b5affec475cffec445effeb4060ffea3c62ffe93864ffe83366fd",
    "f46b48d6f3674bfff2634dfff15f4ffff05b51fff05753ffef5356ffee4f58ffed4b5affec475cffec445effeb4060ffea3c62ffe93864ffe83467ffe73068d6",
    "f066495af2634dfff15f4ffff05b51fff05753ffef5356ffee4f58ffed4b5affec475cffec445effeb4060ffea3c62ffe93864ffe83467ffe73069ffe52d685a",
    "00000000f0604f5af05b51d6ee5753feef5356ffee4f58ffed4b5affec475cffec445effeb4060ffea3c62ffe93864ffe73366fee83068d6e52d685a00000000",
];

/// The app icon at 32 px, `RRGGBBAA`.
const APP_32: [&str; 32] = [
    "000000000000000000000000ff9c311afb973088fc9631d4fb9432f9fc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff36e46f9f46c48d4f36a4988f56b4e1a000000000000000000000000",
    "0000000000000000fc99305afd9830f2fd9631fffc9432fffc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674af2f0664c5a0000000000000000",
    "00000000ff99305afc9830fefd9631fffc9432fffc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff1634dfef3604f5a00000000",
    "ff9c311afd9830f2fd9631fffc9432fffc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efff15e4ff2eb624e1a",
    "fd973187fd9631fffc9432fffc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efff15f4ffff15d50ffef5a5187",
    "fc9631d4fc9432fffc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952d3",
    "fb9432f9fc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9873afff9853bfffa9254fffde0cefff98f56fff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952ffef5752f9",
    "fc9234fffb9035fffb8e36fffb8c37fffa8a38fffa8839fff9873afff9843bfffa9254fffeefe7fffffffffffeefe7fff88b58fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ff",
    "fb9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfffa9254fffeefe7fffffffffffffffffffffffffffeeee7fff7885afff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ff",
    "fb8e36fffb8c37fffa8a38fffa8839fff9863afff9853bfffa9254fffeefe7fffffffffffffffffffffffffffffffffffffffffffeeee8fff6845cfff57146fff56f47fff46d48fff46b49fff3694afff3674bfff4725bfff2634dfff2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ff",
    "fb8c37fffa8a38fffa8839fff9863afff9853bfffa9254fffeefe7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffeede8fff6815efff46d48fff46b49fff3694afff3674bfff2654cfff9bcb2fff36e5dfff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ff",
    "fa8a38fffa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fffffffffffffffffffffffffff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfffac8c1fff9bab3fff26a5ffff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ff",
    "fa8839fff9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fffffffffffffffffffffffffff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efffac7c1fffac6c2fff8b8b4fff16760fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5aff",
    "f9863afff9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fffffffffffffffffffffffffff56f47fff46d48fff46b49fff57d62fff79e8cfff9b5a9fffac3bbfffac8c1fffac7c1fffac6c2fff9c5c2fff9c5c2fff8b6b5fff06362ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bff",
    "f9853bfff9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fffffffffffffffffffffffffff46d48fff57a5bfff9b0a0fffbcac0fffac9c0fffac8c1fffac8c1fffac7c1fffac6c2fff9c5c2fff9c5c2fff9c4c3fff9c3c3fff7a7a8ffee5157ffee4f58ffee4d59ffed4b5affed495bffec485cff",
    "f9833cfff8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67544fff57345fffffffffffffffffffffffffff68b70fffbc6bbfffbcac0fffbc9c0fffac8c1fffac8c1fffac7c1fffac6c2fff9c5c2fff9c5c2fff9c4c3fff9c3c3fff8b5b6fff05f64ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dff",
    "f8813dfff87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fffffffffffffffffffffffffffbc9bffffbcac0fffbc9c0fffbc7bffff7a297fff47c6ffff26558fff05c52fff05952fff9c4c3fff9c3c3fff8b5b6fff05f64ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445eff",
    "f87f3efff77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fffffffffffffffffffffffffffbcac0fffbc9c0fffab9affff37463fff15f4ffff15d50fff05b51fff05952fff05753fff9c3c3fff8b5b6fff05f64ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fff",
    "f77d3ffff77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fffffffffffffffffffffffffffbc9c0fff9b8affff26957fff15f4ffff15d50fff05b51fff05952fff05753ffef5554fff8b5b6fff05f64ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fffeb4060ff",
    "f77b40fff77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fffffffffffffffffffffffffffbc7bffff37363fff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356fff05f64ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fffeb4060ffea3e61ff",
    "f77941fff67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afffffffffffffffffffffffffff7a297fff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fffeb4060ffea3e61ffea3c62ff",
    "f67742fff67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfffffffffffffffffffffffffff47c6ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ff",
    "f67543fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfffffffffffffffffffffffffff16559fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ff",
    "f57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfffffffffffffffffffffffffff05b52fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ff",
    "f57146fff56f47fff46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efffffffffffffffffffffffffff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ff",
    "f36e46f9f46d48fff46b49fff3694afff3674bfff2654cfff2634dfff2614efff15f4ffffffffffffffffffffffffffff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe73267f9",
    "f46c48d4f46b49fff3694afff3674bfff2654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe83268ffe83069d3",
    "f3694987f3694afff3674bfff2654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe83268ffe83069ffe62f6987",
    "f56b441af3674af2f2654cfff2634dfff2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe83268ffe83069ffe62e69f2e1316b1a",
    "00000000f3664c5af1634dfef2614efff15f4ffff15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe83268ffe83069ffe62e69fee82d6b5a00000000",
    "0000000000000000f05f4e5bf15e4ff2f15d50fff05b51fff05952fff05753ffef5554ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe83268ffe83069ffe62e69f2e82d6b5a0000000000000000",
    "000000000000000000000000eb584e1aef5a5187ef5951d4ef5752f9ef5555ffef5356ffee5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec465dffec445effeb425fffeb4060ffea3e61ffea3c62ffea3a63ffe93864ffe93665ffe83467ffe83267f9e63068d4e62f6987e1316b1a000000000000000000000000",
];

/// The app icon at 256 px, every eighth pixel from (4, 4), `RRGGBBAA`.
const APP_256: [&str; 32] = [
    "00000000000000000000000000000000fd9831fffd9632fffc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fffa863afff9843bfff9823cfff8803dfff87e3efff77c40fff77b41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b48fc00000000000000000000000000000000",
    "0000000000000000fb97304ffd9831fffd9632fffc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fffa863afff9843bfff9823cfff8803dfff87e3efff77c3ffff77b41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bff000000000000000000000000",
    "00000000fb9a2f51fd9831fffd9632fffc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9843bfff9823cfff8803dfff87e3efff77c40fff77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dff0000000000000000",
    "00000000fd9831fffd9632fffc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9843bfff9823cfff8803dfff87e3efff77c40fff77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4fff00000000",
    "fd9831fffd9632fffc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fffa863afff9843bfff9823cfff8803dfff87e3efff77c40fff77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d51ffee5b507e",
    "fd9632fffc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fffa863afff9843bfff9823cfff8803dfff87e3efff77c3ffff77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d50fff15b52fff05953ff",
    "fc9433fffc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9843bfff9823cfffffffffffffffffff77c40fff77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d51fff15b52fff05953fff05754ff",
    "fc9234fffc9035fffb8e36fffb8c37fffa8a38fffa8839fff9863afff9843bfff9823cfffffffffffffffffffffffffffffffffff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555ff",
    "fc9035fffb8e36fffb8c37fffa8a38fffa8839fffa863afff9843bfff9823cfffffffffffffffffffffffffffffffffffffffffffffefefff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ff",
    "fb8e36fffb8c37fffa8a38fffa8839fffa863afff9843bfff9823cfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffde1d7fff57146fff56f47fff46d48fff46b49fff3694afff3674bfffac8bffff2634dfff2614efff15f4ffff15d50fff15b52fff05953fff05754ffef5555ffef5356ffef5157ff",
    "fb8c37fffa8a38fffa8839fff9863afff9843bfff9823cfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff89c80fff46d48fff46b49fff3694afff3674bfff3654cfffac8c1fffac8c1fff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ff",
    "fa8a38fffa8839fff9863afff9843bfff9823cfff8803dfff87e3efff77c40fff77a41fffffffffffffffffffffffffff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfffac8c1fffac7c1fffac6c2fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ff",
    "fa8839fffa863afff9843bfff9823cfff8803dfff87e3efff77c40fff77a41fff77942fffffffffffffffffffffffffff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efffac7c1fffac6c2fffac5c2fff9c5c3fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5aff",
    "fa863afff9843bfff9823cfff8803dfff87e3efff77c3ffff77a41fff77942fff67743fffffffffffffffffffffffffff56f47fff46d48fff46b49fff3694afffbcac0fffbc9c0fffbc8c1fffbc8c1fffac7c1fffac6c2fffac5c2fff9c5c3fff9c4c3fff9c3c3ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bff",
    "f9843bfff9823cfff8803dfff87e3efff77c40fff77a41fff77942fff67743fff67544fffffffffffffffffffffffffff46d48fff46b49fffbcac0fffbcac0fffbc9c0fffbc8c1fffbc8c1fffac7c1fffac6c2fffac5c2fff9c5c3fff9c4c3fff9c3c3fff9c3c4ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cff",
    "f9823cfff8803dfff87e3efff77c40fff77a41fff77942fff67743fff67544fff57345fffffffffffffffffffffffffff46b49fffbcac0fffbcac0fffbc9c0fffbc8c1fffbc8c1fff26d5ffff15d51fff15b52fff9c5c3fff9c4c3fff9c3c3fff9c3c4ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dff",
    "f8803dfff87e3efff77c40fff77b41fff77942fff67743fff67544fff57345fff57146fffffffffffffffffffffffffffbcac0fffbcac0fffbc9c0fffbc8c1fff2614efff15f4ffff15d51fff15b52fff05953fff9c4c3fff9c3c3fff9c3c4ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435eff",
    "f87e3efff77c3ffff77b41fff77942fff67743fff67544fff57345fff57146fff56f47fffffffffffffffffffffffffffbcac0fffbc9c0fff8aa9efff2614efff15f4ffff15d50fff15b52fff05953fff05754fff9c3c3fff9c3c4ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fff",
    "f77c40fff77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fffffffffffffffffffffffffffbc9c0fffbc8c1fff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555fff9c2c3ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ff",
    "f77a41fff77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fffffffffffffffffffffffffffbc8c1fff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ff",
    "f77942fff67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afffffffffffffffffffffffffffabfb7fff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ff",
    "f67743fff67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfffffffffffffffffffffffffff15f4ffff15d50fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fffeb3f60ffea3d61ffea3b63ffea3a64ff",
    "f67544fff57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfffffffffffffffffffffffffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ff",
    "f57345fff57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfffffffffffffffffffffffffff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ffe93666ff",
    "f57146fff56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efffffffffffffffffffffffffff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3c63ffea3a64ffe93865ffe93666ffe83467ff",
    "f56f47fff46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffffffffffffffffffffffffffff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fffeb3f60ffea3d61ffea3c63ffea3a64ffe93865ffe93666ffe83467ffe83268ff",
    "f46d48fff46b49fff3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ffe93666ffe83467ffe83268ffe83069ff",
    "f36b48fcf3694afff3674bfff3654cfff2634dfff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ffe93666ffe83467ffe83268ffe83069ffe52a661e",
    "00000000f3674bfff3654cfff2634dfff2614efff15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ffe93666ffe83467ffe83268ffe83069ffe72e6aff00000000",
    "0000000000000000f2634dfff2614efff15f4ffff15d50fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffed475cffec455dffec435effeb415fffeb3f60ffea3d61ffea3b63ffea3a64ffe93865ffe93666ffe83467ffe83268ffe83069ffe72e6aff0000000000000000",
    "000000000000000000000000f15f4ffff15d51fff15b52fff05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ffe93666ffe83467ffe83268ffe83069ffe72e6aff000000000000000000000000",
    "00000000000000000000000000000000ee59517df05953fff05754ffef5555ffef5356ffef5157ffee4f58ffee4d59ffed4b5affed495bffec475cffec455dffec435effeb415fffeb3f60ffea3d62ffea3b63ffea3a64ffe93865ffe93666ffe83467ffe83268ffe83069ffe52a661e00000000000000000000000000000000",
];
/// The 256 px icon's sums of alpha and of premultiplied red, green and blue.
const APP_256_SUMS: [u64; 4] = [16024944, 15300285, 7251563, 6093486];
