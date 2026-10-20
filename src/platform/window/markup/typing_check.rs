//! The markup half of the typing check: while a field is typed into, the region that holds it and
//! every region around it keep their markup, so the window reloads none of them and the field keeps
//! the focus. `--verify-pages` types into the mounted TextBoxes the same way.

use std::time::Duration;

use super::{Frame, Region};
use crate::presentation::showcase::{Driver, Showcase};
use crate::presentation::transport::demo::Scenario;
use crate::presentation::{Command, InputId, WidthClass};

/// The markup of each region from the frame down to the one holding the field, by host.
fn around(regions: &[Region], id: &InputId) -> Option<Vec<(String, String)>> {
    regions.iter().find_map(|region| {
        let mut path = if region.inputs.iter().any(|(i, _)| i == id) {
            Vec::new()
        } else {
            around(&region.children, id)?
        };
        path.insert(0, (region.host.clone(), region.markup.clone()));
        Some(path)
    })
}

fn fields(regions: &[Region], out: &mut Vec<(InputId, String)>) {
    for region in regions {
        out.extend(region.inputs.iter().cloned());
        fields(&region.children, out);
    }
}

/// Where two markups part, with a little of each around it.
fn difference(before: &str, after: &str) -> String {
    let at = before
        .char_indices()
        .zip(after.chars())
        .find(|((_, a), b)| a != b)
        .map_or(before.len().min(after.len()), |((i, _), _)| i);
    let from = before[..at]
        .char_indices()
        .rev()
        .nth(80)
        .map_or(0, |(i, _)| i);
    let show = |text: &str| text[from..].chars().take(200).collect::<String>();
    format!("\n  before: {}\n  after:  {}", show(before), show(after))
}

fn state(label: &str, driver: &mut Driver, width: WidthClass, failures: &mut Vec<String>) {
    let mut ids = Vec::new();
    fields(&Frame::regions(&driver.session.view(), width), &mut ids);
    for (id, text) in ids {
        let Some(before) = around(&Frame::regions(&driver.session.view(), width), &id) else {
            continue;
        };
        // Each key, then a second of the frame loop with the text typed back.
        let keys = Showcase::keystrokes(&id, &text).into_iter().map(Some);
        for key in keys.chain([None]) {
            match &key {
                Some(key) => driver.run(Command::Input(id.clone(), key.clone())),
                None => driver.later(Duration::from_secs(1)),
            }
            let after = around(&Frame::regions(&driver.session.view(), width), &id);
            let changed = after.as_ref().map_or(Some(None), |after| {
                before
                    .iter()
                    .zip(after)
                    .find(|(b, a)| b != a)
                    .map(|(b, a)| Some((b, a)))
            });
            if let Some(changed) = changed {
                let what = match changed {
                    Some(((host, b), (_, a))) => format!("{host} changed{}", difference(b, a)),
                    None => "the field is gone".into(),
                };
                let step = key.map_or("a second later".into(), |k| format!("typing {k:?}"));
                failures.push(format!(
                    "{label} {width:?}: {step} into {}: {what}",
                    id.element()
                ));
                driver.run(Command::Input(id.clone(), text.clone()));
                break;
            }
        }
    }
}

#[test]
fn typing_reloads_no_region_around_the_field() {
    let mut failures = Vec::new();
    for scenario in Scenario::ALL {
        let states = Showcase::views(scenario).len();
        for at in 0..states {
            for width in WidthClass::ALL {
                let mut index = 0;
                Showcase::walk(scenario, |label, driver| {
                    if index == at {
                        state(label, driver, width, &mut failures);
                    }
                    index += 1;
                });
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
