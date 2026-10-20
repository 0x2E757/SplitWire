//! Dragging a profile's navigation item reorders the profiles: pressed, moved more
//! than a few pixels, released a number of items away.

use std::cell::RefCell;
use std::rc::Rc;

use super::events::Subscription;
use super::renderer::SharedInbox;
use super::winrt::*;
use super::xaml::Xaml;
use crate::platform::com::{Com, ComResult};
use crate::presentation::Command;

struct Dragging {
    name: String,
    index: usize,
    start: f32,
    moved: bool,
    element: Com,
}

#[derive(Default)]
pub struct NavDrag {
    active: Option<Dragging>,
    /// The distance between two items: their height and the spacing.
    pub pitch: f32,
    /// The number of profiles, the last index a drag can reach.
    pub count: usize,
}

impl NavDrag {
    const THRESHOLD: f32 = 8.0;

    /// The new index of an item dragged `delta` pixels from `index`.
    pub fn target(index: usize, delta: f32, pitch: f32, count: usize) -> usize {
        let steps = (delta / pitch.max(1.0)).round() as isize;
        (index as isize + steps).clamp(0, count.saturating_sub(1) as isize) as usize
    }

    /// Handlers on one profile item; `command` is its `MoveProfile(name, index)`.
    pub fn bind(
        drag: &Rc<RefCell<NavDrag>>,
        inbox: &SharedInbox,
        element: &Com,
        origin: &Com,
        command: &Command,
    ) -> ComResult<Vec<Subscription>> {
        let Command::MoveProfile(name, index) = command.clone() else {
            return Ok(Vec::new());
        };
        let (press_drag, press_element, press_origin) =
            (drag.clone(), element.clone(), origin.clone());
        let pressed = Subscription::pointer(element, POINTER_PRESSED_EVENT, move |args| {
            let (_, y) = Subscription::position(&args, &press_origin)?;
            press_drag.borrow_mut().active = Some(Dragging {
                name: name.clone(),
                index,
                start: y,
                moved: false,
                element: press_element.clone(),
            });
            Ok(())
        })?;
        let (move_drag, move_origin) = (drag.clone(), origin.clone());
        let moved = Subscription::pointer(element, POINTER_MOVED_EVENT, move |args| {
            let mut drag = move_drag.borrow_mut();
            let Some(active) = drag.active.as_mut() else {
                return Ok(());
            };
            let (_, y) = Subscription::position(&args, &move_origin)?;
            if !active.moved && (y - active.start).abs() > Self::THRESHOLD {
                active.moved = true;
                Xaml::set_opacity(&active.element, 0.6)?;
            }
            Ok(())
        })?;
        let (release_drag, release_inbox, release_origin) =
            (drag.clone(), inbox.clone(), origin.clone());
        let released = Subscription::pointer(element, POINTER_RELEASED_EVENT, move |args| {
            let mut drag = release_drag.borrow_mut();
            let (pitch, count) = (drag.pitch, drag.count);
            let Some(active) = drag.active.take() else {
                return Ok(());
            };
            Xaml::set_opacity(&active.element, 1.0)?;
            if active.moved {
                let (_, y) = Subscription::position(&args, &release_origin)?;
                let target = Self::target(active.index, y - active.start, pitch, count);
                if target != active.index {
                    release_inbox
                        .borrow_mut()
                        .commands
                        .push(Command::MoveProfile(active.name, target));
                }
            }
            Ok(())
        })?;
        let lost_drag = drag.clone();
        let lost = Subscription::pointer(element, POINTER_CAPTURE_LOST_EVENT, move |_| {
            if let Some(active) = lost_drag.borrow_mut().active.take() {
                Xaml::set_opacity(&active.element, 1.0)?;
            }
            Ok(())
        })?;
        Ok(vec![pressed, moved, released, lost])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_moves_by_whole_items_within_the_list() {
        assert_eq!(NavDrag::target(0, 40.0, 38.0, 3), 1);
        assert_eq!(NavDrag::target(0, 15.0, 38.0, 3), 0);
        assert_eq!(NavDrag::target(2, -80.0, 38.0, 3), 0);
        assert_eq!(NavDrag::target(1, 500.0, 38.0, 3), 2);
    }
}
