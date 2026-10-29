# Proposals for reading book-length documents

Source: two real transcribed books used as test input, parsed with the unmodified
`src/md*.rs` in a standalone harness (no GTK4 development files were available, so
nothing below was measured in the GUI):

- Patrick Olivelle, *The Early Upaniṣads* — 1.3 MB, 13 126 lines, 153 headings, 206 table lines
- Richard Rutt, *Zhouyi* — 975 KB, 12 219 lines, 649 headings, 376 quote lines, 717 table lines

Parsing is not a problem: both documents parse in 40–60 ms (release build).

## Done on this branch

1. **Backslash hard line break** (parser fix). A line ending in an unescaped `\` followed by another
   line of the same paragraph is now a hard break (CommonMark). Before, 358 lines in the
   Olivelle book (title page, verses) showed a literal `\` and ran together on one line.
2. **Nested lists four or more columns deep** (parser fix). `    - item` directly after a list item
   was taken for an indented code block, so every third list level (45 blocks in
   Olivelle's per-Upaniṣad contents) rendered as a monospaced `- ...` blob. It now stays
   a nested list item. Two previously red tests (`nested_list_indent_is_kept`,
   `nested_lists_keep_increasing_indent`) pass again.

3. **Large documents (virtualized view, background parsing).** Before, `render_tab` created
   one `GtkLabel` per line in a single synchronous pass on the UI thread, and every find
   keystroke or fragment link rebuilt the whole document. Now parsing runs on the worker
   thread and a `GtkListView` creates widgets only for visible blocks. Measured here
   (release build; render times from `--stats`, the find and fragment rows follow from the
   old code path calling `render_tab` and were not timed separately in the new one):

   | | before | after |
   | --- | --- | --- |
   | Olivelle: render on the UI thread | 840–1100 ms (3 renders at open) | 55–77 ms (1 render) |
   | Zhouyi: render on the UI thread | 900 ms | 71 ms |
   | Resident memory, Olivelle / Zhouyi | 345 / 307 MB | 168 / 166 MB |
   | Find: each keystroke / next hit | full re-render, about 1 s | recount + a few visible rows |
   | Same-document fragment link | full re-render | scroll |

   Chunked rendering was not needed on top of this: rows are built on demand.

   Scrolling needed a second fix. The first virtualized version opened fast but stuttered
   while scrolling (frame clock, 10 000 px scrolled in 16 ms steps: 95th percentile 117 ms,
   60 frames over 25 ms; the old non-virtualized build: 17 ms, 3 frames). GTK kept rebinding
   the same rows (a block was bound 6 times on average) and each bind built a new widget.
   Caching the row widgets brought it to 17 ms and 1 slow frame. A time-sliced build of the
   whole document into a box was tried too: the window stayed responsive but building the
   widgets for the whole book took 7.5 s, so it was dropped.

## Proposed, not implemented

Ordered by expected value for long books.

1. **Sidebar contents for books.** The Contents list stops after 200 headings and
   truncates titles at 34 characters. Zhouyi has 649 headings, so the contents ends
   roughly a third of the way into the book. Options: collapsible H2 sections (open only
   the current one), a filter entry, full title as tooltip.
2. **Diacritic-insensitive search.** Search lowercases only (`BlockIndex::search_text`), so `Upanisad`
   does not find `Upaniṣad`, and `Brhadaranyaka` does not find `Bṛhadāraṇyaka`. Fold
   combining marks and the common Latin-extended letters (ā ī ū ṛ ṣ ṇ ṭ ś ṃ ḥ ñ ü) on both
   sides, with a toggle or a smart-case rule. This matters for Sanskrit and pinyin texts.
   The text index is now built once per document, so folding there costs nothing per search.
3. **Section navigation keys.** `]` / `[` for next/previous heading and a one-line
   "current section" indicator, so a reader can orient themselves in a 13 000-line page.
   Reader zoom (80–160 %) already exists; a maximum text column width would be the
   companion setting for wide windows.
4. **Footnotes in plain-text books.** Olivelle marks notes with superscripts (¹ ²) and
   keeps the text under `### Notes` at the end of a section. A heuristic that links
   superscript digits to the matching entry would be useful, but it is guesswork on
   unstructured text; consider only as an opt-in.
5. **Test suite.** Under the unmodified repo layout `src/md/tests.rs` refers to
   `../examples/regression-suite.md` (resolves to `src/examples/`, which does not exist)
   and calls the private `inline::parse_inline`. I could not run `cargo test` here
   (GTK4 development files missing), but in my harness both stopped compilation until
   I shimmed them. After that, 6 of 103 tests still fail (ordered lists over nine digits,
   GFM table alignment, task + strikethrough, quoted lists, spaced thematic breaks, plus
   lists); the same 6 fail without my changes. See the existing note about the red suite.
