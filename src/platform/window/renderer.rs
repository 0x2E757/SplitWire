//! Puts regions into their hosts: a region is reloaded only when its markup changed, its elements
//! get their event handlers, its fields are read every frame. A field that had the focus gets it
//! back after a reload, with its caret and selection ([`Restore`]).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::bound::BoundCommands;
use super::dropped::PendingItems;
use super::events::Subscription;
use super::focus::{Focused, Restore};
use super::markup::{Event, LineBreaks, Palette, Region};
use super::nav_drag::NavDrag;
use super::winrt::*;
use super::xaml::Xaml;
use crate::platform::com::{Com, ComResult};
use crate::presentation::{Command, InputId};

/// What handlers hand back to the frame loop.
#[derive(Default)]
pub struct Inbox {
    pub commands: Vec<Command>,
    /// Drags in progress over the drop zone, and drops.
    pub drops: Vec<PendingItems>,
    /// The field that has the focus.
    pub focused: Option<Focused>,
}

pub type SharedInbox = Rc<RefCell<Inbox>>;

struct MountedInput {
    id: InputId,
    element: Com,
    /// The field's text as the session holds it: lines broken by `\n`, not the TextBox's `\r`.
    shown: String,
}

struct Mounted {
    markup: String,
    /// Each bind's element and event; with the markup, what a reload depends on.
    binds: Vec<(String, Event)>,
    commands: BoundCommands,
    root: Com,
    _subscriptions: Vec<Subscription>,
    inputs: Vec<MountedInput>,
    children: HashMap<String, Mounted>,
}

/// Mounts regions; shared by the window and the page check.
pub struct Binder {
    pub inbox: SharedInbox,
    pub palette: Palette,
    pub drag: Rc<RefCell<NavDrag>>,
}

impl Binder {
    fn subscribe(
        &self,
        region: &Region,
        root: &Com,
        commands: &BoundCommands,
    ) -> ComResult<Vec<Subscription>> {
        let mut subscriptions = Vec::new();
        for (index, bind) in region.binds.iter().enumerate() {
            let element = Xaml::find(root, &bind.element)?;
            let push = commands.handler(&self.inbox, index);
            let subscription = match bind.event {
                Event::Click => Subscription::click(&element, push)?,
                Event::MenuClick => Subscription::menu_click(&element, push)?,
                Event::Enter => Subscription::enter(&element, push)?,
                Event::Toggled => Subscription::toggled(&element, push)?,
                Event::NavDrag => {
                    subscriptions.extend(NavDrag::bind(
                        &self.drag,
                        &self.inbox,
                        &element,
                        root,
                        &bind.command,
                    )?);
                    continue;
                }
                Event::Drop => {
                    subscriptions.extend(self.drop_zone(&element)?);
                    continue;
                }
            };
            subscriptions.push(subscription);
        }
        Ok(subscriptions)
    }

    fn drop_zone(&self, zone: &Com) -> ComResult<Vec<Subscription>> {
        let enter = self.inbox.clone();
        let leave = self.inbox.clone();
        let drop = self.inbox.clone();
        Ok(vec![
            Subscription::drag(
                zone,
                UI_ELEMENT_ADD_DRAG_ENTER,
                UI_ELEMENT_REMOVE_DRAG_ENTER,
                move |args| {
                    let pending = PendingItems::start(&args, true)?;
                    let mut inbox = enter.borrow_mut();
                    inbox.commands.push(Command::DragOver(Vec::new()));
                    inbox.drops.push(pending);
                    Ok(())
                },
            )?,
            Subscription::drag(
                zone,
                UI_ELEMENT_ADD_DRAG_OVER,
                UI_ELEMENT_REMOVE_DRAG_OVER,
                |args| PendingItems::accept(&args),
            )?,
            Subscription::drag(
                zone,
                UI_ELEMENT_ADD_DRAG_LEAVE,
                UI_ELEMENT_REMOVE_DRAG_LEAVE,
                move |_| {
                    leave.borrow_mut().commands.push(Command::DragLeave);
                    Ok(())
                },
            )?,
            Subscription::drag(
                zone,
                UI_ELEMENT_ADD_DROP,
                UI_ELEMENT_REMOVE_DROP,
                move |args| {
                    let pending = PendingItems::start(&args, false)?;
                    drop.borrow_mut().drops.push(pending);
                    Ok(())
                },
            )?,
        ])
    }

    fn inputs(
        &self,
        region: &Region,
        root: &Com,
    ) -> ComResult<(Vec<MountedInput>, Vec<Subscription>)> {
        let mut inputs = Vec::new();
        let mut subscriptions = Vec::new();
        for (id, text) in &region.inputs {
            let element = Xaml::find(root, &id.element())?;
            let focused = Focused::new(&id.element(), &element);
            let inbox = self.inbox.clone();
            let got = focused.clone();
            subscriptions.push(Subscription::got_focus(&element, move || {
                inbox.borrow_mut().focused = Some(got.clone());
                Ok(())
            })?);
            let inbox = self.inbox.clone();
            subscriptions.push(Subscription::lost_focus(&element, move || {
                let mut inbox = inbox.borrow_mut();
                if inbox.focused.as_ref() == Some(&focused) {
                    inbox.focused = None;
                }
                Ok(())
            })?);
            inputs.push(MountedInput {
                id: id.clone(),
                element,
                shown: text.clone(),
            });
        }
        Ok((inputs, subscriptions))
    }

    /// Mounts `region` and its children; a reload under the focused field leaves its focus in
    /// `restore`, unless one is waiting there already.
    fn mount(
        &self,
        host: &Com,
        region: &Region,
        slot: &mut HashMap<String, Mounted>,
        restore: &mut Option<Restore>,
    ) -> ComResult<()> {
        let binds: Vec<(String, Event)> = region
            .binds
            .iter()
            .map(|b| (b.element.clone(), b.event.clone()))
            .collect();
        let reload = slot
            .get(&region.host)
            .is_none_or(|m| m.markup != region.markup || m.binds != binds);
        if reload {
            if restore.is_none() {
                *restore = self.focus_in(slot.get(&region.host));
            }
            let root = Xaml::load(&self.palette.apply(&region.loadable()))?;
            Xaml::set_content(host, &root)?;
            let commands = BoundCommands::new(&region.binds);
            let mut subscriptions = self.subscribe(region, &root, &commands)?;
            let (inputs, focus_subscriptions) = self.inputs(region, &root)?;
            subscriptions.extend(focus_subscriptions);
            slot.insert(
                region.host.clone(),
                Mounted {
                    markup: region.markup.clone(),
                    binds,
                    commands,
                    root,
                    _subscriptions: subscriptions,
                    inputs,
                    children: HashMap::new(),
                },
            );
        }
        let mounted = slot.get_mut(&region.host).expect("mounted above");
        // Handlers of a region kept as it was raise this frame's commands.
        mounted.commands.update(&region.binds);
        mounted
            .children
            .retain(|host, _| region.children.iter().any(|c| c.host == *host));
        for child in &region.children {
            let child_host = Xaml::find(&mounted.root, &child.host)?;
            self.mount(&child_host, child, &mut mounted.children, restore)?;
        }
        Ok(())
    }

    /// The focus of the focused field in a region about to be reloaded.
    fn focus_in(&self, mounted: Option<&Mounted>) -> Option<Restore> {
        let focused = self.inbox.borrow().focused.clone()?;
        let input = Self::find_input(mounted?, &focused.name)?;
        (Focused::new(&focused.name, &input.element) == focused)
            .then(|| Restore::take(&focused.name, &input.element))
    }

    fn find_input<'a>(mounted: &'a Mounted, element: &str) -> Option<&'a MountedInput> {
        mounted
            .inputs
            .iter()
            .find(|i| i.id.element() == element)
            .or_else(|| {
                mounted
                    .children
                    .values()
                    .find_map(|c| Self::find_input(c, element))
            })
    }
}

/// The frame's regions, mounted in the frame skeleton's hosts.
pub struct Renderer {
    binder: Binder,
    root: Com,
    hosts: HashMap<String, Com>,
    mounted: HashMap<String, Mounted>,
    /// A reloaded field's focus, given back once the field can take it.
    restore: Option<Restore>,
}

impl Renderer {
    /// Loads the skeleton; `hosts` are its host names.
    pub fn new(skeleton: &str, hosts: &[&str], binder: Binder) -> ComResult<Self> {
        let root = Xaml::load(&binder.palette.apply(skeleton))?;
        let hosts = hosts
            .iter()
            .map(|h| Ok(((*h).to_owned(), Xaml::find(&root, h)?)))
            .collect::<ComResult<HashMap<_, _>>>()?;
        Ok(Self {
            binder,
            root,
            hosts,
            mounted: HashMap::new(),
            restore: None,
        })
    }

    pub fn root(&self) -> &Com {
        &self.root
    }

    pub fn render(&mut self, regions: &[Region]) -> ComResult<()> {
        let mut restore = self.restore.take();
        let mounted = regions.iter().try_for_each(|region| {
            let Some(host) = self.hosts.get(&region.host) else {
                return Ok(());
            };
            self.binder
                .mount(host, region, &mut self.mounted, &mut restore)
        });
        self.restore = restore;
        self.refocus();
        mounted
    }

    /// The field whose focus is waiting to be given back.
    pub fn restoring(&self) -> Option<&str> {
        self.restore.as_ref().map(|r| r.name.as_str())
    }

    /// Gives a reloaded field its focus back once it can take it; every frame.
    pub fn refocus(&mut self) {
        let Some(mut restore) = self.restore.take() else {
            return;
        };
        let element = self
            .fields()
            .into_iter()
            .find(|(id, _)| id.element() == restore.name)
            .map(|(_, element)| element);
        if !restore.give(element.as_ref()) {
            self.restore = Some(restore);
        }
    }

    /// Fields whose text changed since the last poll, in the session's form.
    pub fn poll_inputs(&mut self) -> Vec<Command> {
        fn walk(mounted: &mut Mounted, out: &mut Vec<Command>) {
            for input in &mut mounted.inputs {
                if let Ok(text) = Xaml::text(&input.element).map(|t| LineBreaks::typed(&t))
                    && text != input.shown
                {
                    input.shown = text.clone();
                    out.push(Command::Input(input.id.clone(), text));
                }
            }
            for child in mounted.children.values_mut() {
                walk(child, out);
            }
        }
        let mut commands = Vec::new();
        for mounted in self.mounted.values_mut() {
            walk(mounted, &mut commands);
        }
        commands
    }

    /// Every mounted field with its TextBox.
    pub fn fields(&self) -> Vec<(InputId, Com)> {
        fn walk(mounted: &Mounted, out: &mut Vec<(InputId, Com)>) {
            for input in &mounted.inputs {
                out.push((input.id.clone(), input.element.clone()));
            }
            for child in mounted.children.values() {
                walk(child, out);
            }
        }
        let mut fields = Vec::new();
        for mounted in self.mounted.values() {
            walk(mounted, &mut fields);
        }
        fields
    }

    /// Writes text the session replaced into the shown fields.
    pub fn reset(&mut self, resets: &[(InputId, String)]) {
        fn walk(mounted: &mut Mounted, id: &InputId, text: &str) {
            for input in &mut mounted.inputs {
                if input.id == *id && input.shown != text {
                    input.shown = text.to_owned();
                    let _ = Xaml::set_text(&input.element, &LineBreaks::shown(text));
                }
            }
            for child in mounted.children.values_mut() {
                walk(child, id, text);
            }
        }
        for (id, text) in resets {
            for mounted in self.mounted.values_mut() {
                walk(mounted, id, text);
            }
        }
    }

    /// The commands the mounted region at `host` raises now; regions inside others by their own
    /// host name.
    pub fn commands(&self, host: &str) -> Option<Vec<Command>> {
        fn walk(slot: &HashMap<String, Mounted>, host: &str) -> Option<Vec<Command>> {
            slot.get(host)
                .map(|m| m.commands.current())
                .or_else(|| slot.values().find_map(|m| walk(&m.children, host)))
        }
        walk(&self.mounted, host)
    }

    /// The page's scroll viewer, scrolled back to its top.
    pub fn scroll_top(&self, name: &str) {
        fn walk(mounted: &Mounted, name: &str) -> Option<Com> {
            Xaml::find(&mounted.root, name)
                .ok()
                .or_else(|| mounted.children.values().find_map(|c| walk(c, name)))
        }
        for mounted in self.mounted.values() {
            if let Some(viewer) = walk(mounted, name) {
                let _ = Xaml::scroll_to_top(&viewer);
            }
        }
    }
}
