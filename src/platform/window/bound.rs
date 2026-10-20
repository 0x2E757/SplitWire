//! The commands a mounted region's elements raise. A command is not part of a region's markup, so
//! a region whose markup did not change is not reloaded, and its handlers would keep the command
//! of the frame that loaded it (a Copy with the text of then). The handlers look their command up
//! here when they fire, and every render puts the frame's commands here.

use std::cell::RefCell;
use std::rc::Rc;

use super::markup::Bind;
use super::renderer::SharedInbox;
use crate::platform::com::ComResult;
use crate::presentation::Command;

#[derive(Clone, Default)]
pub struct BoundCommands(Rc<RefCell<Vec<Command>>>);

impl BoundCommands {
    pub fn new(binds: &[Bind]) -> Self {
        let commands = Self::default();
        commands.update(binds);
        commands
    }

    /// The commands of this frame's binds, in the binds' order.
    pub fn update(&self, binds: &[Bind]) {
        *self.0.borrow_mut() = binds.iter().map(|b| b.command.clone()).collect();
    }

    pub fn current(&self) -> Vec<Command> {
        self.0.borrow().clone()
    }

    /// The handler of bind `index`: it hands the bind's command of the latest frame to the inbox.
    pub fn handler(
        &self,
        inbox: &SharedInbox,
        index: usize,
    ) -> impl Fn() -> ComResult<()> + 'static {
        let (commands, inbox) = (self.0.clone(), inbox.clone());
        move || {
            if let Some(command) = commands.borrow().get(index) {
                inbox.borrow_mut().commands.push(command.clone());
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::window::markup::Event;
    use crate::platform::window::renderer::Inbox;

    fn copy(text: &str) -> Bind {
        Bind {
            element: "E1".into(),
            event: Event::Click,
            command: Command::Copy(text.into()),
        }
    }

    #[test]
    fn a_handler_raises_the_command_of_the_latest_frame() {
        let inbox: SharedInbox = Rc::new(RefCell::new(Inbox::default()));
        let commands = BoundCommands::new(&[copy("old lines")]);
        let click = commands.handler(&inbox, 0);
        click().unwrap();
        // The next frame: the same markup, so no reload, but new text to copy.
        commands.update(&[copy("new lines")]);
        click().unwrap();
        assert_eq!(
            inbox.borrow().commands,
            [
                Command::Copy("old lines".into()),
                Command::Copy("new lines".into())
            ]
        );
    }
}
