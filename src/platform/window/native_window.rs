//! The top-level window (TaskbarMetrics' rules): the system title bar tinted like the app, opened
//! centred at 1280 × 1024 or where it was closed, never smaller than 780 × 960 (or the work area),
//! a frame timer that keeps running inside Windows' modal size and move loops. With the tray,
//! closing hides it.

use std::cell::{Cell, RefCell};
use std::ptr;

use super::island::Island;
use crate::platform::abi::{Raw, wide};
use crate::platform::com::{ComResult, Hr};
use crate::presentation::Placement;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// `MSG`.
#[repr(C)]
pub struct Message {
    hwnd: Raw,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    x: i32,
    y: i32,
    private: u32,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            hwnd: ptr::null_mut(),
            message: 0,
            wparam: 0,
            lparam: 0,
            time: 0,
            x: 0,
            y: 0,
            private: 0,
        }
    }
}

/// `WNDCLASSEXW`, also the tray's hidden window's.
#[repr(C)]
pub(super) struct WindowClass {
    pub(super) size: u32,
    pub(super) style: u32,
    pub(super) procedure: unsafe extern "system" fn(Raw, u32, usize, isize) -> isize,
    pub(super) class_extra: i32,
    pub(super) window_extra: i32,
    pub(super) instance: Raw,
    pub(super) icon: Raw,
    pub(super) cursor: Raw,
    pub(super) background: Raw,
    pub(super) menu: *const u16,
    pub(super) name: *const u16,
    pub(super) small_icon: Raw,
}

#[repr(C)]
struct MinMaxInfo {
    reserved: [i32; 2],
    max_size: [i32; 2],
    max_position: [i32; 2],
    min_track: [i32; 2],
    max_track: [i32; 2],
}

#[repr(C)]
#[derive(Default)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[repr(C)]
#[derive(Default)]
struct WindowPlacement {
    length: u32,
    flags: u32,
    show: u32,
    minimized: [i32; 2],
    maximized: [i32; 2],
    normal: Rect,
}

type Refresh = dyn FnMut() -> ComResult<()>;

thread_local! {
    static TICK: Cell<Option<*mut Refresh>> = const { Cell::new(None) };
    static TICK_ERROR: Cell<Option<Hr>> = const { Cell::new(None) };
    static CHILD: Cell<Raw> = const { Cell::new(ptr::null_mut()) };
    static CLOSED: Cell<Option<Placement>> = const { Cell::new(None) };
    static KEEP_ON_CLOSE: Cell<bool> = const { Cell::new(false) };
    static DROPPED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// The frame callback, reachable from the window procedure: inside a modal size or move loop
/// Windows dispatches the timer there instead of returning to `run`.
struct Tick<'a>(std::marker::PhantomData<&'a mut ()>);

impl<'a> Tick<'a> {
    fn install(refresh: &'a mut (dyn FnMut() -> ComResult<()> + 'a)) -> Self {
        // SAFETY: only the lifetime is erased; `Drop` removes the pointer before it ends.
        let raw: *mut Refresh = unsafe {
            std::mem::transmute::<*mut (dyn FnMut() -> ComResult<()> + 'a), *mut Refresh>(refresh)
        };
        TICK.with(|tick| tick.set(Some(raw)));
        Self(std::marker::PhantomData)
    }

    fn fire() -> ComResult<()> {
        let Some(refresh) = TICK.with(Cell::take) else {
            return Ok(());
        };
        // SAFETY: installed by a live `Tick`; taken out while it runs, so it never re-enters.
        let result = unsafe { (*refresh)() };
        TICK.with(|tick| tick.set(Some(refresh)));
        result
    }
}

impl Drop for Tick<'_> {
    fn drop(&mut self) {
        TICK.with(|tick| tick.set(None));
    }
}

pub struct NativeWindow(Raw);

impl NativeWindow {
    const CLASS: &'static str = "SplitWire.Window";
    pub const WIDTH: i32 = 1280;
    pub const HEIGHT: i32 = 1024;
    const MIN_WIDTH: i32 = 780;
    const MIN_HEIGHT: i32 = 960;
    const FRAME_MS: u32 = 16;

    /// A hidden window titled `title`, placed by `placement` or centred.
    pub fn new(title: &str, placement: Option<Placement>) -> ComResult<Self> {
        let name = wide(Self::CLASS);
        // SAFETY: plain Win32 calls with valid, NUL-terminated strings.
        unsafe {
            let instance = GetModuleHandleW(ptr::null());
            let class = WindowClass {
                size: std::mem::size_of::<WindowClass>() as u32,
                style: 0,
                procedure: Self::procedure,
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: ptr::null_mut(),
                cursor: LoadCursorW(ptr::null_mut(), 32512usize as *const u16),
                background: ptr::null_mut(),
                menu: ptr::null(),
                name: name.as_ptr(),
                small_icon: ptr::null_mut(),
            };
            RegisterClassExW(&class);
            let hwnd = CreateWindowExW(
                0,
                name.as_ptr(),
                wide(title).as_ptr(),
                0x02cf_0000,
                i32::MIN,
                i32::MIN,
                Self::WIDTH,
                Self::HEIGHT,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            );
            if hwnd.is_null() {
                return Err(crate::platform::com::E_FAIL);
            }
            DragAcceptFiles(hwnd, 1);
            let window = Self(hwnd);
            match placement.filter(|p| window.restore(p)) {
                Some(_) => {}
                None => window.center(),
            }
            Ok(window)
        }
    }

    pub fn raw(&self) -> Raw {
        self.0
    }

    fn scale(&self) -> f64 {
        // SAFETY: a plain query of the window's DPI.
        f64::from(unsafe { GetDpiForWindow(self.0) }.max(96)) / 96.0
    }

    /// Centred on the primary work area at 1280 × 1024 of visible frame, no larger than it.
    fn center(&self) {
        let scale = self.scale();
        let mut area = Rect::default();
        // SAFETY: SPI_GETWORKAREA fills a RECT.
        unsafe {
            if SystemParametersInfoW(0x30, 0, &mut area as *mut Rect as Raw, 0) == 0 {
                return;
            }
            let width = ((f64::from(Self::WIDTH) * scale) as i32).min(area.right - area.left);
            let height = ((f64::from(Self::HEIGHT) * scale) as i32).min(area.bottom - area.top);
            SetWindowPos(
                self.0,
                ptr::null_mut(),
                area.left + (area.right - area.left - width) / 2,
                area.top + (area.bottom - area.top - height) / 2,
                width,
                height,
                0x14,
            );
        }
    }

    /// Moves the hidden window to `placement` (DIP); false when that frame is on no monitor.
    fn restore(&self, placement: &Placement) -> bool {
        let scale = self.scale();
        let px = |v: i32| (f64::from(v) * scale) as i32;
        let normal = Rect {
            left: px(placement.x),
            top: px(placement.y),
            right: px(placement.x + placement.width),
            bottom: px(placement.y + placement.height),
        };
        // SAFETY: MONITOR_DEFAULTTONULL; SetWindowPlacement with SW_HIDE keeps it hidden.
        unsafe {
            if MonitorFromRect(&normal, 0).is_null() {
                return false;
            }
            let placement = WindowPlacement {
                length: std::mem::size_of::<WindowPlacement>() as u32,
                normal,
                ..Default::default()
            };
            SetWindowPlacement(self.0, &placement) != 0
        }
    }

    pub fn show(&self, maximized: bool) {
        // SAFETY: SW_SHOWMAXIMIZED or SW_SHOW.
        unsafe {
            ShowWindow(self.0, if maximized { 3 } else { 5 });
        }
    }

    /// The system title bar in the app's colours, dark in the dark theme.
    pub fn tint(&self, dark: bool, caption: &str, text: &str) {
        let colorref = |hex: &str| {
            let value = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
            ((value & 0xFF) << 16) | (value & 0xFF00) | ((value >> 16) & 0xFF)
        };
        let dark = i32::from(dark);
        // SAFETY: DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_CAPTION_COLOR and DWMWA_TEXT_COLOR take 4 bytes.
        unsafe {
            DwmSetWindowAttribute(self.0, 20, &dark as *const i32 as Raw, 4);
            for (attribute, color) in [(35, colorref(caption)), (36, colorref(text))] {
                DwmSetWindowAttribute(self.0, attribute, &color as *const u32 as Raw, 4);
            }
        }
    }

    /// The client area's width in DIP: the width class follows it.
    pub fn width_dip(&self) -> f64 {
        let mut rect = Rect::default();
        // SAFETY: fills a RECT of a live window.
        unsafe {
            GetClientRect(self.0, &mut rect);
        }
        f64::from(rect.right - rect.left) / self.scale()
    }

    /// On screen: shown and not minimized.
    pub fn visible(&self) -> bool {
        // SAFETY: plain queries.
        unsafe { IsWindowVisible(self.0) != 0 && IsIconic(self.0) == 0 }
    }

    /// Shows a hidden or minimized window again and brings it to the front.
    pub fn raise(&self) {
        // SAFETY: SW_RESTORE for a minimized window, SW_SHOW for a hidden one.
        unsafe {
            ShowWindow(self.0, if IsIconic(self.0) != 0 { 9 } else { 5 });
            SetForegroundWindow(self.0);
        }
    }

    /// The window's frame now, in DIP.
    pub fn placement(&self) -> Option<Placement> {
        // SAFETY: a window of ours.
        unsafe { Self::placement_of(self.0) }
    }

    /// With the tray, closing the window hides it and the process goes on.
    pub fn keep_on_close(keep: bool) {
        KEEP_ON_CLOSE.with(|k| k.set(keep));
    }

    /// Ends the message loop: the tray's Exit.
    pub fn quit() {
        // SAFETY: posts WM_QUIT to this thread.
        unsafe { PostQuitMessage(0) }
    }

    pub fn attach(&self, island: &Island) {
        CHILD.with(|child| child.set(island.child));
        // SAFETY: resizes the island's child to the client area.
        unsafe { Self::resize(self.0) }
    }

    /// The window's frame when it was last closed, in DIP; taken once.
    pub fn take_closed() -> Option<Placement> {
        CLOSED.with(Cell::take)
    }

    /// Files dropped on the window outside the drop zone's XAML handling.
    pub fn take_dropped() -> Vec<String> {
        DROPPED.with(|d| std::mem::take(&mut *d.borrow_mut()))
    }

    /// Runs the message loop, calling `refresh` every frame, until the window closes.
    pub fn run(
        &self,
        island: &Island,
        mut refresh: impl FnMut() -> ComResult<()>,
    ) -> ComResult<()> {
        let _tick = Tick::install(&mut refresh);
        // SAFETY: a standard message loop; the island sees each message first.
        unsafe {
            SetTimer(self.0, 1, Self::FRAME_MS, ptr::null_mut());
            let mut message = Message::default();
            loop {
                if let Some(error) = TICK_ERROR.with(Cell::take) {
                    return Err(error);
                }
                let status = GetMessageW(&mut message, ptr::null_mut(), 0, 0);
                if status <= 0 {
                    return Ok(());
                }
                if message.hwnd == self.0 && message.message == 0x0113 && message.wparam == 1 {
                    Tick::fire()?;
                    continue;
                }
                if island.pre_translate(&message as *const Message as *const std::ffi::c_void) {
                    continue;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    /// Dispatches what is queued, without waiting.
    pub fn drain() {
        // SAFETY: a non-blocking message pump.
        unsafe {
            let mut message = Message::default();
            while PeekMessageW(&mut message, ptr::null_mut(), 0, 0, 1) != 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }

    unsafe fn resize(hwnd: Raw) {
        let child = CHILD.with(Cell::get);
        if child.is_null() {
            return;
        }
        let mut rect = Rect::default();
        // SAFETY: plain Win32 calls on live windows.
        unsafe {
            GetClientRect(hwnd, &mut rect);
            SetWindowPos(child, ptr::null_mut(), 0, 0, rect.right, rect.bottom, 0x54);
        }
    }

    unsafe fn placement_of(hwnd: Raw) -> Option<Placement> {
        let mut placement = WindowPlacement {
            length: std::mem::size_of::<WindowPlacement>() as u32,
            ..Default::default()
        };
        // SAFETY: fills the struct the length names.
        let scale = unsafe {
            if GetWindowPlacement(hwnd, &mut placement) == 0 {
                return None;
            }
            f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0
        };
        let dip = |v: i32| (f64::from(v) / scale).round() as i32;
        let normal = placement.normal;
        Some(Placement {
            x: dip(normal.left),
            y: dip(normal.top),
            width: dip(normal.right - normal.left),
            height: dip(normal.bottom - normal.top),
            maximized: placement.show == 3 || (placement.show == 2 && placement.flags & 2 != 0),
        })
    }

    unsafe fn dropped(drop: Raw) {
        // SAFETY: HDROP from WM_DROPFILES, released with DragFinish.
        unsafe {
            let count = DragQueryFileW(drop, u32::MAX, ptr::null_mut(), 0);
            for index in 0..count {
                let mut path = [0u16; 1024];
                let length =
                    DragQueryFileW(drop, index, path.as_mut_ptr(), path.len() as u32) as usize;
                let path = String::from_utf16_lossy(&path[..length]);
                DROPPED.with(|d| d.borrow_mut().push(path));
            }
            DragFinish(drop);
        }
    }

    unsafe extern "system" fn procedure(
        hwnd: Raw,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        // SAFETY: the window procedure of our class; each message's parameters are as documented.
        unsafe {
            match message {
                // WM_SIZE; a minimized window keeps the island's size.
                0x0005 => {
                    if wparam != 1 {
                        Self::resize(hwnd);
                    }
                    0
                }
                // WM_TIMER from a modal size or move loop.
                0x0113 if wparam == 1 => {
                    if let Err(error) = Tick::fire() {
                        TICK_ERROR.with(|e| e.set(Some(error)));
                    }
                    0
                }
                // WM_CLOSE hides the window; without the tray it also ends the loop.
                0x0010 => {
                    CLOSED.with(|closed| closed.set(Self::placement_of(hwnd)));
                    ShowWindow(hwnd, 0);
                    if !KEEP_ON_CLOSE.with(Cell::get) {
                        PostQuitMessage(0);
                    }
                    0
                }
                // WM_GETMINMAXINFO: 780 × 960 of frame, or the work area when smaller.
                0x0024 => {
                    let scale = f64::from(GetDpiForWindow(hwnd).max(96)) / 96.0;
                    let mut width = (f64::from(Self::MIN_WIDTH) * scale) as i32;
                    let mut height = (f64::from(Self::MIN_HEIGHT) * scale) as i32;
                    let mut monitor = MonitorInfo {
                        size: std::mem::size_of::<MonitorInfo>() as u32,
                        ..Default::default()
                    };
                    if GetMonitorInfoW(MonitorFromWindow(hwnd, 2), &mut monitor) != 0 {
                        width = width.min(monitor.work.right - monitor.work.left);
                        height = height.min(monitor.work.bottom - monitor.work.top);
                    }
                    (*(lparam as *mut MinMaxInfo)).min_track = [width, height];
                    0
                }
                // WM_DPICHANGED: the suggested frame.
                0x02e0 => {
                    let rect = &*(lparam as *const Rect);
                    SetWindowPos(
                        hwnd,
                        ptr::null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        0x14,
                    );
                    0
                }
                // WM_DROPFILES.
                0x0233 => {
                    Self::dropped(wparam as Raw);
                    0
                }
                _ => DefWindowProcW(hwnd, message, wparam, lparam),
            }
        }
    }
}

impl Drop for NativeWindow {
    fn drop(&mut self) {
        CHILD.with(|child| child.set(ptr::null_mut()));
        // SAFETY: the window is ours.
        unsafe {
            DestroyWindow(self.0);
        }
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
    fn ShowWindow(hwnd: Raw, command: i32) -> i32;
    fn IsIconic(hwnd: Raw) -> i32;
    fn IsWindowVisible(hwnd: Raw) -> i32;
    fn SetForegroundWindow(hwnd: Raw) -> i32;
    fn GetWindowPlacement(hwnd: Raw, placement: *mut WindowPlacement) -> i32;
    fn SetWindowPlacement(hwnd: Raw, placement: *const WindowPlacement) -> i32;
    fn MonitorFromRect(rect: *const Rect, flags: u32) -> Raw;
    fn MonitorFromWindow(hwnd: Raw, flags: u32) -> Raw;
    fn GetMonitorInfoW(monitor: Raw, info: *mut MonitorInfo) -> i32;
    fn GetMessageW(message: *mut Message, hwnd: Raw, min: u32, max: u32) -> i32;
    fn PeekMessageW(message: *mut Message, hwnd: Raw, min: u32, max: u32, remove: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn PostQuitMessage(code: i32);
    fn GetClientRect(hwnd: Raw, rect: *mut Rect) -> i32;
    fn SystemParametersInfoW(action: u32, value: u32, data: Raw, flags: u32) -> i32;
    fn SetWindowPos(
        hwnd: Raw,
        after: Raw,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn LoadCursorW(instance: Raw, name: *const u16) -> Raw;
    fn GetDpiForWindow(hwnd: Raw) -> u32;
    fn SetTimer(hwnd: Raw, id: usize, milliseconds: u32, callback: Raw) -> usize;
}

#[link(name = "shell32")]
unsafe extern "system" {
    fn DragAcceptFiles(hwnd: Raw, accept: i32);
    fn DragQueryFileW(drop: Raw, index: u32, file: *mut u16, size: u32) -> u32;
    fn DragFinish(drop: Raw);
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(hwnd: Raw, attribute: u32, value: Raw, size: u32) -> Hr;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn layouts_match_the_sdk() {
        assert_eq!(size_of::<Message>(), 48); // MSG
        assert_eq!(size_of::<WindowClass>(), 80); // WNDCLASSEXW
        assert_eq!(size_of::<WindowPlacement>(), 44); // WINDOWPLACEMENT
        assert_eq!(offset_of!(WindowPlacement, normal), 28); // WINDOWPLACEMENT.rcNormalPosition
        assert_eq!(size_of::<MinMaxInfo>(), 40); // MINMAXINFO
        assert_eq!(offset_of!(MinMaxInfo, min_track), 24); // MINMAXINFO.ptMinTrackSize
        assert_eq!(size_of::<MonitorInfo>(), 40); // MONITORINFO
        assert_eq!(offset_of!(MonitorInfo, work), 20); // MONITORINFO.rcWork
    }
}
