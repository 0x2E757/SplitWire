//! What a laid-out tree looks like: an element's visual children, where it lies relative to
//! another, its runtime class and `Tag`, a TextBlock's text and baseline, a control's font size.

use std::ptr;

use super::winrt::*;
use crate::platform::abi::Raw;
use crate::platform::com::{Com, ComResult, HString, check, factory};

/// `Windows.Foundation.Rect`: an element's box, in DIP.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn top(&self) -> f64 {
        f64::from(self.y)
    }

    pub fn middle(&self) -> f64 {
        f64::from(self.y) + f64::from(self.height) / 2.0
    }

    pub fn empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Whether this box holds `other`, give or take `tolerance` on each side.
    pub fn covers(&self, other: &Rect, tolerance: f32) -> bool {
        self.x <= other.x + tolerance
            && self.y <= other.y + tolerance
            && self.x + self.width >= other.x + other.width - tolerance
            && self.y + self.height >= other.y + other.height - tolerance
    }
}

/// A TextBlock's text, font size and first baseline from its top, its top padding included.
pub struct TextLine {
    pub text: String,
    pub size: f64,
    pub baseline: f64,
}

/// `VisualTreeHelper`, kept for a walk over a tree.
pub struct Layout {
    helper: Com,
}

impl Layout {
    pub fn new() -> ComResult<Self> {
        Ok(Self {
            helper: factory(
                "Windows.UI.Xaml.Media.VisualTreeHelper",
                &VISUAL_TREE_STATICS,
            )?,
        })
    }

    /// The visual children of `element`, templates' parts included.
    pub fn children(&self, element: &Com) -> ComResult<Vec<Com>> {
        let object = element.query(&DEPENDENCY_OBJECT)?;
        // SAFETY: GetChildrenCount and GetChild take a DependencyObject; GetChild returns one.
        unsafe {
            let count: unsafe extern "system" fn(Raw, Raw, *mut i32) -> i32 =
                self.helper.slot(VISUAL_TREE_CHILDREN_COUNT);
            let mut n = 0;
            check(count(self.helper.raw(), object.raw(), &mut n))?;
            let child: unsafe extern "system" fn(Raw, Raw, i32, *mut Raw) -> i32 =
                self.helper.slot(VISUAL_TREE_GET_CHILD);
            (0..n)
                .map(|i| {
                    let mut found = ptr::null_mut();
                    check(child(self.helper.raw(), object.raw(), i, &mut found))?;
                    Com::owned(found)
                })
                .collect()
        }
    }

    /// Finishes a pending layout pass of a live tree.
    pub fn update(root: &Com) -> ComResult<()> {
        let element = root.query(&UI_ELEMENT)?;
        // SAFETY: UpdateLayout takes nothing.
        unsafe {
            let update: unsafe extern "system" fn(Raw) -> i32 =
                element.slot(UI_ELEMENT_UPDATE_LAYOUT);
            check(update(element.raw()))
        }
    }

    /// `element`'s box in `root`'s coordinates.
    pub fn bounds(element: &Com, root: &Com) -> ComResult<Rect> {
        let framework = element.query(&FRAMEWORK_ELEMENT)?;
        let own = Rect {
            x: 0.0,
            y: 0.0,
            width: framework.value::<f64>(FRAMEWORK_ACTUAL_WIDTH)? as f32,
            height: framework.value::<f64>(FRAMEWORK_ACTUAL_HEIGHT)? as f32,
        };
        let element = element.query(&UI_ELEMENT)?;
        let root = root.query(&UI_ELEMENT)?;
        // SAFETY: TransformToVisual takes an element and returns a transform; TransformBounds
        // takes a Rect by value and writes one.
        unsafe {
            let to_visual: unsafe extern "system" fn(Raw, Raw, *mut Raw) -> i32 =
                element.slot(UI_ELEMENT_TRANSFORM_TO_VISUAL);
            let mut transform = ptr::null_mut();
            check(to_visual(element.raw(), root.raw(), &mut transform))?;
            let transform = Com::owned(transform)?.query(&GENERAL_TRANSFORM)?;
            let bounds: unsafe extern "system" fn(Raw, Rect, *mut Rect) -> i32 =
                transform.slot(GENERAL_TRANSFORM_BOUNDS);
            let mut result = Rect::default();
            check(bounds(transform.raw(), own, &mut result))?;
            Ok(result)
        }
    }

    /// The element's runtime class, such as `Windows.UI.Xaml.Controls.TextBlock`.
    pub fn class(element: &Com) -> String {
        // SAFETY: IInspectable.GetRuntimeClassName returns an owned HSTRING.
        unsafe {
            let name: unsafe extern "system" fn(Raw, *mut Raw) -> i32 =
                element.slot(INSPECTABLE_CLASS_NAME);
            let mut text = ptr::null_mut();
            if name(element.raw(), &mut text) < 0 {
                return String::new();
            }
            HString(text).text()
        }
    }

    /// `FrameworkElement.Tag` when it is a string.
    pub fn tag(element: &Com) -> Option<String> {
        let tag = element
            .query(&FRAMEWORK_ELEMENT)
            .ok()?
            .object(FRAMEWORK_GET_TAG)
            .ok()?;
        tag.query(&PROPERTY_VALUE)
            .ok()?
            .string(PROPERTY_VALUE_GET_STRING)
            .ok()
    }

    /// A TextBlock's line; `None` for any other element.
    pub fn text(element: &Com) -> Option<TextLine> {
        let text = element.query(&TEXT_BLOCK).ok()?;
        let padding = text.value::<[f64; 4]>(TEXT_BLOCK_PADDING).ok()?;
        Some(TextLine {
            text: text.string(TEXT_BLOCK_TEXT).ok()?,
            size: text.value::<f64>(TEXT_BLOCK_FONT_SIZE).ok()?,
            baseline: padding[1] + text.value::<f64>(TEXT_BLOCK_BASELINE).ok()?,
        })
    }

    /// A TextBox's text and font size; `None` for any other element.
    pub fn text_box(element: &Com) -> Option<(String, f64)> {
        let text = element
            .query(&TEXT_BOX)
            .ok()?
            .string(TEXT_BOX_GET_TEXT)
            .ok()?;
        let size = element
            .query(&CONTROL)
            .ok()?
            .value::<f64>(CONTROL_FONT_SIZE)
            .ok()?;
        Some((text, size))
    }
}
