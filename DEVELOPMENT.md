# SilkMark development notes

## Source layout

- `src/main.rs` — GTK process bootstrap and signal callbacks only.
- `src/app.rs` — application state and small shared context types.
- `src/app/core.rs` — tab creation, session/state synchronization and reader-level UI state.
- `src/app/navigation.rs` — history, URL navigation, search, completion and tab navigation.
- `src/app/bookmarks_ui.rs` — bookmark UI operations.
- `src/app/cache.rs` — document/image memory cache, async polling and image queue.
- `src/app/render.rs` — tab render entry point and sidebar rendering.
- `src/app/docview.rs` — virtualized document view: a `GtkListView` per tab with one row per `md::Block`, row widget builders, search highlighting and section/position jumps.
- `src/md.rs` — block Markdown parser and public document model.
- `src/md/blocks.rs` — display blocks, heading anchors and search text derived from a parsed document (built on the worker thread).
- `src/md/inline.rs` — inline Markdown parsing, links, entities and table-cell inline markup.
- `src/md/tests.rs` — Markdown regression tests, kept out of production parser code.
- `src/net.rs` — URL normalization, local file access, libcurl HTTPS and disk cache.
- `src/math.rs` — lightweight math parser/renderer.
- `src/highlight.rs` — lightweight code syntax highlighting.
- `src/ffi.rs` — raw GTK/GDK/Cairo/Glib FFI surface.
- `STABILIZATION.md` — v0.40 release gate and v1.0 feature-freeze rules.

## Threading and rendering

- Fetching *and* parsing (`md::parse`, including the block index) run on a worker thread; the UI thread only receives the finished `Document`.
- GTK widgets are created on the UI thread only, and only for the rows the list view asks for (`bind`). Opening a 1 MB document does not build thousands of widgets.
- `GtkListView` unbinds and rebinds the same rows many times while scrolling (rows differ a lot in height, so its window of realized rows keeps shifting). Rebuilding a row each time made scrolling stutter, so `DocViewCtx` keeps built row widgets in an LRU cache (`ROW_CACHE`) and reuses them. The cache is dropped when the search highlight changes or a new document is shown.
- Measure scrolling, not only opening: a smooth-scroll check is a loop that moves the vertical adjustment every 16 ms and records the frame clock deltas.
- Search counts hits from the precomputed `BlockIndex::search_text`; it never rebuilds the document. Only the visible rows are rebuilt to move the highlight.
- A reading position is stored as `block index + fraction of the block scrolled past`, because pixel offsets are only estimates in a virtualized list.
- `gtk_list_view_scroll_to` needs GTK 4.12. It is looked up with `dlsym`; older GTK 4 falls back to an adjustment estimate that the row-alignment step corrects once the row exists.

## Clean-code rules

1. Keep `main.rs` focused on bootstrap and callbacks; application behavior belongs in `app/*`.
2. Keep network, Markdown parsing, rendering and persistence separated.
3. Prefer small single-purpose functions and explicit names over comments explaining dense code.
4. Keep unsafe/FFI code at the GTK/libcurl boundary; do not spread raw-pointer manipulation into parser/business logic.
5. Avoid new global state unless it represents process-wide configuration.
6. Avoid `unwrap()` in runtime paths unless the invariant is local and obvious; prefer `Result`/`Option` propagation.
7. New Markdown behavior requires a regression test in `src/md/tests.rs`.
8. The project targets Edition 2024 and follows the stable Rust channel via `rust-toolchain.toml`; Cargo.toml does not declare an MSRV.
9. Run before release (or execute `tools/release-check.sh`):

```bash
rustc --version
cargo fmt --all
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

## Rust 2024 FFI safety policy

SilkMark intentionally uses direct C ABI bindings for GTK, GDK-Pixbuf, Cairo, GLib, and libcurl.
Functions that cross those ABI boundaries are declared `unsafe`; operations inside those boundary functions are therefore covered by the crate-level `unsafe_op_in_unsafe_fn` policy. Keep ordinary parsing, URL, cache, and data-model code safe Rust. New unsafe code should stay localized to FFI adapters/callbacks and document any non-obvious pointer lifetime assumption.

### Diagram modules

- `src/graph.rs` owns the shared graph model, layered layout, and Cairo rendering.
- `src/mermaid.rs` parses the supported Mermaid flowchart subset into the shared graph model.
- `src/graphviz.rs` parses the supported Graphviz/DOT subset into the same graph model.

Keep syntax-specific parsing out of the renderer so future diagram front ends can reuse the same layout and drawing code.
