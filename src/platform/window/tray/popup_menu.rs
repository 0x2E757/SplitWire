//! The tray's right-click menu as a Win32 popup menu (`TrackPopupMenuEx` with submenus): each
//! item's text with its key right-aligned after a tab, its mark as a 32-bit bitmap, the default
//! item bold. Built from the view, shown at the pointer, the chosen item's action returned.

use std::io;
use std::ptr;

use super::abi::*;
use crate::platform::abi::{Raw, wide};
use crate::presentation::tray::{MenuArt, TrayAction, TrayEntry, TrayItem};

/// A premultiplied 32-bit bitmap for a menu item, deleted with the menu.
struct Bitmap(Raw);

impl Bitmap {
    fn of(pixels: &[u32], size: u32) -> io::Result<Self> {
        let header = BitmapInfoHeader {
            size: std::mem::size_of::<BitmapInfoHeader>() as u32,
            width: size as i32,
            // Negative: rows top-down, as the canvas has them.
            height: -(size as i32),
            planes: 1,
            bit_count: 32,
            compression: 0,
            image_size: 0,
            x_per_meter: 0,
            y_per_meter: 0,
            used: 0,
            important: 0,
        };
        let mut bits: Raw = ptr::null_mut();
        // SAFETY: a DIB section of size × size 32-bit pixels; the copy fills exactly them. Each
        // 0xAARRGGBB is B, G, R, A in memory: the DIB's order.
        unsafe {
            let bitmap =
                CreateDIBSection(ptr::null_mut(), &header, 0, &mut bits, ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                return Err(io::Error::last_os_error());
            }
            ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u32, pixels.len());
            Ok(Self(bitmap))
        }
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        // SAFETY: the bitmap is ours; the menu that showed it is gone first.
        unsafe {
            DeleteObject(self.0);
        }
    }
}

pub struct PopupMenu {
    menu: Raw,
    /// The action of item id `n` is at `n - 1`.
    actions: Vec<TrayAction>,
    items: usize,
    bitmaps: Vec<Bitmap>,
}

impl PopupMenu {
    /// The menu of `items`, its marks drawn for a dark or light menu at `mark` pixels.
    pub fn build(items: &[TrayItem], dark: bool, mark: u32) -> io::Result<Self> {
        let mut popup = Self {
            menu: Self::create()?,
            actions: Vec::new(),
            items: 0,
            bitmaps: Vec::new(),
        };
        let menu = popup.menu;
        popup.fill(menu, items, dark, mark)?;
        Ok(popup)
    }

    /// Every item, submenus' included.
    pub fn items(&self) -> usize {
        self.items
    }

    fn create() -> io::Result<Raw> {
        // SAFETY: no arguments.
        let menu = unsafe { CreatePopupMenu() };
        if menu.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(menu)
    }

    fn fill(&mut self, menu: Raw, items: &[TrayItem], dark: bool, mark: u32) -> io::Result<()> {
        for (position, item) in items.iter().enumerate() {
            let mut info = MenuItemInfo::new();
            // The text the item points to, kept until it is inserted.
            let _text = match item {
                TrayItem::Separator => {
                    info.kind = MFT_SEPARATOR;
                    Vec::new()
                }
                TrayItem::Entry(entry) => self.entry(&mut info, entry, dark, mark)?,
                TrayItem::Submenu(entry, children) => {
                    let _text = self.entry(&mut info, entry, dark, mark)?;
                    info.mask |= MIIM_SUBMENU;
                    // Inserted before it is filled: the root's DestroyMenu then takes it along.
                    info.submenu = Self::create()?;
                    Self::insert(menu, position, &info)?;
                    self.items += 1;
                    self.fill(info.submenu, children, dark, mark)?;
                    continue;
                }
            };
            Self::insert(menu, position, &info)?;
            self.items += 1;
        }
        Ok(())
    }

    /// The item's text, state, id and mark; the text must outlive the insert.
    fn entry(
        &mut self,
        info: &mut MenuItemInfo,
        entry: &TrayEntry,
        dark: bool,
        mark: u32,
    ) -> io::Result<Vec<u16>> {
        // `&` marks a mnemonic in a menu; names and titles show it as it is.
        let mut label = entry.label.replace('&', "&&");
        if let Some(key) = &entry.key {
            label = format!("{label}\t{}", key.replace('&', "&&"));
        }
        let mut text = wide(&label);
        info.mask |= MIIM_STRING | MIIM_STATE | MIIM_ID;
        info.text = text.as_mut_ptr();
        info.length = (text.len() - 1) as u32;
        if !entry.enabled {
            info.state |= MFS_DISABLED;
        }
        if entry.default {
            info.state |= MFS_DEFAULT;
        }
        if let Some(action) = &entry.action {
            self.actions.push(action.clone());
            info.id = self.actions.len() as u32;
        }
        if let Some(canvas) = MenuArt::canvas(&entry.lead, dark, mark) {
            let bitmap = Bitmap::of(&canvas.premultiplied(), mark)?;
            info.mask |= MIIM_BITMAP;
            info.bitmap = bitmap.0;
            self.bitmaps.push(bitmap);
        }
        Ok(text)
    }

    fn insert(menu: Raw, position: usize, info: &MenuItemInfo) -> io::Result<()> {
        // SAFETY: the item's strings and handles live through the call.
        match unsafe { InsertMenuItemW(menu, position as u32, 1, info) } {
            0 => Err(io::Error::last_os_error()),
            _ => Ok(()),
        }
    }

    /// Shows the menu at the pointer for `owner` until it closes; the chosen item's action.
    pub fn track(&self, owner: Raw) -> Option<TrayAction> {
        let mut point = Point::default();
        // SAFETY: the menu and the window are ours. Without the foreground the menu would not
        // close on a click elsewhere; the posted message lets it close at once (KB135788).
        let chosen = unsafe {
            GetCursorPos(&mut point);
            SetForegroundWindow(owner);
            let chosen = TrackPopupMenuEx(
                self.menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
                point.x,
                point.y,
                owner,
                ptr::null_mut(),
            );
            PostMessageW(owner, 0, 0, 0);
            chosen
        };
        usize::try_from(chosen)
            .ok()
            .and_then(|id| id.checked_sub(1))
            .and_then(|index| self.actions.get(index).cloned())
    }

    /// The items of the menu itself, not of its submenus.
    pub fn top_level(&self) -> usize {
        usize::try_from(Self::count(self.menu)).unwrap_or(0)
    }

    fn count(menu: Raw) -> i32 {
        // SAFETY: a menu of ours.
        unsafe { GetMenuItemCount(menu) }
    }
}

impl Drop for PopupMenu {
    fn drop(&mut self) {
        // SAFETY: destroys the menu and its submenus; the bitmaps go after it.
        unsafe {
            DestroyMenu(self.menu);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::showcase::Driver;
    use crate::presentation::transport::demo::Scenario;

    /// Menus are built, never shown: nothing appears on the screen.
    #[test]
    fn every_scenario_builds_its_menu_without_showing_it() {
        for scenario in Scenario::ALL {
            let view = Driver::new(scenario).session.tray();
            for dark in [false, true] {
                let menu = PopupMenu::build(&view.menu, dark, 20).unwrap();
                assert_eq!(PopupMenu::count(menu.menu), view.menu.len() as i32);
                assert!(menu.items() >= view.menu.len());
            }
        }
        let view = Driver::new(Scenario::Normal).session.tray();
        let menu = PopupMenu::build(&view.menu, false, 16).unwrap();
        assert_eq!(menu.actions.last(), Some(&TrayAction::Exit));
        assert!(menu.bitmaps.len() >= 4, "the header, the dots, the alerts");
    }
}
