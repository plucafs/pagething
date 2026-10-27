# pagething — V1 Plan

Infinite canvas of EPUB pages. Desktop Rust app, egui/eframe.

## Decisions (locked)

| Area | Decision |
|---|---|
| Stack | Rust, eframe 0.37 / egui 0.37 (MIT OR Apache), desktop only |
| Book pipeline | XHTML → Markdown (`htmd`) → blocks (`pulldown-cmark`) → egui Galley |
| Pagination | Fixed-size pages (~600×900), **block-measure packing**: measure blocks, pack until page full |
| EPUB parsing | `rbook` 0.7 (Apache-2.0) — raw XHTML, images, cover |
| Page look | White rectangles (paper) or dark reading pages; custom margins; global page number, number only, bottom-center |
| Canvas | Pages flow left→right in N **columns** (wraps continuously); no rows setting |
| Pan/zoom | MMB-drag pans, wheel zooms toward cursor, two-finger scroll pans, Ctrl+0 resets |
| Open | Menu bar: **File → Open (Ctrl+O)**, **Recent** submenu (last 10), **Quit**. Empty canvas + hint on launch. Single book at a time |
| Jump | **Ctrl+G** dialog, type page number |
| Layout | **Ctrl+L** dialog, type columns; reflows live |
| Search | **Ctrl+F** dialog, results list with snippets, click/Enter jumps; match highlighted yellow on target page until next search |
| Selection | Left-drag on text selects (hit-test); left-drag on background does nothing; **Ctrl+C** copies; never crosses a page boundary |
| Settings | **Ctrl+,** dialog: margins, theme (Light/Dark/System), page mode (paper/dark pages) |
| Persistence | Per-book viewport (pan + zoom) in views.json; recents in recents.json; columns/margins/theme later |
| Loading | Async: background thread parses EPUB + decodes images; "Loading…" state; galley layout on UI thread (font atlas is single-writer) |
| Fonts | Bundled DejaVuSans + Bold (`assets/fonts/`), `FontFamily::Name("pagething-bold")` — egui 0.37 has no font-weight API |
| Zoom text | Crisp at any zoom: visible galleys are re-laid at zoom× font sizes (DisplayCache keyed by block+zoom); mesh-stretch fallback if row counts drift |
| Out of v1 | Annotations/bookmarks, TOC, multi-book, WASM, theming beyond L/D/System |

## Crates

```toml
eframe = "0.37"              # app shell (egui 0.37)
rbook = { version = "0.7", default-features = false }  # EPUB 2/3 parse
htmd = "0.5"                 # XHTML -> Markdown
pulldown-cmark = "0.13"      # Markdown -> block events
rfd = "0.15"                 # native file dialogs (File > Open)
serde = { version = "1", features = ["derive"] }
serde_json = "1"             # settings + recents storage
dirs = "6"                   # config dir
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "gif", "webp"] }  # EPUB images
```

Notes:
- **Not** `egui_commonmark` — it can't do text selection. We render blocks via our own Galley-based renderer so selection falls out of the design.
- **Not** `epub` (GPL-3.0) or `epub-parser` (strips HTML). `rbook` gives raw XHTML for `htmd` and covers via `manifest().cover_image()`.
- Clipboard via `ctx.copy_text()` (egui built-in). No extra crate.

## Modules

```
src/
  main.rs            eframe entry point
  app.rs             Phases (Empty | Loading | Ready), menu bar, shortcuts, recents persistence
  book/
    load.rs          rbook open -> spine XHTML chunks, image map, cover bytes
    convert.rs       htmd: XHTML -> Markdown
    blocks.rs        pulldown-cmark -> Vec<Block>  (Heading/Para/List/Quote/Image/Rule)
    paginate.rs      BlockMetrics -> Vec<Page> via block-measure packing (pure, tested)
    prepare.rs       background-thread job: load + convert + blocks + image decode
  render/
    fonts.rs         DejaVu registration, bold family helper
    layout.rs        measure + build Galley for one Block at content width  (SINGLE source of truth)
    page_view.rs     paint page: rect, margins, rows, images, cover, page number
  canvas/
    view.rs          pan/zoom state, screen<->world transforms, Ctrl+0 reset
    grid.rs          page i -> world rect given columns + gap
  features/ (later milestones)
    selection.rs     hit-test text vs bg, range within page, Ctrl+C copy
    search.rs        per-page plain text index, query -> Vec<Hit{page, snippet, range}>
    dialogs.rs       Ctrl+G jump, Ctrl+L columns, Ctrl+F search, Ctrl+, settings
```

## Key technical rules

1. **Layout is done once.** `render::layout` measures and builds every block's Galley at load time (content width = page width − 2×margin). Pagination, painting, hit-testing, and search-highlight all consume the same stored Galleys. This guarantees no drift between measure and draw.
2. **Fonts before pagination.** Run pagination on the first frame after `ctx.fonts()` is ready; keep pages cached. Margins/columns changes trigger repaginate or regrid.
3. **Cover = page 1** (full-bleed image inside the white rect). Content pages follow; global numbering = position in Vec<Page>.
4. **Selection** uses `Galley::cursor_from_pos` / `pos_from_cursor` for glyph-accurate ranges; selection highlight painted under text, cleared on next left-press elsewhere.
5. **Search** indexes `page.plain_text` (concatenated block text). One active highlight set (yellow) replaced on each new search.
6. **Viewport position** = world point at screen center + zoom; restored on open of the same book.

## Milestones (build order)

1. **Shell + canvas** — eframe app, pan/zoom (MMB/wheel/2-finger/Ctrl+0), grid of placeholder white pages with numbers, columns from a constant. **DONE**
2. **Book pipeline** — `book/*`: open EPUB, XHTML→MD→blocks, unit-test on a sample book. **DONE**
3. **Open + pagination + page rendering** — menu bar (File/Open/Recent/Quit), Ctrl+O + rfd, async load with "Loading…", block-measure packing, real content on pages, cover page 1, DejaVu bold, images. **DONE**
4. **Navigation** — Ctrl+G jump (center, keep zoom), Ctrl+L columns reflow (1–16, live), columns persisted in settings.json. **DONE**
5. **Search** — Ctrl+F dialog, results list, jump, yellow highlight. *~1 day*
6. **Selection** — hit-test drag, Ctrl+C copy, single-page clamp. *~1 day*
7. **Position/columns persistence** — restore last book viewport position, columns, margins across sessions (recents already persist). *~half day*
8. **Settings + theming** — Ctrl+, margins + Light/Dark/System + paper/dark page mode. *~half day*

Total remaining: roughly 3–4 days; the schedule risk (milestone 3) is behind us.

## Verification

- Milestone 2+: `cargo test` with 2–3 real EPUBs (plain prose, one with images, one with nested lists) checked into `tests/books/`.
- Milestone 3: golden-image test optional; otherwise manual check — no block overflow past margin, page number never overlaps text.
- Milestone 5–6: unit tests for search hit→page mapping and selection range clamping.
- Every milestone: `cargo clippy -- -D warnings && cargo fmt --check`.
