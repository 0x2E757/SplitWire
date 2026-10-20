# Alignment

Part of [Window](../window.md). The UI kit lines a row's marks up on one middle line (as CSS `align-items:
center` would), and a title with a smaller subtitle on one baseline.

## Text in a row

A TextBlock in "Segoe UI Variable Text" has a line of 1.2 em with its baseline at 0.9 em, so its capitals sit
above the middle of its line, and a 14.4 px line rounds to 15 px: a dot or icon centred beside it lands a pixel
low. A single-line text in a row (`Kit::line`) takes a block line height of 1.4 em, rounded to an even pixel
count: XAML puts the baseline at 3/4 of it, which puts the middle of the capitals in the middle of the line.

- The header's title is trimmed to its capitals and the subtitle to its baseline, both at the bottom of one
  StackPanel (`Kit::titled`): the baselines meet whatever the sizes, and the title is centred like the controls.
- A TextBox has no 32 px floor (`TextControlThemeMinHeight` 0) and 2 px under its line, whose baseline is at
  1.079 em; centred in the field's frame, its capitals are in the middle. Its placeholder takes the typed text's
  font, so the text does not move when typing starts.
- The switch's label is a ContentPresenter's text, which no implicit style reaches: the template gives it the
  font and line of the other texts.
- An icon before a title that may wrap stands in the middle of the title's first line (`Kit::first_line`:
  InfoBars, alerts, the Add page's facts).
- A card or row without a description or caption has no empty line under its title.
- A Status card's stats are three lines in equal columns, the labels, the values and the captions, each an
  `align:baseline` row (`Kit::spread_stats`), so the values share a baseline whatever their captions.

## The check

`--verify-pages` lays every state out in a XAML island of a window that is never shown (`HiddenHost`: a hidden
tool window far off the screen, no `WS_VISIBLE`, never activated), then reads the `Tag` marks of `Align`:

| Tag | Checked |
|---|---|
| `align:row` | every mark's middle within 1 px of the others and of the row's middle |
| `align:baseline` | every text's baseline within 1 px |
| `align:block` | one mark: the box of what it shows (an empty line is not part of it) |
| `align:free` | left out (the bell's count on its corner) |
| `align:cover` | its first child, the click target, covers it (a Routes card's header) |

A text's middle is the middle of its capitals (0.7 em above the baseline); a TextBox's is computed from its text
view. Icons, dots, swatches and frames count by their boxes. The problems are printed once each, with the first
state that shows them and how many do.

## Pictures

`SplitWire.Window.exe --render-pages <folder>` lays out every state of `--verify-pages` the same way and writes a
PNG of each (`RenderTargetBitmap`, 1280 × 1000 and 960 × 1000), and of every profile menu (`menu-<name>`, 320 ×
260), for reviewing the look. Nothing is shown.
