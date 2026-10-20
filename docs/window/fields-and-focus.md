# Fields and focus

Part of [Window](../window.md).

A region that reloads replaces its TextBoxes, and a replaced TextBox has lost the focus. So typing into a field
reloads neither the region that holds it nor any region around it.

## Live parts

A field's text is not in the markup: the region holds `§name§`, filled when it loads. What a key changes
around the field is a live part: `Out::live` puts it into a ContentControl of its own as a child region, so a
change reloads only that part. They are a field's red frame, the Add buttons and the as-you-type verdicts, the
editors' line gutters, an expander's frame, badge and summary, a settings row's stripe and texts and its
"Use …" fix, the MTU row's "Open" link, the log level counts and the paused bar, the Lists groups under
their search, and the import review's title, notes and expanders, which follow the typed name.

Anything that appears or disappears with the typed text goes into a live part too: element names are numbered
in order, so an element that comes or goes renames the ones after it and changes the region's markup. A live
part that may be empty carries its own margins, so the panel around it needs no spacing for it.

## A reload under the focused field

The service's state may still change a region with the focused field. `Restore` then takes the field's caret
and selection before the reload, puts them into the new TextBox and focuses it, trying again on the next frames:
a control just put into its host joins the live tree, and takes the focus, only after the next layout. The
focus record names the TextBox and not only the field, so the LostFocus of a replaced TextBox does not clear
its successor's record.

## Line breaks

A TextBox breaks lines with `\r` and keeps one line unless `AcceptsReturn` precedes `Text`; `LineBreaks` writes
`\r` and reads back `\n`, so an untouched field raises no `Input`.

## Checks

The unit test `typing_reloads_no_region_around_the_field` types into every field of every state of the
`Showcase`, key by key and back, then lets a second pass, and compares the markup of the regions around the
field. `--verify-pages` does the same with the mounted TextBoxes: the text put into the TextBox, the caret at
its end, then the frame loop's poll, commands, render and resets. The field must stay the same TextBox with its
text and caret, and a forced reload under a focused field must hand its caret and selection on.
