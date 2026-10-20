//! A picture of an element (`RenderTargetBitmap`): its pixels as premultiplied BGRA.

use std::ptr;
use std::time::{Duration, Instant};

use super::native_window::NativeWindow;
use super::winrt::*;
use crate::platform::abi::Raw;
use crate::platform::com::{Com, ComResult, E_FAIL, activate, check, factory};

pub struct Snapshot {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}

impl Snapshot {
    /// Renders `element` as it is laid out now.
    pub fn of(element: &Com) -> ComResult<Self> {
        let bitmap = activate(
            "Windows.UI.Xaml.Media.Imaging.RenderTargetBitmap",
            &RENDER_TARGET_BITMAP,
        )?;
        let element = element.query(&UI_ELEMENT)?;
        // SAFETY: RenderAsync takes an element and returns an IAsyncAction; GetPixelsAsync returns
        // an IAsyncOperation<IBuffer>, whose GetResults gives the buffer.
        let buffer = unsafe {
            let render: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32 =
                bitmap.slot(RENDER_ASYNC);
            let mut action = ptr::null_mut();
            check(render(bitmap.raw(), element.raw(), &mut action))?;
            Self::wait(&Com::owned(action)?)?;
            let pixels: unsafe extern "system" fn(Raw, *mut Raw) -> i32 =
                bitmap.slot(RENDER_GET_PIXELS);
            let mut operation = ptr::null_mut();
            check(pixels(bitmap.raw(), &mut operation))?;
            let operation = Com::owned(operation)?;
            Self::wait(&operation)?;
            operation.object(ASYNC_BUFFER_GET_RESULTS)?
        };
        let width = bitmap.value::<i32>(RENDER_PIXEL_WIDTH)? as u32;
        let height = bitmap.value::<i32>(RENDER_PIXEL_HEIGHT)? as u32;
        Ok(Self {
            width,
            height,
            bgra: Self::bytes(&buffer)?,
        })
    }

    /// Lets XAML run until the asynchronous call finishes.
    fn wait(operation: &Com) -> ComResult<()> {
        let info = operation.query(&ASYNC_INFO)?;
        let start = Instant::now();
        loop {
            match info.value::<i32>(ASYNC_INFO_STATUS)? {
                0 if start.elapsed() < Duration::from_secs(10) => {
                    NativeWindow::drain();
                    std::thread::sleep(Duration::from_millis(1));
                }
                1 => return Ok(()),
                0 => return Err(E_FAIL),
                _ => {
                    let code = info.value::<i32>(ASYNC_INFO_ERROR_CODE)?;
                    return Err(if code < 0 { code } else { E_FAIL });
                }
            }
        }
    }

    fn bytes(buffer: &Com) -> ComResult<Vec<u8>> {
        let statics = factory("Windows.Storage.Streams.DataReader", &DATA_READER_STATICS)?;
        // SAFETY: FromBuffer takes an IBuffer and returns a reader; ReadBytes fills `length`
        // bytes.
        unsafe {
            let from: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32 =
                statics.slot(DATA_READER_FROM_BUFFER);
            let mut reader = ptr::null_mut();
            check(from(statics.raw(), buffer.raw(), &mut reader))?;
            let reader = Com::owned(reader)?.query(&DATA_READER)?;
            let length = reader.value::<u32>(DATA_READER_UNCONSUMED)?;
            let mut bytes = vec![0u8; length as usize];
            let read: unsafe extern "system" fn(Raw, u32, *mut u8) -> i32 =
                reader.slot(DATA_READER_READ_BYTES);
            check(read(reader.raw(), length, bytes.as_mut_ptr()))?;
            Ok(bytes)
        }
    }
}
