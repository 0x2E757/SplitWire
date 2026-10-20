//! A XAML island in a window that is never shown, for the checks that need a live tree: implicit
//! styles, templates and text reach an element only in a live tree, and only a live tree can be
//! pictured. The window is a hidden tool window far off the screen: no `WS_VISIBLE`, never
//! `ShowWindow`, no taskbar button, never activated.

use std::ptr;
use std::time::{Duration, Instant};

use super::island::Island;
use super::layout::Layout;
use super::native_window::{NativeWindow, WindowClass};
use crate::platform::abi::{Raw, wide};
use crate::platform::com::{Com, ComResult, E_FAIL};

pub struct HiddenHost {
    island: Island,
    hwnd: Raw,
}

impl HiddenHost {
    const CLASS: &'static str = "SplitWire.Window.Check";
    /// `WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE`.
    const EX_STYLE: u32 = 0x0800_0080;
    /// `WS_POPUP`, without `WS_VISIBLE`.
    const STYLE: u32 = 0x8000_0000;
    /// `SWP_NOZORDER | SWP_NOACTIVATE`, without `SWP_SHOWWINDOW`.
    const MOVE: u32 = 0x14;

    pub fn new() -> ComResult<Self> {
        let name = wide(Self::CLASS);
        // SAFETY: plain Win32 calls with valid, NUL-terminated strings.
        let hwnd = unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class = WindowClass {
                size: std::mem::size_of::<WindowClass>() as u32,
                style: 0,
                procedure: DefWindowProcW,
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: ptr::null_mut(),
                cursor: ptr::null_mut(),
                background: ptr::null_mut(),
                menu: ptr::null(),
                name: name.as_ptr(),
                small_icon: ptr::null_mut(),
            };
            RegisterClassExW(&class);
            CreateWindowExW(
                Self::EX_STYLE,
                name.as_ptr(),
                name.as_ptr(),
                Self::STYLE,
                -32000,
                -32000,
                640,
                480,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err(E_FAIL);
        }
        let island = match Island::attach(hwnd) {
            Ok(island) => island,
            Err(hr) => {
                // SAFETY: our own window.
                unsafe { DestroyWindow(hwnd) };
                return Err(hr);
            }
        };
        Ok(Self { island, hwnd })
    }

    /// Puts `root` into the island at `width` × `height` DIP and lets XAML lay it out.
    pub fn show(&self, root: &Com, width: f64, height: f64) -> ComResult<()> {
        // SAFETY: plain queries and a move of our own hidden windows.
        unsafe {
            if IsWindowVisible(self.hwnd) != 0 {
                return Err(E_FAIL);
            }
            let scale = f64::from(GetDpiForWindow(self.hwnd).max(96)) / 96.0;
            let (w, h) = ((width * scale) as i32, (height * scale) as i32);
            SetWindowPos(self.hwnd, ptr::null_mut(), -32000, -32000, w, h, Self::MOVE);
            SetWindowPos(self.island.child, ptr::null_mut(), 0, 0, w, h, Self::MOVE);
        }
        self.island.set_content(root)?;
        self.settle(root, width, height)
    }

    /// Lets XAML run until `root` is laid out at its full size.
    fn settle(&self, root: &Com, width: f64, height: f64) -> ComResult<()> {
        let start = Instant::now();
        loop {
            NativeWindow::drain();
            let bounds = Layout::bounds(root, root)?;
            let done = (f64::from(bounds.width) - width).abs() < 1.0
                && (f64::from(bounds.height) - height).abs() < 1.0;
            if done {
                Layout::update(root)?;
                NativeWindow::drain();
                return Ok(());
            }
            if start.elapsed() > Duration::from_secs(10) {
                return Err(E_FAIL);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

impl Drop for HiddenHost {
    fn drop(&mut self) {
        // SAFETY: our own window; the island goes with it.
        unsafe {
            DestroyWindow(self.hwnd);
        }
        NativeWindow::drain();
    }
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Raw;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassExW(class: *const WindowClass) -> u16;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Raw,
        menu: Raw,
        instance: Raw,
        param: Raw,
    ) -> Raw;
    fn DefWindowProcW(hwnd: Raw, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(hwnd: Raw) -> i32;
    fn IsWindowVisible(hwnd: Raw) -> i32;
    fn SetWindowPos(
        hwnd: Raw,
        after: Raw,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn GetDpiForWindow(hwnd: Raw) -> u32;
}
