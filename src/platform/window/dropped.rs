//! Files dropped on the drop zone: the drag's storage items, read asynchronously by WinRT and
//! polled every frame until they arrive.

use std::path::PathBuf;

use super::winrt::*;
use crate::platform::abi::Raw;
use crate::platform::com::{Com, ComResult};

/// A drag's `GetStorageItemsAsync` in progress.
pub struct PendingItems {
    operation: Com,
    /// Only the names are needed while the files are dragged over the zone.
    pub names_only: bool,
}

impl PendingItems {
    /// Accepts a copy and asks for the dragged items.
    pub fn start(args: &Com, names_only: bool) -> ComResult<Self> {
        let args = args.query(&DRAG_ARGS2)?;
        // DataPackageOperation.Copy.
        args.set_value(DRAG_ARGS_PUT_ACCEPTED, 1u32)?;
        let view = args
            .object(DRAG_ARGS_DATA_VIEW)?
            .query(&DATA_PACKAGE_VIEW)?;
        Ok(Self {
            operation: view.object(DATA_VIEW_STORAGE_ITEMS)?,
            names_only,
        })
    }

    /// Only accepts the copy, as DragOver must every time.
    pub fn accept(args: &Com) -> ComResult<()> {
        args.query(&DRAG_ARGS2)?
            .set_value(DRAG_ARGS_PUT_ACCEPTED, 1u32)
    }

    /// The items' paths once the operation completed; `Ok(None)` while it runs.
    pub fn poll(&self) -> ComResult<Option<Vec<PathBuf>>> {
        // AsyncStatus: Started 0, Completed 1, Canceled 2, Error 3.
        match self
            .operation
            .query(&ASYNC_INFO)?
            .value::<i32>(ASYNC_INFO_STATUS)?
        {
            0 => return Ok(None),
            1 => {}
            _ => return Ok(Some(Vec::new())),
        }
        let items = self.operation.object(ASYNC_ITEMS_GET_RESULTS)?;
        let count = items.value::<u32>(VECTOR_ITEMS_SIZE)?;
        let mut paths = Vec::new();
        for index in 0..count {
            // SAFETY: GetAt(index, *item) of IVectorView<IStorageItem>.
            let item = unsafe {
                let at: unsafe extern "system" fn(Raw, u32, *mut Raw) -> i32 =
                    items.slot(VECTOR_ITEMS_GET_AT);
                let mut item = std::ptr::null_mut();
                crate::platform::com::check(at(items.raw(), index, &mut item))?;
                Com::owned(item)?
            };
            let item = item.query(&STORAGE_ITEM)?;
            let path = item.string(STORAGE_ITEM_PATH)?;
            paths.push(if path.is_empty() {
                PathBuf::from(item.string(STORAGE_ITEM_NAME)?)
            } else {
                PathBuf::from(path)
            });
        }
        Ok(Some(paths))
    }
}
