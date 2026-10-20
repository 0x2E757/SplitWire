//! Choose files…: `IFileOpenDialog` with `FOS_ALLOWMULTISELECT` and a `*.conf` filter.

use std::path::PathBuf;
use std::ptr;

use super::winrt::*;
use crate::platform::abi::{Raw, wide};
use crate::platform::com::{CoTaskMemFree, Com, ComResult, check, create};

/// `COMDLG_FILTERSPEC`.
#[repr(C)]
struct FilterSpec {
    name: *const u16,
    pattern: *const u16,
}

pub struct ConfPicker;

impl ConfPicker {
    const ALLOW_MULTISELECT: u32 = 0x200;
    const FORCE_FILESYSTEM: u32 = 0x40;
    const FILE_MUST_EXIST: u32 = 0x1000;
    /// `HRESULT_FROM_WIN32(ERROR_CANCELLED)`.
    const CANCELLED: i32 = 0x8007_04C7_u32 as i32;
    /// `SIGDN_FILESYSPATH`.
    const FILE_SYSTEM_PATH: u32 = 0x8005_8000;

    /// The files chosen, or none when the dialog was cancelled. Modal over `owner`.
    pub fn pick(owner: Raw) -> ComResult<Vec<PathBuf>> {
        let dialog = create(&FILE_OPEN_DIALOG_CLASS, &FILE_OPEN_DIALOG)?;
        let name = wide("WireGuard configuration (*.conf)");
        let pattern = wide("*.conf");
        let all_name = wide("All files (*.*)");
        let all = wide("*.*");
        let filters = [
            FilterSpec {
                name: name.as_ptr(),
                pattern: pattern.as_ptr(),
            },
            FilterSpec {
                name: all_name.as_ptr(),
                pattern: all.as_ptr(),
            },
        ];
        let title = wide("Choose WireGuard .conf files");
        // SAFETY: the IFileOpenDialog calls of the SDK, with buffers that outlive them.
        unsafe {
            let options = dialog.value::<u32>(FILE_DIALOG_GET_OPTIONS)?;
            dialog.set_value(
                FILE_DIALOG_SET_OPTIONS,
                options | Self::ALLOW_MULTISELECT | Self::FORCE_FILESYSTEM | Self::FILE_MUST_EXIST,
            )?;
            let set_types: unsafe extern "system" fn(Raw, u32, *const FilterSpec) -> i32 =
                dialog.slot(FILE_DIALOG_SET_FILE_TYPES);
            check(set_types(
                dialog.raw(),
                filters.len() as u32,
                filters.as_ptr(),
            ))?;
            let set_title: unsafe extern "system" fn(Raw, *const u16) -> i32 =
                dialog.slot(FILE_DIALOG_SET_TITLE);
            check(set_title(dialog.raw(), title.as_ptr()))?;
            let show: unsafe extern "system" fn(Raw, Raw) -> i32 = dialog.slot(MODAL_SHOW);
            match show(dialog.raw(), owner) {
                Self::CANCELLED => return Ok(Vec::new()),
                hr => check(hr)?,
            }
            let items = dialog
                .object(FILE_OPEN_DIALOG_RESULTS)?
                .query(&SHELL_ITEM_ARRAY)?;
            let count = items.value::<u32>(SHELL_ITEM_ARRAY_COUNT)?;
            let mut paths = Vec::new();
            for index in 0..count {
                let at: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32 =
                    items.slot(SHELL_ITEM_ARRAY_AT);
                let mut item = ptr::null_mut();
                check(at(items.raw(), index, &mut item))?;
                let item = Com::owned(item)?;
                paths.push(Self::path(&item)?);
            }
            Ok(paths)
        }
    }

    fn path(item: &Com) -> ComResult<PathBuf> {
        let item = item.query(&SHELL_ITEM)?;
        // SAFETY: GetDisplayName returns a CoTaskMem string the caller frees.
        unsafe {
            let get: unsafe extern "system" fn(Raw, u32, *mut *mut u16) -> i32 =
                item.slot(SHELL_ITEM_DISPLAY_NAME);
            let mut name = ptr::null_mut();
            check(get(item.raw(), Self::FILE_SYSTEM_PATH, &mut name))?;
            let mut length = 0;
            while *name.add(length) != 0 {
                length += 1;
            }
            let path = String::from_utf16_lossy(std::slice::from_raw_parts(name, length));
            CoTaskMemFree(name as Raw);
            Ok(PathBuf::from(path))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn the_filter_spec_is_two_pointers() {
        assert_eq!(size_of::<FilterSpec>(), 16); // COMDLG_FILTERSPEC
    }
}
