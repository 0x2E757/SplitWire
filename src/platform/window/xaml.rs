//! Element operations the window needs, each one interface call: load markup, find an element,
//! set a host's content, read and write a field, focus, show or hide.

use std::ptr;

use super::winrt::*;
use crate::platform::abi::Raw;
use crate::platform::com::{Com, ComResult, HString, check, factory};

pub struct Xaml;

impl Xaml {
    /// Parses markup into elements (`XamlReader.Load`); needs XAML on this thread, no window.
    pub fn load(markup: &str) -> ComResult<Com> {
        let reader = factory("Windows.UI.Xaml.Markup.XamlReader", &XAML_READER_STATICS)?;
        let text = HString::new(markup)?;
        // SAFETY: Load takes an HSTRING and returns the root object.
        unsafe {
            let load: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32 =
                reader.slot(XAML_READER_LOAD);
            let mut root = ptr::null_mut();
            check(load(reader.raw(), text.0, &mut root))?;
            Com::owned(root)
        }
    }

    /// The element named `name` in `root`'s namescope.
    pub fn find(root: &Com, name: &str) -> ComResult<Com> {
        let element = root.query(&FRAMEWORK_ELEMENT)?;
        let name = HString::new(name)?;
        // SAFETY: FindName takes an HSTRING and returns an object or null.
        unsafe {
            let find: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32 =
                element.slot(FRAMEWORK_FIND_NAME);
            let mut found = ptr::null_mut();
            check(find(element.raw(), name.0, &mut found))?;
            Com::owned(found)
        }
    }

    /// `ContentControl.Content`.
    pub fn set_content(host: &Com, child: &Com) -> ComResult<()> {
        host.query(&CONTENT_CONTROL)?
            .set_value(CONTENT_CONTROL_PUT_CONTENT, child.raw())
    }

    pub fn set_opacity(element: &Com, opacity: f64) -> ComResult<()> {
        element
            .query(&UI_ELEMENT)?
            .set_value(UI_ELEMENT_PUT_OPACITY, opacity)
    }

    pub fn text(text_box: &Com) -> ComResult<String> {
        text_box.query(&TEXT_BOX)?.string(TEXT_BOX_GET_TEXT)
    }

    pub fn set_text(text_box: &Com, text: &str) -> ComResult<()> {
        text_box
            .query(&TEXT_BOX)?
            .set_string(TEXT_BOX_PUT_TEXT, text)
    }

    /// The selection: its start (the caret when nothing is selected) and length.
    pub fn selection(text_box: &Com) -> ComResult<(i32, i32)> {
        let text_box = text_box.query(&TEXT_BOX)?;
        Ok((
            text_box.value::<i32>(TEXT_BOX_GET_SELECTION_START)?,
            text_box.value::<i32>(TEXT_BOX_GET_SELECTION_LENGTH)?,
        ))
    }

    pub fn select(text_box: &Com, (start, length): (i32, i32)) -> ComResult<()> {
        let text_box = text_box.query(&TEXT_BOX)?;
        text_box.set_value(TEXT_BOX_PUT_SELECTION_START, start)?;
        text_box.set_value(TEXT_BOX_PUT_SELECTION_LENGTH, length)
    }

    /// `Control.Focus(FocusState.Programmatic)`: whether the focus moved there. A control just put
    /// into its host is not in the live tree before the next layout, and takes no focus yet.
    pub fn focus(control: &Com) -> ComResult<bool> {
        let control = control.query(&CONTROL)?;
        // SAFETY: Focus takes the focus state and returns whether it moved.
        unsafe {
            let focus: unsafe extern "system" fn(Raw, i32, *mut u8) -> i32 =
                control.slot(CONTROL_FOCUS);
            let mut moved = 0;
            check(focus(control.raw(), 3, &mut moved))?;
            Ok(moved != 0)
        }
    }

    pub fn scroll_to_top(viewer: &Com) -> ComResult<()> {
        viewer
            .query(&SCROLL_VIEWER)?
            .set_value(SCROLL_VIEWER_SCROLL_TO_VERTICAL, 0.0f64)
    }
}
