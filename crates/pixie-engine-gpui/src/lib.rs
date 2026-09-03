//! pixie's gpui engine: renders the kernel `Element` tree in a real
//! window and drives the reactive loop — click → kernel `Listener` →
//! `flush` → rebuild → redraw. The World lives inside the single gpui
//! root entity; everything stays on the main thread.
//!
//! Vocabulary: Text / Button / TextField / Column / Row / Grid
//! (+ its GridCell items) / Stack / ListView / ScrollView /
//! HScrollView / Image / Svg / DataTable / Modal / BarChart /
//! LineChart / ProgressBar / Spinner / Checkbox / Switch / Slider / Select /
//! RadioGroup / TabBar / Segmented.
//! TextField state (caret, selection, IME composition, focus) lives in
//! per-field `PixieInput` entities keyed by element-tree path, so it
//! survives rebuilds — positional state transfer, engine-side. Scroll
//! offsets follow the same rule in a second path-keyed map of gpui
//! `ScrollHandle`s, which also feeds the draggable scrollbar thumb
//! every scrolling viewport paints over its far edge. A Select's
//! open/closed popover flag is a third path-keyed map under the same
//! GC — transient engine state a dump never sees.
//! Box decoration (`borderRadius:` / `borderWidth:` / `borderColor:`)
//! is shared by every element that paints a box — Column, Row, Grid,
//! Button — and applied by `style_box` on the div that carries the
//! background, since a wrapper would round the fill and leave the
//! border square.
//! The charts paint themselves through a `canvas` element (quads for
//! BarChart, a stroked `PathBuilder` polyline for LineChart) inside
//! cute_ui's plot chrome; the Spinner is a stroked 120° arc rotating
//! over a background ring, driven by `request_animation_frame`.

mod text_input;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, BoxShadow, Bounds, Context, DispatchPhase, Entity, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathBuilder, Pixels, ScrollHandle, SharedString,
    TitlebarOptions, WeakEntity, Window, WindowBounds, WindowOptions, canvas, deferred, div,
    fill, hsla, point, prelude::*, px, relative, rgb, rgba, size,
};
use futures::FutureExt as _;
use gpui::Asset as _;
use pixie_kernel::{Component, Element, Handle, List, Runtime, Str, TextListener, World};

use text_input::PixieInput;

struct Root<C: Component> {
    runtime: Runtime,
    view: Handle<C>,
    tree: Element,
    /// Live text editors keyed by element-tree path. Stale paths are
    /// dropped after each render pass.
    inputs: HashMap<Vec<usize>, Entity<PixieInput>>,
    /// Live scroll viewports keyed by element-tree path — the `inputs`
    /// rule applied to scroll position, so a wheeled-down list keeps
    /// its place across a rebuild. Same pass, same GC.
    scrolls: HashMap<Vec<usize>, ScrollState>,
    /// Open/closed state of each Select's option popover, keyed by
    /// element-tree path — the `scrolls` rule applied to a bool.
    /// `Rc<Cell<..>>` because the click handlers that flip it are
    /// registered during render and must not re-enter the Root.
    /// Same pass, same GC. Headless runs never populate it: the
    /// script verb selects through the kernel finder directly.
    /// (open, control bounds x/y/w/h from its last paint) — the
    /// bounds anchor the hoisted option panel right under the
    /// control (taffy absolute resolves against the direct parent,
    /// so the overlay cannot inherit them by position).
    selects: HashMap<Vec<usize>, Rc<Cell<(bool, (f32, f32, f32, f32))>>>,
    /// True while the 16 ms async pump chain is scheduled.
    pumping: bool,
    /// The window's decoded-image cache (§8.38). Every `img()` and
    /// `svg()` under the root resolves through it, so the budget is
    /// the app's whole image footprint.
    images: Entity<PixieImageCache>,
}

/// A bounded, least-recently-used image cache.
///
/// gpui ships exactly one implementation — `RetainAllImageCache`, a
/// `HashMap` with `clear()`/`remove()` and no policy — and pixie
/// installed none, so a decoded image was retained for the life of
/// the process. Fine for a screen of icons; not fine for a media
/// library, where scrolling past five thousand covers kept five
/// thousand bitmaps resident with nothing an app could do about it.
///
/// A framework should not make an app think about this, so the budget
/// is bounded by default and `PIXIE_IMAGE_BUDGET_MB` moves it. Only
/// LOADED entries count against it: an in-flight decode has no size
/// yet, and dropping it would restart the load.
pub struct PixieImageCache {
    entries: HashMap<u64, ImageEntry>,
    /// Monotonic tick — the recency key. A `u64` at one bump per
    /// image lookup does not wrap in any real session.
    clock: u64,
    bytes: usize,
    budget: usize,
}

struct ImageEntry {
    item: gpui::ImageCacheItem,
    used: u64,
    /// Zero until the decode lands and the size is known.
    bytes: usize,
}

/// 256 MB of decoded pixels: roughly a thousand 256×256 BGRA
/// thumbnails, which is a library view's worth and still small
/// beside what a media app would otherwise retain forever.
const DEFAULT_IMAGE_BUDGET_MB: usize = 256;

impl PixieImageCache {
    fn new() -> Self {
        let budget = std::env::var("PIXIE_IMAGE_BUDGET_MB")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(DEFAULT_IMAGE_BUDGET_MB)
            .saturating_mul(1024 * 1024);
        PixieImageCache {
            entries: HashMap::new(),
            clock: 0,
            bytes: 0,
            budget,
        }
    }

    /// Decoded size, summed over every frame — an animated GIF costs
    /// what it actually costs.
    fn image_bytes(img: &gpui::RenderImage) -> usize {
        (0..img.frame_count())
            .filter_map(|f| img.as_bytes(f).map(|b| b.len()))
            .sum()
    }

    /// Drop least-recently-used LOADED entries until the budget holds.
    /// The just-touched entry is never a candidate, so a single image
    /// larger than the whole budget still renders — it simply evicts
    /// everything else, which is the honest behavior for a budget set
    /// smaller than the working set.
    fn evict(&mut self, keep: u64, window: &mut Window, cx: &mut App) {
        while self.bytes > self.budget {
            let victim = self
                .entries
                .iter()
                .filter(|(_, e)| e.used != keep && e.bytes > 0)
                .min_by_key(|(_, e)| e.used)
                .map(|(k, _)| *k);
            let Some(key) = victim else {
                return;
            };
            let Some(mut e) = self.entries.remove(&key) else {
                return;
            };
            self.bytes = self.bytes.saturating_sub(e.bytes);
            if let Some(Ok(img)) = e.item.get() {
                cx.drop_image(img, Some(window));
            }
        }
    }

    /// Bytes currently held, for tests and for a future `imageBudget:`
    /// surface to report against.
    pub fn resident_bytes(&self) -> usize {
        self.bytes
    }
}

impl gpui::ImageCache for PixieImageCache {
    fn load(
        &mut self,
        resource: &gpui::Resource,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<Result<std::sync::Arc<gpui::RenderImage>, gpui::ImageCacheError>> {
        let key = gpui::hash(resource);
        self.clock += 1;
        let now = self.clock;
        if let Some(e) = self.entries.get_mut(&key) {
            e.used = now;
            let got = e.item.get();
            if e.bytes == 0 {
                if let Some(Ok(img)) = &got {
                    e.bytes = PixieImageCache::image_bytes(img);
                    self.bytes += e.bytes;
                    self.evict(now, window, cx);
                }
            }
            return got;
        }
        // Miss: start the decode on the background pool and park the
        // task, exactly as `RetainAllImageCache` does. The element is
        // notified on the next frame so the load lands in a paint.
        let fut = gpui::AssetLogger::<gpui::ImageAssetLoader>::load(resource.clone(), cx);
        let task = cx.background_executor().spawn(fut).shared();
        self.entries.insert(
            key,
            ImageEntry {
                item: gpui::ImageCacheItem::Loading(task.clone()),
                used: now,
                bytes: 0,
            },
        );
        let entity = window.current_view();
        window
            .spawn(cx, {
                async move |cx| {
                    let _ = task.await;
                    cx.on_next_frame(move |_, cx| {
                        cx.notify(entity);
                    });
                }
            })
            .detach();
        None
    }
}

/// Everything one scrolling viewport (ScrollView / HScrollView /
/// virtualized ListView) keeps between frames.
///
/// `handle` is gpui's: the scrolling div writes the wheel offset into
/// it and layout fills in the viewport bounds and maximum offset, which
/// is exactly the geometry cute_ui's `scrollbarThumbRect` derives from
/// `frame_` and `content_extent_`. `drag`/`hover` are the thumb's own
/// transient state; they are `Rc` cells rather than fields read through
/// the Root entity because the mouse handlers that touch them are
/// registered during paint and must not re-enter the render that
/// created them.
#[derive(Clone, Default)]
struct ScrollState {
    handle: ScrollHandle,
    /// `Some((pointer, offset))` sampled at mouse-down while the thumb
    /// is held — cute_ui's `drag_start_pos_` / `drag_start_scroll_`.
    drag: Rc<Cell<Option<(f32, f32)>>>,
    /// Pointer inside the thumb's (inflated) hit rect.
    hover: Rc<Cell<bool>>,
}

/// Scrollbar metrics. cute_ui paints a 4 px thumb inset 2 px with a
/// 20 px floor and a 3 px hit inflation; pixie's dark theme wants a
/// slightly chunkier 6 px thumb and a 24 px floor.
const THUMB_W: f32 = 6.0;
const THUMB_MIN: f32 = 24.0;
const THUMB_INSET: f32 = 2.0;
const THUMB_SLOP: f32 = 3.0;

/// Slider metrics: a 4 px track and a 14 px round thumb inside a
/// 20 px-tall, parent-wide box.
const SLIDER_TRACK: f32 = 4.0;
const SLIDER_THUMB: f32 = 14.0;
const SLIDER_H: f32 = 20.0;

/// The clip height of a scrolling viewport: the `height:` prop when the
/// view set one, else the 320 px the engine used before the prop
/// existed (kernel `0.0` = unset).
/// The palette of a tree whose ROOT is a `theme:` scope (peeking
/// through the non-visual Anim/Semantics wrappers). None = the tree
/// follows the engine mirror.
fn root_scope_theme(mut el: &Element) -> Option<&'static Theme> {
    loop {
        match el {
            Element::Themed { theme, .. } => {
                return pixie_kernel::theme::by_name(theme.as_str());
            }
            Element::Anim { children, .. } | Element::Semantics { children, .. } => {
                el = children.first()?;
            }
            _ => return None,
        }
    }
}

fn viewport_h(height: f64) -> f32 {
    if height > 0.0 { height as f32 } else { 320.0 }
}

/// The palette lives in `pixie_kernel::theme` now (§8.37): color
/// tokens resolve INSIDE the element tree, scoped by `theme:`, so the
/// table has to be somewhere both tiers can see. Re-exported here
/// because it was this crate's public surface first.
pub use pixie_kernel::theme::{DARK as THEME_DARK, LIGHT as THEME_LIGHT, Theme};

/// The root theme as a process-local mirror. `PIXIE_THEME` and Cmd+T
/// write it; `Root::render` syncs it into the World, which is what
/// the tree actually resolves against. It stays a global because the
/// text-input entities and canvas painters read colors outside any
/// Root borrow — but only the ROOT palette rides here. Everything
/// inside the tree goes through `render_el`'s scoped `th`.
static THEME_LIGHT_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn theme() -> &'static Theme {
    if THEME_LIGHT_ON.load(std::sync::atomic::Ordering::Relaxed) {
        &THEME_LIGHT
    } else {
        &THEME_DARK
    }
}

fn set_theme_light(on: bool) {
    THEME_LIGHT_ON.store(on, std::sync::atomic::Ordering::Relaxed);
}

gpui::actions!(pixie_app, [ToggleTheme]);

/// Parse a pixie color string through gpui's hex parser
/// (`#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`), or resolve a semantic
/// theme-token name so views and styles can say `color: "accent"`
/// and follow the theme. Empty = unset; an invalid string is also
/// `None` so the caller falls back to the theme default — cute's
/// QColor contract: a bad color degrades, it never aborts a frame.
fn parse_color(s: &Str) -> Option<gpui::Rgba> {
    let t = s.as_str();
    if t.is_empty() {
        return None;
    }
    // Token NAMES no longer reach here: `pixie_kernel::theme::resolve`
    // rewrites them to hex on every rebuild, scoped by `theme:`. The
    // root-theme fallback stays for anything the kernel pass does not
    // walk (engine-internal callers), and for a name it does not know
    // the answer is still `None` — a bad color degrades, it never
    // aborts a frame (cute's QColor contract).
    if let Some(v) = theme().token(t) {
        return Some(if matches!(t, "textDim" | "selection" | "scrim") {
            rgba(v)
        } else {
            rgb(v)
        });
    }
    gpui::Rgba::try_from(t).ok()
}

/// 15 %-toward-white is the derived hover tint for custom Button
/// backgrounds (the fixed 0x45475a → 0x585b70 theme pair's ratio).
fn lighten(c: gpui::Rgba, f: f32) -> gpui::Rgba {
    gpui::Rgba {
        r: c.r + (1.0 - c.r) * f,
        g: c.g + (1.0 - c.g) * f,
        b: c.b + (1.0 - c.b) * f,
        a: c.a,
    }
}

/// The container style triple every flex container applies the same
/// way: `spacing` `-1.0` = unset (the historical `gap_2()` — 8 px),
/// `padding` `0.0` = none, empty `background` = transparent.
fn style_container(d: gpui::Div, spacing: f64, padding: f64, background: &Str) -> gpui::Div {
    let mut d = if spacing >= 0.0 {
        d.gap(px(spacing as f32))
    } else {
        d.gap_2()
    };
    if padding > 0.0 {
        d = d.p(px(padding as f32));
    }
    if let Some(c) = parse_color(background) {
        d = d.bg(c);
    }
    d
}

/// Box decoration, shared by every element that paints a box (§8.79):
/// a corner radius, a border thickness and the border's color. The
/// radius has to sit on the div that carries the background — a
/// wrapper would round the fill and leave the border square — so this
/// is a styling helper rather than a wrapper element.
///
/// `md_default` keeps an element's existing look when the author sets
/// nothing: a Button rounded itself before the prop existed.
fn style_box<E: gpui::Styled>(
    mut e: E,
    radius: f64,
    width: f64,
    color: &Str,
    th: &Theme,
    md_default: bool,
) -> E {
    if radius > 0.0 {
        e = e.rounded(px(radius as f32));
    } else if md_default {
        e = e.rounded_md();
    }
    if width > 0.0 {
        e = e
            .border(px(width as f32))
            .border_color(parse_color(color).unwrap_or(rgb(th.border)));
    }
    e
}

/// Create-or-reuse the scroll state for the viewport at `pass.path` and
/// mark that path live for this pass's GC — `Element::TextField`'s
/// editor lookup, one map over.
fn scroll_state(
    scrolls: &mut HashMap<Vec<usize>, ScrollState>,
    pass: &mut RenderPass,
) -> ScrollState {
    let key = pass.path.clone();
    pass.seen.push(key.clone());
    scrolls.entry(key).or_default().clone()
}

/// cute_ui's `paintScrollbar` / `scrollbarThumbRect` /
/// `hitScrollbarThumb` / `begin|updateScrollbarDrag`, ported onto a
/// gpui `ScrollHandle` and returned as an overlay to hang beside the
/// scrolling div.
///
/// Two structural notes, both learned from gpui rather than guessed:
///
/// * The overlay is a sibling of the scroller, never a child of it —
///   a div's scroll offset is applied to *every* child, absolute ones
///   included (`with_element_offset` in `Div::prepaint`), so a thumb
///   inside the viewport would scroll away with the content.
/// * The thumb is painted by a `canvas`, not positioned as a div,
///   because the numbers it needs are current-frame only inside paint:
///   `ScrollHandle::max_offset`/`bounds` are filled in by the
///   scroller's *prepaint*, which runs after the whole tree's layout
///   has already resolved any `.top(px(..))` we could have computed at
///   render time. A positioned div would trail by a frame and, on the
///   very first frame — when the handle still reads all zeros — would
///   be missing entirely. Painting also puts a `&mut Window` in reach,
///   so the drag listeners can be window-wide `on_mouse_event`s: a
///   `div().on_mouse_move` only fires while hovered, and a scrollbar
///   drag has to survive the pointer leaving a 6 px strip.
fn scrollbar(st: &ScrollState, horizontal: bool) -> gpui::AnyElement {
    let handle = st.handle.clone();
    let drag = st.drag.clone();
    let hover = st.hover.clone();
    div()
        .absolute()
        .inset_0()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, _: &mut App| {
                    let max_off = handle.max_offset();
                    let max = if horizontal { max_off.x } else { max_off.y }.as_f32();
                    if std::env::var("PIXIE_TRACE_SCROLL").is_ok() {
                        eprintln!(
                            "pixie scroll: horizontal={horizontal} box={:?} max={max_off:?} off={:?}",
                            bounds.size,
                            handle.offset()
                        );
                    }
                    // Nothing to scroll: no thumb, and any drag that
                    // was live when the content shrank is dropped
                    // (cute_ui's `paintScrollbar` early-out).
                    if max <= 0.0 {
                        if drag.take().is_some() {
                            window.refresh();
                        }
                        return;
                    }
                    // gpui's offset counts *down* from zero as content
                    // scrolls up; cute_ui's `scroll_pos_` counts up.
                    let raw = handle.offset();
                    let raw = if horizontal { raw.x } else { raw.y }.as_f32();
                    let off = (-raw).clamp(0.0, max);
                    let (x0, y0) = (bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
                    let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    let viewport = if horizontal { w } else { h };
                    let track = (viewport - 2.0 * THUMB_INSET).max(1.0);
                    // Thumb length in proportion to the visible slice of
                    // the content, floored so a 100k-row list still
                    // leaves something to grab.
                    let len = (track * (viewport / (viewport + max)))
                        .max(THUMB_MIN)
                        .min(track);
                    let pos = (track - len) * (off / max);
                    let thumb = if horizontal {
                        Bounds::new(
                            point(
                                px(x0 + THUMB_INSET + pos),
                                px(y0 + h - THUMB_INSET - THUMB_W),
                            ),
                            size(px(len), px(THUMB_W)),
                        )
                    } else {
                        Bounds::new(
                            point(
                                px(x0 + w - THUMB_INSET - THUMB_W),
                                px(y0 + THUMB_INSET + pos),
                            ),
                            size(px(THUMB_W), px(len)),
                        )
                    };
                    let held = drag.get().is_some();
                    let tint = if held || hover.get() {
                        rgb(theme().scrollbar_active)
                    } else {
                        rgb(theme().scrollbar)
                    };
                    // The track itself stays transparent — only the
                    // thumb paints, over whatever the viewport clipped.
                    window.paint_quad(fill(thumb, tint).corner_radii(px(THUMB_W / 2.0)));

                    // A precise click on a 6 px strip is unkind, so the
                    // hit rect is the thumb inflated on every side.
                    let hit = Bounds::new(
                        thumb.origin - point(px(THUMB_SLOP), px(THUMB_SLOP)),
                        size(
                            thumb.size.width + px(2.0 * THUMB_SLOP),
                            thumb.size.height + px(2.0 * THUMB_SLOP),
                        ),
                    );
                    // Dragging the thumb across the track's free travel
                    // covers the whole 0..max range — cute_ui's
                    // `updateScrollbarDrag` gain.
                    let gain = max / (track - len).max(1.0);

                    // Listeners are re-registered every paint, so each
                    // frame's closures close over that frame's geometry.
                    {
                        let drag = drag.clone();
                        window.on_mouse_event(
                            move |ev: &MouseDownEvent, phase, window, cx: &mut App| {
                                if phase != DispatchPhase::Bubble
                                    || ev.button != MouseButton::Left
                                    || !hit.contains(&ev.position)
                                {
                                    return;
                                }
                                let at = if horizontal { ev.position.x } else { ev.position.y };
                                drag.set(Some((at.as_f32(), off)));
                                // The overlay paints last, so in the
                                // bubble phase this listener runs before
                                // anything underneath it: swallow the
                                // press so a button beneath the thumb
                                // does not see the start of a drag.
                                cx.stop_propagation();
                                window.refresh();
                            },
                        );
                    }
                    {
                        let (drag, hover, handle) =
                            (drag.clone(), hover.clone(), handle.clone());
                        window.on_mouse_event(
                            move |ev: &MouseMoveEvent, phase, window, _: &mut App| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }
                                let over = hit.contains(&ev.position);
                                if hover.replace(over) != over {
                                    window.refresh();
                                }
                                let Some((from, start)) = drag.get() else {
                                    return;
                                };
                                // A button released outside the window
                                // never sends us a MouseUp; a move with
                                // the button up ends the drag anyway.
                                if ev.pressed_button != Some(MouseButton::Left) {
                                    drag.set(None);
                                    window.refresh();
                                    return;
                                }
                                let at = if horizontal { ev.position.x } else { ev.position.y };
                                let next =
                                    (start + (at.as_f32() - from) * gain).clamp(0.0, max);
                                let cur = handle.offset();
                                handle.set_offset(if horizontal {
                                    point(px(-next), cur.y)
                                } else {
                                    point(cur.x, px(-next))
                                });
                                window.refresh();
                            },
                        );
                    }
                    {
                        let drag = drag.clone();
                        window.on_mouse_event(
                            move |_: &MouseUpEvent, phase, window, _: &mut App| {
                                if phase == DispatchPhase::Bubble && drag.take().is_some() {
                                    window.refresh();
                                }
                            },
                        );
                    }
                },
            )
            .size_full(),
        )
        .into_any_element()
}

impl<C: Component> Root<C> {
    /// Run a World mutation through the reactive loop: mutate, flush,
    /// rebuild if any view dirtied, repaint — and keep the async pump
    /// alive while spawned tasks are pending.
    fn apply(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut World)) {
        let view = self.view;
        let tree = self.runtime.with(|w| {
            pixie_kernel::contain("handler", || f(w));
            w.flush();
            if w.take_dirty_views().is_empty() {
                None
            } else {
                Some(pixie_kernel::build_prepared(w, view))
            }
        });
        if let Some(t) = tree {
            self.tree = t;
        }
        cx.notify();
        self.ensure_pump(cx);
    }

    /// One async-executor turn plus the reactive tail.
    fn pump_tick(&mut self, cx: &mut Context<Self>) {
        self.runtime.turn();
        let view = self.view;
        let tree = self.runtime.with(|w| {
            w.flush();
            if w.take_dirty_views().is_empty() {
                None
            } else {
                Some(pixie_kernel::build_prepared(w, view))
            }
        });
        if let Some(t) = tree {
            self.tree = t;
            cx.notify();
        }
    }

    /// Schedule the 16 ms pump chain while tasks are live. Completions
    /// arrive from worker threads between turns; production replaces
    /// this cadence with real wakers into the platform loop.
    fn ensure_pump(&mut self, cx: &mut Context<Self>) {
        if self.pumping || !self.runtime.has_tasks() {
            return;
        }
        self.pumping = true;
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(16))
                    .await;
                let live = this.update(cx, |this, cx| {
                    this.pump_tick(cx);
                    this.runtime.has_tasks()
                });
                match live {
                    Ok(true) => continue,
                    _ => break,
                }
            }
            let _ = this.update(cx, |this, _| this.pumping = false);
        })
        .detach();
    }
}

/// Per-render bookkeeping for the element walk.
struct RenderPass {
    next_id: usize,
    path: Vec<usize>,
    /// Paths of the stateful widgets present in this pass — TextField
    /// editors and scrolling viewports both (map GC).
    seen: Vec<Vec<usize>>,
    /// TextFields in document order (tab ring).
    order: Vec<Entity<PixieInput>>,
    /// Open Modals, hoisted out of the walk. taffy resolves an
    /// `absolute` child against its DIRECT parent (every gpui div is
    /// `position: relative` by default, so there is no positioned-
    /// ancestor search to exploit): an overlay left in place would dim
    /// only its enclosing Column. Root::render re-parents them onto the
    /// padding-free outer frame, where `inset_0` means the window.
    overlays: Vec<gpui::AnyElement>,
    /// Path prefixes owned by lazily-built ListViews this pass. The
    /// input-editor GC must not collect a TextField that lives inside
    /// a lazy row: those paths are only walked when the row range is
    /// built, never in the eager pass.
    lazy_prefixes: Vec<Vec<usize>>,
}

impl<C: Component> Render for Root<C> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Handler-queued OS notifications drain here — the one place
        // every windowed run passes with a platform handle in reach.
        // (An unbundled binary logs-and-drops inside the platform.)
        for (title, body) in pixie_kernel::notify::drain() {
            cx.show_system_notification(gpui::SystemNotification {
                tag: title.clone().into(),
                title: title.into(),
                body: body.into(),
                actions: Vec::new(),
            });
        }
        // Tasks spawned BEFORE the window existed (an embedder can
        // `Runtime::spawn` ahead of `run_app` — pixie-py's timers do)
        // have no `apply` to start the pump for them; arm it here.
        // Guarded by `pumping`/`has_tasks`, so per-frame it is a no-op.
        self.ensure_pump(cx);
        // §8.35: time is an INPUT to the tree. Advance the animation
        // clock to the wall clock and rebuild before painting, then
        // ask for the next frame while anything is still moving. The
        // whole-view rebuild is the model the Spinner already
        // established — `request_animation_frame` notifies the root.
        let view = self.view;
        let reduced = cx.reduce_motion();
        let now = ANIM_CLOCK.elapsed().as_secs_f64() * 1000.0;
        // §8.37: the World remembers which palette the current tree
        // was resolved with, so a Cmd+T flip is detected by comparing
        // it against the mirror rather than by tracking a flag here.
        // The rebuild is what makes the flip visible — token colors
        // live in the tree now, not in a global read at paint time.
        let want_light = THEME_LIGHT_ON.load(std::sync::atomic::Ordering::Relaxed);
        let th: &'static Theme = if want_light {
            &THEME_LIGHT
        } else {
            &THEME_DARK
        };
        // An app whose WHOLE tree is theme-scoped (`theme:` on the
        // root) owns the window ground too: the engine root's bg/text
        // and its padding ring follow the declared palette, not the
        // mirror — otherwise a scoped light app sits in a dark frame
        // (caught in the live window; invisible to dumps).
        let th = root_scope_theme(&self.tree).unwrap_or(th);
        let (rebuilt, animating) = self.runtime.with(|w| {
            let flipped = pixie_kernel::theme::is_light(w) != want_light;
            if flipped {
                pixie_kernel::theme::set_light(w, want_light);
            }
            pixie_kernel::anim::set_reduced(w, reduced);
            pixie_kernel::anim::set_now(w, now);
            let animating = pixie_kernel::anim::active(w);
            let tree = (flipped || animating).then(|| pixie_kernel::build_prepared(w, view));
            (tree, animating)
        });
        if let Some(t) = rebuilt {
            self.tree = t;
        }
        if animating {
            window.request_animation_frame();
        }
        let mut pass = RenderPass {
            next_id: 0,
            path: Vec::new(),
            seen: Vec::new(),
            order: Vec::new(),
            overlays: Vec::new(),
            lazy_prefixes: Vec::new(),
        };
        let body = render_el(
            &self.tree,
            &mut pass,
            &mut self.inputs,
            &mut self.scrolls,
            &mut self.selects,
            Slot::Flow,
            Sem::default(),
            th,
            cx,
        );
        // One liveness rule for both path-keyed maps: a key survives if
        // this pass walked it, or if it sits under a lazily-built
        // ListView whose rows were never walked eagerly.
        let live = |k: &Vec<usize>| {
            pass.seen.contains(k) || pass.lazy_prefixes.iter().any(|p| k.starts_with(p))
        };
        self.inputs.retain(|k, _| live(k));
        self.scrolls.retain(|k, _| live(k));
        self.selects.retain(|k, _| live(k));
        // Wire Tab / Shift-Tab as a ring over document order.
        let n = pass.order.len();
        for i in 0..n {
            let (next, prev) = if n > 1 {
                (
                    Some(pass.order[(i + 1) % n].read(cx).focus_handle.clone()),
                    Some(pass.order[(i + n - 1) % n].read(cx).focus_handle.clone()),
                )
            } else {
                (None, None)
            };
            pass.order[i].update(cx, |inp, _| {
                inp.next_focus = next;
                inp.prev_focus = prev;
            });
        }
        // The outer frame carries no padding so a Modal overlay dims
        // the window edge to edge; the body's padding and column
        // rhythm move one level in. Overlays come last, so they paint
        // above the body even before `deferred` has its say.
        // §8.38: every `img()` and `svg()` resolves its cache from the
        // nearest ancestor that declares one, so the whole frame sits
        // inside the cache scope — that is what makes the bounded
        // cache the DEFAULT rather than something an app opts into.
        // It carries LAYOUT only: `ImageCacheElement` refines a style
        // for sizing but paints nothing of its own, so the background
        // stays on a real div inside it.
        gpui::image_cache(self.images.clone())
            .relative()
            .flex()
            .size_full()
            .child(
                div()
                    .relative()
                    .flex()
                    .size_full()
                    .bg(rgb(th.window_bg))
                    .text_color(rgb(th.text))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .p_4()
                            .size_full()
                            .child(body),
                    )
                    .children(pass.overlays),
            )
    }
}

/// Route a committed edit (or submit) back into the World through the
/// root entity, then run the reactive loop.
fn make_text_cb<C: Component>(
    root: WeakEntity<Root<C>>,
    f: TextListener,
) -> text_input::TextCallback {
    Rc::new(move |text: &str, cx: &mut App| {
        let s = Str::from(text);
        let f = f.clone();
        let _ = root.update(cx, move |root, cx| {
            root.apply(cx, move |w| f(w, s));
        });
    })
}

/// How the parent places this element. A `Row` lets a TextField share
/// the line instead of claiming the full width; a `Grid` cell asks an
/// item to fill the track it landed in — CSS grid stretches items by
/// default, but a Button's own div has to opt into that (it would
/// otherwise hug its label and leave the cell half-painted).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Slot {
    Flow,
    Row,
    Grid,
}

/// The authored accessibility overrides flowing into ONE element from
/// an enclosing `Element::Semantics` (§8.36). The wrapper is layout
/// transparent — it hands its role and label to the child's own div
/// rather than adding a box — so this is how they get there.
#[derive(Clone, Copy, Default)]
struct Sem<'a> {
    role: Option<&'a Str>,
    label: Option<&'a Str>,
}

fn accesskit_role(r: pixie_kernel::a11y::Role) -> gpui::Role {
    use pixie_kernel::a11y::Role as R;
    match r {
        R::Button => gpui::Role::Button,
        R::Label => gpui::Role::Label,
        R::Heading => gpui::Role::Heading,
        R::TextInput => gpui::Role::TextInput,
        R::Image => gpui::Role::Image,
        R::List => gpui::Role::List,
        R::ListItem => gpui::Role::ListItem,
        R::Table => gpui::Role::Table,
        R::Dialog => gpui::Role::Dialog,
        R::Progress => gpui::Role::ProgressIndicator,
        R::Slider => gpui::Role::Slider,
        R::Group => gpui::Role::Group,
        R::CheckBox => gpui::Role::CheckBox,
        R::Switch => gpui::Role::Switch,
        R::ComboBox => gpui::Role::ComboBox,
        R::RadioGroup => gpui::Role::RadioGroup,
        R::TabList => gpui::Role::TabList,
    }
}

/// Tell assistive technology what this element is. The role and name
/// come from `pixie_kernel::a11y` — the same derivation the headless
/// `a11y` dump prints, so what a screen reader hears and what the
/// tier gate checks cannot drift apart.
fn with_a11y<E: gpui::StatefulInteractiveElement>(d: E, el: &Element, sem: Sem<'_>) -> E {
    let role = sem
        .role
        .and_then(|r| pixie_kernel::a11y::Role::parse(r.as_str()))
        .or_else(|| pixie_kernel::a11y::role_of(el));
    let Some(role) = role else {
        return d;
    };
    let mut d = d.role(accesskit_role(role));
    let name = match sem.label {
        Some(l) if !l.as_str().is_empty() => l.clone(),
        _ => pixie_kernel::a11y::name_of(el),
    };
    if !name.as_str().is_empty() {
        d = d.aria_label(SharedString::from(name.as_str().to_string()));
    }
    let value = pixie_kernel::a11y::value_of(el);
    if !value.as_str().is_empty() {
        d = d.aria_value(SharedString::from(value.as_str().to_string()));
    }
    d
}

/// The flex participation a fading wrapper has to stand in for. Only
/// the elements that HAVE `grow:` answer; everything else hugs its
/// content either way.
fn child_flex(el: &Element) -> (f64, f64) {
    match el {
        Element::Button { grow, basis, .. } => (*grow, *basis),
        Element::Column { grow, .. } | Element::Row { grow, .. } | Element::Grid { grow, .. } => {
            (*grow, 0.0)
        }
        Element::Text { grow, .. } => (*grow, 0.0),
        _ => (0.0, 0.0),
    }
}

fn render_el<C: Component>(
    el: &Element,
    pass: &mut RenderPass,
    inputs: &mut HashMap<Vec<usize>, Entity<PixieInput>>,
    scrolls: &mut HashMap<Vec<usize>, ScrollState>,
    // A Select's open-popover flag, keyed by element path like
    // `scrolls` and GC'd by the same pass rule.
    selects: &mut HashMap<Vec<usize>, Rc<Cell<(bool, (f32, f32, f32, f32))>>>,
    slot: Slot,
    // Authored accessibility overrides from an enclosing
    // `Element::Semantics`; `Sem::default()` everywhere else.
    sem: Sem<'_>,
    // The palette in force HERE (§8.37). Element color props arrive
    // already resolved to hex by the kernel pass; this is for the
    // chrome the engine paints itself — borders, scrollbars, field
    // surfaces — which has to follow the same scope.
    th: &'static Theme,
    cx: &mut Context<Root<C>>,
) -> gpui::AnyElement {
    match el {
        Element::Text {
            text,
            font_size,
            color,
            align,
            grow,
        } => {
            pass.next_id += 1;
            let mut d = with_a11y(div().id(pass.next_id), el, sem);
            if *font_size > 0.0 {
                d = d.text_size(px(*font_size as f32));
            }
            // Always SET the color, never inherit it: the enclosing
            // div's text color comes from the root palette, which a
            // `theme:` scope does not reach (§8.37). Inheriting it
            // put dark-theme labels on a light-theme button.
            d = d.text_color(parse_color(color).unwrap_or(rgb(th.text)));
            // Alignment needs the div to own its box: flex + full
            // width, content pushed by justify; a grown text absorbs
            // spare main-axis space (its content bottom-anchored, the
            // calculator-readout shape).
            match align.as_str() {
                "right" => d = d.w_full().flex().justify_end(),
                "center" => d = d.w_full().flex().justify_center(),
                _ => {}
            }
            if *grow > 0.0 {
                d = d.flex_grow(*grow as f32).items_end();
            }
            d.child(SharedString::from(text.as_str().to_string()))
                .into_any_element()
        }
        Element::Button {
            label,
            background,
            hover_background,
            active_background,
            width,
            height,
            font_size,
            color,
            grow,
            basis,
            border_radius,
            border_width,
            border_color,
            on_click,
        } => {
            let f = on_click.clone();
            pass.next_id += 1;
            let id = pass.next_id;
            // Precedence per state: explicit prop > derived-from-custom
            // background (15 % toward white for hover, 25 % for press
            // — the fixed 0x45475a → 0x585b70 pair's ratio) > theme.
            let rest = parse_color(background).unwrap_or(rgb(th.surface));
            let hover = parse_color(hover_background).unwrap_or_else(|| {
                match parse_color(background) {
                    Some(c) => lighten(c, 0.15),
                    None => rgb(th.surface_hover),
                }
            });
            let active = parse_color(active_background).unwrap_or_else(|| {
                match parse_color(background) {
                    Some(c) => lighten(c, 0.25),
                    None => rgb(th.surface_pressed),
                }
            });
            let mut d = with_a11y(div().id(id), el, sem).px_3().py_1();
            // A grid item with no sizing props of its own fills the
            // track it was placed in: the grid already decided how big
            // this key is, so the button paints the whole cell.
            let fills_cell = slot == Slot::Grid && *grow == 0.0 && *width == 0.0 && *height == 0.0;
            if fills_cell {
                d = d.size_full();
            }
            // Sizing: `grow` shares the parent's main axis (basis in
            // px anchors gapped-column spans; `width` is ignored
            // then); otherwise fixed px (0 = hug the label). Sized or
            // grown buttons center their label so keypad grids read
            // as keys, not chips.
            if *grow > 0.0 {
                d = d
                    .flex_grow(*grow as f32)
                    .flex_basis(px(*basis as f32))
                    .flex_shrink_0();
            } else if *width > 0.0 {
                d = d.w(px(*width as f32));
            }
            if *height > 0.0 {
                d = d.h(px(*height as f32));
            } else if *grow > 0.0 {
                // A grown key with no fixed height fills its row —
                // rows that grow vertically scale the whole keypad.
                d = d.h_full();
            }
            if fills_cell || *grow > 0.0 || *width > 0.0 || *height > 0.0 {
                d = d.flex().items_center().justify_center();
            }
            if *font_size > 0.0 {
                d = d.text_size(px(*font_size as f32));
            }
            // Same rule as Text: the scoped palette decides the label
            // color, rather than whatever the root div inherited down.
            d = d.text_color(parse_color(color).unwrap_or(rgb(th.text)));
            let d = style_box(d, *border_radius, *border_width, border_color, th, true);
            d.bg(rest)
                .hover(move |s| s.bg(hover))
                .active(move |s| s.bg(active))
                .cursor_pointer()
                .child(SharedString::from(label.as_str().to_string()))
                .on_click(cx.listener(move |this: &mut Root<C>, _ev, _window, cx| {
                    let f = f.clone();
                    this.apply(cx, move |w| f(w));
                }))
                .into_any_element()
        }
        Element::TextField {
            value,
            placeholder,
            on_change,
            on_submit,
        } => {
            let key = pass.path.clone();
            let entity = match inputs.get(&key) {
                Some(e) => e.clone(),
                None => {
                    let e = cx.new(|cx| {
                        PixieInput::new(cx, value.as_str(), placeholder.as_str())
                    });
                    inputs.insert(key.clone(), e.clone());
                    e
                }
            };
            pass.seen.push(key);
            pass.order.push(entity.clone());
            let root = cx.entity().downgrade();
            let commit_cb = on_change
                .clone()
                .map(|f| make_text_cb(root.clone(), f));
            let submit_cb = on_submit.clone().map(|f| make_text_cb(root, f));
            let (v, p) = (
                value.as_str().to_string(),
                placeholder.as_str().to_string(),
            );
            entity.update(cx, |inp, cx| {
                inp.on_commit = commit_cb;
                inp.on_submit = submit_cb;
                inp.sync(&v, &p, cx);
            });
            pass.next_id += 1;
            let wrap = if slot == Slot::Row {
                div().flex_1().min_w(px(160.))
            } else {
                div().w_full()
            };
            // The editor entity paints the text; the wrapper is what
            // assistive technology sees, so the role and the current
            // value ride here.
            with_a11y(wrap.id(pass.next_id), el, sem)
                .child(entity)
                .into_any_element()
        }
        Element::Column {
            spacing,
            padding,
            background,
            grow,
            border_radius,
            border_width,
            border_color,
            children,
        } => {
            let mut d = style_container(div().flex().flex_col(), *spacing, *padding, background);
            d = style_box(d, *border_radius, *border_width, border_color, th, false);
            if *grow > 0.0 {
                d = d.flex_grow(*grow as f32);
            }
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                d = d.child(render_el(c, pass, inputs, scrolls, selects, Slot::Flow, Sem::default(), th, cx));
                pass.path.pop();
            }
            d.into_any_element()
        }
        Element::Row {
            spacing,
            padding,
            background,
            grow,
            border_radius,
            border_width,
            border_color,
            children,
        } => {
            let mut d = style_container(
                div().flex().flex_row().items_center(),
                *spacing,
                *padding,
                background,
            );
            d = style_box(d, *border_radius, *border_width, border_color, th, false);
            if *grow > 0.0 {
                d = d.flex_grow(*grow as f32);
            }
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                d = d.child(render_el(c, pass, inputs, scrolls, selects, Slot::Row, Sem::default(), th, cx));
                pass.path.pop();
            }
            d.into_any_element()
        }
        Element::Grid {
            columns,
            rows,
            spacing,
            padding,
            background,
            grow,
            border_radius,
            border_width,
            border_color,
            children,
        } => {
            // `grid_cols(n)` is gpui's whole track vocabulary:
            // `repeat(n, minmax(0, 1fr))`, equal columns by
            // construction. The gap `style_container` sets applies to
            // both axes here, unlike a flex row.
            let cols = (*columns).clamp(1, u16::MAX as i64) as u16;
            let mut d = style_container(
                div().grid().grid_cols(cols),
                *spacing,
                *padding,
                background,
            );
            d = style_box(d, *border_radius, *border_width, border_color, th, false);
            // Without explicit row tracks taffy sizes implicit rows to
            // their content, so a grown grid keeps its slack at the
            // bottom; `rows:` divides the height the way `columns:`
            // divides the width.
            if *rows > 0 {
                d = d.grid_rows((*rows).clamp(1, u16::MAX as i64) as u16);
            }
            if *grow > 0.0 {
                d = d.flex_grow(*grow as f32);
            }
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                d = d.child(render_el(c, pass, inputs, scrolls, selects, Slot::Grid, Sem::default(), th, cx));
                pass.path.pop();
            }
            d.into_any_element()
        }
        // §8.37. A scope, not a box: nothing is painted here, the
        // subtree simply resolves its chrome against another palette.
        // The element COLOR props inside were already rewritten to hex
        // by the kernel pass, so this only redirects what the engine
        // paints on its own.
        Element::Themed { theme, children } => {
            let Some(child) = children.first() else {
                return div().into_any_element();
            };
            let th = pixie_kernel::theme::by_name(theme.as_str()).unwrap_or(th);
            pass.path.push(0);
            let rendered = render_el(child, pass, inputs, scrolls, selects, slot, sem, th, cx);
            pass.path.pop();
            rendered
        }
        // §8.36. Purely semantic: no box, no styling, nothing painted.
        // The role and label ride down to the child's own div, which
        // is what assistive technology ends up reading.
        Element::Semantics {
            role,
            label,
            children,
        } => {
            let Some(child) = children.first() else {
                return div().into_any_element();
            };
            let sem = Sem {
                role: (!role.as_str().is_empty()).then_some(role),
                label: (!label.as_str().is_empty()).then_some(label),
            };
            pass.path.push(0);
            let rendered = render_el(child, pass, inputs, scrolls, selects, slot, sem, th, cx);
            pass.path.pop();
            rendered
        }
        // §8.35. A settled wrapper renders NOTHING of its own: the
        // child goes straight out, so `animate:` on a grown element
        // costs no layout box. Only a running fade introduces a div,
        // and that div copies the child's flex participation so the
        // transient does not resize anything around it.
        Element::Anim {
            opacity, children, ..
        } => {
            let Some(child) = children.first() else {
                return div().into_any_element();
            };
            pass.path.push(0);
            let rendered = render_el(child, pass, inputs, scrolls, selects, slot, sem, th, cx);
            pass.path.pop();
            if *opacity >= 1.0 {
                return rendered;
            }
            let mut d = div().flex().opacity((*opacity).clamp(0.0, 1.0) as f32);
            let (grow, basis) = child_flex(child);
            if grow > 0.0 {
                d = d.flex_grow(grow as f32).flex_shrink_0();
                d = if basis > 0.0 {
                    d.flex_basis(px(basis as f32))
                } else {
                    d.flex_basis(px(0.))
                };
            }
            d.child(rendered).into_any_element()
        }
        Element::GridCell {
            col_span,
            row_span,
            children,
        } => {
            let mut d = div();
            if *col_span > 1 {
                d = d.col_span((*col_span).clamp(1, u16::MAX as i64) as u16);
            }
            if *row_span > 1 {
                d = d.row_span((*row_span).clamp(1, u16::MAX as i64) as u16);
            }
            // The wrapper IS the grid item, so its child still sees a
            // grid slot: one element in, the spans applied out.
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                d = d.child(render_el(c, pass, inputs, scrolls, selects, Slot::Grid, Sem::default(), th, cx));
                pass.path.pop();
            }
            d.into_any_element()
        }
        // A z-layering container: child 0 renders in flow (it sizes the
        // Stack's own box), children 1.. are each wrapped absolute +
        // inset_0 so they overlay that box edge-to-edge. taffy
        // resolves `absolute` against the DIRECT parent — here that's
        // this `.relative()` div itself, so no hoisting through
        // `pass.overlays` is needed (contrast Modal, which must escape
        // its container entirely). Later children paint above earlier
        // ones because they are later gpui children of the same div.
        // Wrappers are transparent to path-keying: the push/pop below
        // brackets only the recursive `render_el` call, so a TextField
        // inside a Stack keys on its child index exactly like it would
        // inside a Column.
        Element::Stack(children) => {
            let mut d = div().relative();
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                let rendered = render_el(c, pass, inputs, scrolls, selects, Slot::Flow, Sem::default(), th, cx);
                pass.path.pop();
                d = d.child(if i == 0 {
                    rendered
                } else {
                    div().absolute().inset_0().child(rendered).into_any_element()
                });
            }
            d.into_any_element()
        }
        // `item_height > 0` pins every row to a fixed height; the
        // wrapper div is applied after recursing, so it never disturbs
        // the child's own `pass.path`. `virtualized` is the v0
        // scaffold: a clipped scrolling viewport (the ScrollView
        // shape). Building only the visible rows needs lazy children in
        // the kernel Element — deferred, see DESIGN §11.
        Element::ListView {
            virtualized,
            item_height,
            height,
            grow,
            children,
            lazy,
        } => {
            // True virtualization: a lazy single-repeater list builds
            // only the row range gpui's uniform_list asks for, each
            // frame, against the live World (reached through the Root
            // entity — legal here because layout runs after render
            // returns the tree, so Root is not leased).
            if *virtualized && lazy.is_some() {
                let rows = lazy.clone().expect("checked");
                let ih = *item_height;
                pass.next_id += 1;
                let id = pass.next_id;
                let base_path = pass.path.clone();
                pass.lazy_prefixes.push(base_path.clone());
                let st = scroll_state(scrolls, pass);
                let root = cx.entity().downgrade();
                let list = gpui::uniform_list(
                    id,
                    rows.len,
                    move |range: std::ops::Range<usize>,
                          _window: &mut Window,
                          cx: &mut App| {
                        if std::env::var("PIXIE_TRACE_LAZY").is_ok() {
                            eprintln!("pixie lazy: building rows {range:?} of {}", rows.len);
                        }
                        let mut out: Vec<gpui::AnyElement> = Vec::new();
                        let Some(root) = root.upgrade() else {
                            return out;
                        };
                        root.update(cx, |this, cx| {
                            let built =
                                this.runtime.with(|w| (rows.build)(w, range.clone()));
                            let mut row_pass = RenderPass {
                                next_id: 0,
                                path: base_path.clone(),
                                seen: Vec::new(),
                                order: Vec::new(),
                                // Overlays (Modal) inside lazy rows have
                                // no root frame to hoist onto from here;
                                // they are dropped. Documented.
                                overlays: Vec::new(),
                                lazy_prefixes: Vec::new(),
                            };
                            for (k, el) in built.iter().enumerate() {
                                row_pass.path.push(range.start + k);
                                let e = render_el(
                                    el,
                                    &mut row_pass,
                                    &mut this.inputs,
                                    &mut this.scrolls,
                                    &mut this.selects,
                                    Slot::Flow,
                                    Sem::default(),
                                    th,
                                    cx,
                                );
                                row_pass.path.pop();
                                let mut wrap = div().w_full().flex_none();
                                if ih > 0.0 {
                                    wrap = wrap.h(px(ih as f32));
                                }
                                out.push(wrap.child(e).into_any_element());
                            }
                        });
                        out
                    },
                )
                .w_full();
                let list = if *grow > 0.0 {
                    // The outer relative div takes the flex share; the
                    // uniform_list fills it.
                    list.h_full()
                } else {
                    list.h(px(viewport_h(*height)))
                };
                // `UniformList` takes a `UniformListScrollHandle`, not
                // the plain one a div takes — but that wrapper is only
                // y-flip and scroll-to-item state (which we never
                // touch) around a `base_handle` that IS a plain
                // `ScrollHandle`, and `track_scroll` installs exactly
                // that base into the interactivity. Swapping ours in
                // first therefore hands the virtualized list the same
                // handle as the ScrollViews — same thumb, same
                // across-rebuild offset. (`UniformList` implements
                // `InteractiveElement` but not
                // `StatefulInteractiveElement`, so the div-side
                // `track_scroll` is genuinely out of reach.)
                let ulh = gpui::UniformListScrollHandle::new();
                ulh.0.borrow_mut().base_handle = st.handle.clone();
                let list = list.track_scroll(&ulh);
                let mut outer = div().relative();
                if *grow > 0.0 {
                    // min_h(0) so the flex child may shrink below its
                    // content and actually scroll.
                    outer = outer.flex_grow(*grow as f32).min_h(px(0.));
                }
                return outer
                    .child(list)
                    .child(scrollbar(&st, false))
                    .into_any_element();
            }
            let mut rows: Vec<gpui::AnyElement> = Vec::new();
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                let rendered = render_el(c, pass, inputs, scrolls, selects, Slot::Flow, Sem::default(), th, cx);
                pass.path.pop();
                rows.push(if *item_height > 0.0 {
                    // `flex_none` so the clipped viewport cannot squash
                    // the rows it is meant to scroll past.
                    div()
                        .h(px(*item_height as f32))
                        .flex_none()
                        .child(rendered)
                        .into_any_element()
                } else {
                    rendered
                });
            }
            let frame = div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .border_1()
                .border_color(rgb(th.border))
                .rounded_md();
            if *virtualized {
                // A static-children virtualized list is still just a
                // clipped scroll port (one div is both the flex column
                // and the viewport — the shape gpui's own image_gallery
                // example uses), so it gets a ScrollView's plumbing:
                // a path-keyed handle, and a thumb over the right edge.
                pass.next_id += 1;
                let id = pass.next_id;
                let st = scroll_state(scrolls, pass);
                let frame = frame.id(id).overflow_y_scroll().track_scroll(&st.handle);
                let frame = if *grow > 0.0 {
                    frame.h_full()
                } else {
                    frame.max_h(px(viewport_h(*height)))
                };
                let mut outer = div().relative();
                if *grow > 0.0 {
                    outer = outer.flex_grow(*grow as f32).min_h(px(0.));
                }
                outer
                    .child(frame.children(rows))
                    .child(scrollbar(&st, false))
                    .into_any_element()
            } else {
                let frame = if *grow > 0.0 {
                    frame.flex_grow(*grow as f32).min_h(px(0.))
                } else {
                    frame
                };
                frame.children(rows).into_any_element()
            }
        }
        // Scroll views are stateful: the offset lives in a path-keyed
        // `ScrollHandle` (so it survives a rebuild, unlike gpui's own
        // id-keyed element state) and the div still needs a
        // `pass.next_id` identity for its hitbox. Three divs deep: the
        // outermost is the positioning context the thumb overlay is
        // measured against, the middle one scrolls and clips, the inner
        // one lays the children out.
        Element::ScrollView { height, children } => {
            pass.next_id += 1;
            let id = pass.next_id;
            let st = scroll_state(scrolls, pass);
            let mut inner = div().flex().flex_col().gap_2();
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                inner = inner.child(render_el(c, pass, inputs, scrolls, selects, Slot::Flow, Sem::default(), th, cx));
                pass.path.pop();
            }
            div()
                .relative()
                .child(
                    div()
                        .id(id)
                        .overflow_y_scroll()
                        .track_scroll(&st.handle)
                        .max_h(px(viewport_h(*height)))
                        .child(inner),
                )
                .child(scrollbar(&st, false))
                .into_any_element()
        }
        // The horizontal twin, thumb along the bottom edge. No
        // `height:`: this one clips on width, and its box is as tall as
        // the tallest child either way.
        // The horizontal twin, thumb along the bottom edge. There is no
        // `height:` here — this one clips on width, and its box is as
        // tall as the tallest child either way.
        //
        // `.flex()` on the viewport and `.flex_none()` on the row are
        // load-bearing, not decoration. gpui's default `Display` is
        // `Block`, and a block-level child fills its parent's content
        // width instead of overflowing it — so the row measured exactly
        // as wide as the viewport, `max_offset.x` came out zero, and
        // there was nothing to scroll even though the text was visibly
        // clipped (the wheel silently did nothing; adding the thumb is
        // what made it obvious). As a flex item with `flex: 0 0 auto`
        // the row takes its natural width instead, which is what
        // overflows. Zed's own code-block scroller carries the same fix
        // and the same explanation. `restrict_scroll_to_axis` is
        // deliberately NOT set: a plain vertical wheel scrolling this
        // box horizontally is the documented behavior (DESIGN §8.12).
        Element::HScrollView(children) => {
            pass.next_id += 1;
            let id = pass.next_id;
            let st = scroll_state(scrolls, pass);
            let mut inner = div().flex().flex_row().flex_none().gap_2().items_center();
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                inner = inner.child(render_el(c, pass, inputs, scrolls, selects, Slot::Row, Sem::default(), th, cx));
                pass.path.pop();
            }
            div()
                .relative()
                .child(
                    div()
                        .id(id)
                        .flex()
                        .overflow_x_scroll()
                        .track_scroll(&st.handle)
                        .max_w_full()
                        .child(inner),
                )
                .child(scrollbar(&st, true))
                .into_any_element()
        }
        Element::Image {
            source,
            width,
            height,
        } => {
            // Fixed-size neutral box so a slow or missing asset doesn't
            // collapse the layout: same footprint as the declared w/h,
            // else a default 24x24 (§11.15). `with_loading` fires after
            // gpui's 200 ms LOADING_DELAY; `with_fallback` fires on a
            // decode/IO error (e.g. a missing file) — same box, dimmer
            // tone so the two states read as distinct at a glance. The
            // source resolves through the cwd → exe-dir chain first.
            let place_w = if *width > 0.0 { *width as f32 } else { 24.0 };
            let place_h = if *height > 0.0 { *height as f32 } else { 24.0 };
            pass.next_id += 1;
            let mut image = with_a11y(gpui::img(resolve_asset(source.as_str())).id(pass.next_id), el, sem)
                .with_loading(move || {
                    div()
                        .w(px(place_w))
                        .h(px(place_h))
                        .bg(rgb(th.border))
                        .rounded_md()
                        .into_any_element()
                })
                .with_fallback(move || {
                    div()
                        .w(px(place_w))
                        .h(px(place_h))
                        .bg(rgb(th.panel))
                        .rounded_md()
                        .into_any_element()
                });
            if *width > 0.0 {
                image = image.w(px(*width as f32));
            }
            if *height > 0.0 {
                image = image.h(px(*height as f32));
            }
            image.into_any_element()
        }
        Element::Svg {
            source,
            width,
            height,
        } => {
            // SVGs have no intrinsic raster size the way Image's decoded
            // bitmap does, so — unlike Image — a fully-unset Svg can't
            // fall back to "let the asset decide": it needs an explicit
            // default or it paints at 0x0. Painted as a single-color
            // mask tinted by `.text_color`, per gpui's `svg()` element.
            // Resolved here (DESIGN §11.15) before it reaches `.path`:
            // that string is also the cache key `SvgRenderer` hashes
            // the rasterized SVG under and the exact `path` argument
            // `PixieAssets::load` receives, so an unresolved relative
            // source would cache-key correctly but still read from the
            // wrong directory once it hit disk.
            let resolved_source = resolve_asset(source.as_str());
            pass.next_id += 1;
            let mut icon = with_a11y(
                gpui::svg()
                    .path(SharedString::from(
                        resolved_source.to_string_lossy().into_owned(),
                    ))
                    .id(pass.next_id),
                el,
                sem,
            )
            .text_color(rgb(th.text));
            if *width > 0.0 {
                icon = icon.w(px(*width as f32));
            }
            if *height > 0.0 {
                icon = icon.h(px(*height as f32));
            }
            if *width <= 0.0 && *height <= 0.0 {
                icon = icon.w(px(24.)).h(px(24.));
            }
            icon.into_any_element()
        }
        Element::DataTable(children) => {
            let mut d = div()
                .flex()
                .flex_col()
                .border_1()
                .border_color(rgb(th.border))
                .rounded_md()
                .overflow_hidden();
            // The first Row child is the header; later Row children are
            // data rows with an alternating stripe (zebra: odd data-row
            // index). Non-Row children render unwrapped. Counting only
            // looks at Row children — the wrapper divs are applied after
            // recursing, so they never disturb the child's own path.
            let mut header_seen = false;
            let mut data_row_ix = 0usize;
            for (i, c) in children.iter().enumerate() {
                pass.path.push(i);
                let rendered = render_el(c, pass, inputs, scrolls, selects, Slot::Flow, Sem::default(), th, cx);
                pass.path.pop();
                let wrapped = if matches!(c, Element::Row { .. }) {
                    if !header_seen {
                        header_seen = true;
                        div()
                            .bg(rgb(th.border))
                            .border_b_2()
                            .border_color(rgb(th.surface))
                            .px_2()
                            .py_1()
                            .child(rendered)
                            .into_any_element()
                    } else {
                        let stripe = data_row_ix % 2 == 1;
                        data_row_ix += 1;
                        let mut rd = div().px_2().py_1();
                        if stripe {
                            rd = rd.bg(rgb(th.panel));
                        }
                        rd.child(rendered).into_any_element()
                    }
                } else {
                    rendered
                };
                d = d.child(wrapped);
            }
            d.into_any_element()
        }
        Element::Modal { open, children } => {
            if !*open {
                // Closed: nothing paints. (Zed's own `ModalLayer`
                // returns a bare `div()` for the same reason.) `.absolute()`
                // pulls this placeholder out of the parent's flex flow so a
                // bound-closed Modal costs zero layout instead of eating a
                // flex `gap` slot (§11.14).
                return div().absolute().into_any_element();
            }
            let mut surface = div()
                .bg(rgb(th.window_bg))
                .border_1()
                .border_color(rgb(th.surface))
                .rounded_md()
                // Soft drop shadow approximating cute_ui's
                // `fillRectShadow` (blur 12 / offset (0,8)) on the
                // dialog surface (§11.14).
                .shadow(vec![
                    BoxShadow::new(px(0.), px(8.), hsla(0., 0., 0., 0.35)).blur_radius(px(12.)),
                ])
                .p_4()
                .flex()
                .flex_col()
                .gap_2()
                .min_w(px(240.));
            for (i, c) in children.iter().enumerate() {
                // The dim/surface divs are transparent to path-keying:
                // children key on their index in `children`, so a
                // TextField keeps its editor across open/close.
                pass.path.push(i);
                surface = surface.child(render_el(c, pass, inputs, scrolls, selects, Slot::Flow, Sem::default(), th, cx));
                pass.path.pop();
            }
            // `deferred` paints after every sibling, so the overlay
            // wins over anything the root frame draws later.
            // `.occlude()` swallows clicks on the dim area instead of
            // closing — cute_ui's `ModalElement::dispatchClick` rule.
            pass.overlays.push(
                deferred(
                    div()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .bg(rgba(th.scrim_rgba))
                        .flex()
                        .items_center()
                        .justify_center()
                        .occlude()
                        .child(surface),
                )
                .into_any_element(),
            );
            // In-place placeholder: the overlay itself is re-parented
            // onto the root frame by `Root::render`.
            div().into_any_element()
        }
        // The charts own their pixels: a plot box holds a `canvas`,
        // whose paint callback draws into the bounds taffy resolved for
        // it. Data is copied into the closure (f32, gpui's pixel
        // scalar) so the callback stays `'static`.
        Element::BarChart {
            data,
            labels,
            width,
            height,
        } => {
            let values: Vec<f32> = data.iter().map(|v| *v as f32).collect();
            let n = values.len();
            let plot = canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, _| {
                    let Some(max) = chart_max(&values) else {
                        return;
                    };
                    let (x0, y0) = (bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
                    let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    let slot = w / values.len() as f32;
                    // 2px between bars; a bar never collapses below 1px.
                    let bar_w = (slot - 2.0).max(1.0);
                    for (i, v) in values.iter().enumerate() {
                        let bar_h = (v / max).clamp(0.0, 1.0) * h;
                        // Baseline at the bottom: bars grow upward.
                        let origin = point(px(x0 + slot * i as f32), px(y0 + h - bar_h));
                        // cute_ui rounds bar tops by 3px; a short or
                        // narrow bar clamps so the radius never eats
                        // the quad.
                        let r = 3.0f32.min(bar_w / 2.0).min(bar_h / 2.0);
                        window.paint_quad(
                            fill(
                                Bounds::new(origin, size(px(bar_w), px(bar_h))),
                                rgb(th.accent),
                            )
                            .corner_radii(px(r)),
                        );
                    }
                },
            );
            chart_box(
                plot.size_full().into_any_element(),
                labels,
                n,
                *width,
                *height,
                LabelAnchor::Slots,
                th,
            )
        }
        Element::LineChart {
            data,
            labels,
            width,
            height,
        } => {
            let values: Vec<f32> = data.iter().map(|v| *v as f32).collect();
            let n_samples = values.len();
            let plot = canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, _| {
                    let Some(max) = chart_max(&values) else {
                        return;
                    };
                    let (x0, y0) = (bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
                    let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    let n = values.len();
                    // A lone sample sits in the middle; otherwise the
                    // samples span the full width, first to last.
                    let at = |i: usize| {
                        let x = if n == 1 {
                            x0 + w / 2.0
                        } else {
                            x0 + w * i as f32 / (n - 1) as f32
                        };
                        let y = y0 + h - (values[i] / max).clamp(0.0, 1.0) * h;
                        point(px(x), px(y))
                    };
                    if n >= 2 {
                        let mut pb = PathBuilder::stroke(px(2.));
                        pb.move_to(at(0));
                        for i in 1..n {
                            pb.line_to(at(i));
                        }
                        // A degenerate path (every sample identical, so
                        // the stroke tessellates to nothing) is dropped
                        // rather than panicking the frame.
                        if let Ok(path) = pb.build() {
                            window.paint_path(path, rgb(th.accent));
                        }
                    }
                    // Dots mark every sample — and are the whole chart
                    // when there is only one.
                    for i in 0..n {
                        let c = at(i);
                        let origin = point(c.x - px(2.5), c.y - px(2.5));
                        window.paint_quad(
                            fill(
                                Bounds::new(origin, size(px(5.), px(5.))),
                                rgb(th.accent),
                            )
                            .corner_radii(px(2.5)),
                        );
                    }
                },
            );
            chart_box(
                plot.size_full().into_any_element(),
                labels,
                n_samples,
                *width,
                *height,
                LabelAnchor::Samples,
                th,
            )
        }
        Element::ProgressBar { value } => {
            let frac = (*value).clamp(0.0, 1.0) as f32;
            pass.next_id += 1;
            with_a11y(div().id(pass.next_id), el, sem)
                .w_full()
                .h(px(8.))
                .bg(rgb(th.border))
                .rounded_full()
                .child(
                    div()
                        .h_full()
                        .w(relative(frac))
                        .bg(rgb(th.accent))
                        .rounded_full(),
                )
                .into_any_element()
        }
        // cute_ui's `SpinnerElement`: a 120° accent arc sweeping once a
        // second over a full background ring. Both are polylines
        // through `PathBuilder::stroke` (gpui has no arc primitive),
        // and the paint callback asks for the next frame itself — the
        // element carries no animation state at all.
        Element::Spinner { size } => {
            let side = if *size > 0.0 { *size as f32 } else { 24. };
            let arc = canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, cx: &mut App| {
                    let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    // cute_ui insets the ring by 2px so the stroke
                    // stays inside the box.
                    let r = w.min(h) / 2.0 - 2.0;
                    if r <= 0.0 {
                        return;
                    }
                    let (cx0, cy0) = (
                        bounds.origin.x.as_f32() + w / 2.0,
                        bounds.origin.y.as_f32() + h / 2.0,
                    );
                    // The background ring is the same polyline over a
                    // full turn.
                    if let Some(p) =
                        spinner_arc(cx0, cy0, r, 0.0, TAU, SPINNER_RING_SEGMENTS)
                    {
                        window.paint_path(p, rgb(th.border));
                    }
                    // One revolution per second off the app-wide clock.
                    // Reduced motion parks the arc at 12 o'clock and
                    // stops asking for frames, as gpui's own
                    // `request_animation_frame` docs require.
                    let still = cx.reduce_motion();
                    let phase = if still {
                        0.75
                    } else {
                        ANIM_CLOCK.elapsed().as_secs_f32() % 1.0
                    };
                    if let Some(p) = spinner_arc(
                        cx0,
                        cy0,
                        r,
                        phase * TAU,
                        SPINNER_SWEEP,
                        SPINNER_ARC_SEGMENTS,
                    ) {
                        window.paint_path(p, rgb(th.accent));
                    }
                    if !still {
                        window.request_animation_frame();
                    }
                },
            );
            div()
                .w(px(side))
                .h(px(side))
                .flex_none()
                .child(arc.size_full())
                .into_any_element()
        }
        // A labeled on/off box. The mark is BOUND state: clicking
        // reports `!checked` through `on_toggle` and paints nothing
        // itself — the box moves when the app writes the state back
        // (TextField's controlled rule). Click wiring mirrors Button's
        // `apply` path.
        Element::Checkbox {
            label,
            checked,
            on_toggle,
        } => {
            pass.next_id += 1;
            let id = pass.next_id;
            // The 16x16 mark box: bordered surface when unchecked;
            // accent fill holding a smaller window-colored mark when
            // checked (readable on the accent in both palettes).
            let mut mark = div()
                .w(px(16.))
                .h(px(16.))
                .flex_none()
                .rounded(px(4.))
                .flex()
                .items_center()
                .justify_center();
            if *checked {
                mark = mark.bg(rgb(th.accent)).child(
                    div()
                        .w(px(8.))
                        .h(px(8.))
                        .rounded(px(2.))
                        .bg(rgb(th.window_bg)),
                );
            } else {
                mark = mark
                    .border_1()
                    .border_color(rgb(th.border))
                    .bg(rgb(th.surface));
            }
            let mut d = with_a11y(div().id(id), el, sem)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(mark)
                .child(
                    div()
                        .text_color(rgb(th.text))
                        .child(SharedString::from(label.as_str().to_string())),
                );
            if let Some(f) = on_toggle.clone() {
                let next = !*checked;
                d = d.cursor_pointer().on_click(cx.listener(
                    move |this: &mut Root<C>, _ev, _window, cx| {
                        let f = f.clone();
                        this.apply(cx, move |w| f(w, next));
                    },
                ));
            }
            d.into_any_element()
        }
        // The pill-and-thumb twin: same contract, same wiring, thumb
        // parked left (2px inset) or right by justify — the eased
        // thumb slide is deferred (no `animate:` clock here yet).
        Element::Switch {
            label,
            checked,
            on_toggle,
        } => {
            pass.next_id += 1;
            let id = pass.next_id;
            let mut pill = div()
                .w(px(36.))
                .h(px(20.))
                .flex_none()
                .rounded_full()
                .flex()
                .items_center()
                .px(px(2.));
            pill = if *checked {
                pill.bg(rgb(th.accent)).justify_end()
            } else {
                pill.bg(rgb(th.surface)).justify_start()
            };
            let pill = pill.child(
                div()
                    .w(px(16.))
                    .h(px(16.))
                    .rounded_full()
                    .bg(rgb(th.window_bg)),
            );
            let mut d = with_a11y(div().id(id), el, sem)
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.))
                .child(pill)
                .child(
                    div()
                        .text_color(rgb(th.text))
                        .child(SharedString::from(label.as_str().to_string())),
                );
            if let Some(f) = on_toggle.clone() {
                let next = !*checked;
                d = d.cursor_pointer().on_click(cx.listener(
                    move |this: &mut Root<C>, _ev, _window, cx| {
                        let f = f.clone();
                        this.apply(cx, move |w| f(w, next));
                    },
                ));
            }
            d.into_any_element()
        }
       Element::Slider {
            value,
            min,
            max,
            step,
            on_change,
        } => {
            let st = scroll_state(scrolls, pass);
            let drag = st.drag.clone();
            let (value, min, max, step) = (*value, *min, *max, *step);
            let on_change = on_change.clone();
            let root = cx.entity().downgrade();
            let (track_bg, fill_bg, thumb_bg, thumb_border) = (
                rgb(th.border),
                rgb(th.accent),
                rgb(th.window_bg),
                rgb(th.border),
            );
            let paint = canvas(
                |_, _, _| (),
                move |bounds: Bounds<Pixels>, _, window: &mut Window, _: &mut App| {
                    let (x0, y0) = (bounds.origin.x.as_f32(), bounds.origin.y.as_f32());
                    let (w, h) = (bounds.size.width.as_f32(), bounds.size.height.as_f32());
                    let cy = y0 + h / 2.0;
                    // The thumb stays inside the box at both extremes,
                    // so its center travels the width minus one thumb
                    // diameter.
                    let travel = (w - SLIDER_THUMB).max(1.0);
                    let range = max - min;
                    let frac = if range > 0.0 {
                        (((value - min) / range) as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let thumb_cx = x0 + SLIDER_THUMB / 2.0 + travel * frac;
                    let track = Bounds::new(
                        point(
                            px(x0 + SLIDER_THUMB / 2.0),
                            px(cy - SLIDER_TRACK / 2.0),
                        ),
                        size(px(travel), px(SLIDER_TRACK)),
                    );
                    window.paint_quad(
                        fill(track, track_bg).corner_radii(px(SLIDER_TRACK / 2.0)),
                    );
                    let filled = Bounds::new(
                        point(
                            px(x0 + SLIDER_THUMB / 2.0),
                            px(cy - SLIDER_TRACK / 2.0),
                        ),
                        size(px(travel * frac), px(SLIDER_TRACK)),
                    );
                    window.paint_quad(
                        fill(filled, fill_bg).corner_radii(px(SLIDER_TRACK / 2.0)),
                    );
                    let thumb = Bounds::new(
                        point(
                            px(thumb_cx - SLIDER_THUMB / 2.0),
                            px(cy - SLIDER_THUMB / 2.0),
                        ),
                        size(px(SLIDER_THUMB), px(SLIDER_THUMB)),
                    );
                    window.paint_quad(
                        fill(thumb, thumb_bg)
                            .corner_radii(px(SLIDER_THUMB / 2.0))
                            .border_widths(px(1.0))
                            .border_color(thumb_border),
                    );

                    // Pointer x → snapped value. `slider_snap` is the
                    // same clamp-and-snap the `slide:` verb runs, so a
                    // click and a script land on identical values.
                    let value_at = move |at_x: f32| -> f64 {
                        let frac = ((at_x - x0 - SLIDER_THUMB / 2.0) / travel)
                            .clamp(0.0, 1.0) as f64;
                        pixie_kernel::slider_snap(min, max, step, min + frac * (max - min))
                    };
                    // Route the new value through the reactive loop —
                    // Button's `apply` wiring, reached through the weak
                    // root the way a TextField commit is.
                    let send = {
                        let root = root.clone();
                        let on_change = on_change.clone();
                        move |v: f64, cx: &mut App| {
                            if let Some(f) = on_change.clone() {
                                let _ = root.update(cx, move |root, cx| {
                                    root.apply(cx, move |w| f(w, v));
                                });
                            }
                        }
                    };

                    // Press anywhere in the box: jump to that fraction
                    // and start a drag. The `sync` rule gates the fire
                    // — only a snapped value that differs from the
                    // bound one goes out.
                    {
                        let (drag, send) = (drag.clone(), send.clone());
                        window.on_mouse_event(
                            move |ev: &MouseDownEvent, phase, window, cx: &mut App| {
                                if phase != DispatchPhase::Bubble
                                    || ev.button != MouseButton::Left
                                    || !bounds.contains(&ev.position)
                                {
                                    return;
                                }
                                let v = value_at(ev.position.x.as_f32());
                                // The drag cell doubles as the
                                // last-sent memory (an f32 slot — the
                                // move dedup compares in f32 space so
                                // one pixel never refires).
                                drag.set(Some((v as f32, 0.0)));
                                if v != value {
                                    send(v, cx);
                                }
                                cx.stop_propagation();
                                window.refresh();
                            },
                        );
                    }
                    // Dragging updates continuously, firing only when
                    // the snapped value actually changed since the
                    // last fire (the `sync` rule).
                    {
                        let (drag, send) = (drag.clone(), send.clone());
                        window.on_mouse_event(
                            move |ev: &MouseMoveEvent, phase, window, cx: &mut App| {
                                if phase != DispatchPhase::Bubble {
                                    return;
                                }
                                let Some((last, _)) = drag.get() else {
                                    return;
                                };
                                // A button released outside the window
                                // never sends a MouseUp; a move with
                                // the button up ends the drag anyway.
                                if ev.pressed_button != Some(MouseButton::Left) {
                                    drag.set(None);
                                    window.refresh();
                                    return;
                                }
                                let v = value_at(ev.position.x.as_f32());
                                if v as f32 != last {
                                    drag.set(Some((v as f32, 0.0)));
                                    send(v, cx);
                                    window.refresh();
                                }
                            },
                        );
                    }
                    {
                        let drag = drag.clone();
                        window.on_mouse_event(
                            move |_: &MouseUpEvent, phase, window, _: &mut App| {
                                if phase == DispatchPhase::Bubble && drag.take().is_some() {
                                    window.refresh();
                                }
                            },
                        );
                    }
                },
            );
            pass.next_id += 1;
            with_a11y(div().id(pass.next_id), el, sem)
                .w_full()
                .h(px(SLIDER_H))
                .cursor_pointer()
                .child(paint.size_full())
                .into_any_element()
        }
        // The closed dropdown: a bordered control showing the current
        // option, whose open/closed popover state is engine-side and
        // path-keyed (the TextField rule — it survives rebuilds and
        // never appears in a dump). The option list is hoisted through
        // `pass.overlays` like Modal: taffy resolves `absolute`
        // against the DIRECT parent, so an overlay left in place
        // would be clipped and mis-anchored by whatever container
        // holds the Select. Hoisting discards the control's layout
        // position, so the control's bounds are recorded at paint by
        // an inert canvas (the scrollbar rule: post-layout geometry
        // comes from a paint hook) into the same path-keyed cell as
        // the open flag, and the overlay anchors to them — both are
        // window-content coordinates, the overlay's parent being the
        // padding-free root frame. Clicking elsewhere does not
        // dismiss it — deferred, with the note in the ledger summary.
        Element::Select {
            options,
            selected,
            on_select,
        } => {
            let key = pass.path.clone();
            pass.seen.push(key.clone());
            let flag = selects.entry(key).or_default().clone();
            let (open, at) = flag.get();
            // Verification hook: `PIXIE_DEBUG_OPEN_SELECTS=1` renders
            // every Select open without a click, so a screenshot can
            // prove the anchoring. The first frame has no recorded
            // bounds yet (they arrive with the first paint), so it
            // asks for one more frame; that converges as soon as the
            // canvas has painted and runs only under the env var.
            let debug_open = std::env::var_os("PIXIE_DEBUG_OPEN_SELECTS").is_some();
            let open = open || debug_open;
            if debug_open && at.2 == 0.0 {
                cx.notify();
            }
            let shown = options
                .get(*selected)
                .map(|s| s.as_str().to_string())
                .unwrap_or_default();
            pass.next_id += 1;
            let control = with_a11y(div().id(pass.next_id), el, sem)
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .rounded_md()
                .border_1()
                .border_color(rgb(th.border))
                .cursor_pointer()
                .text_color(rgb(th.text))
                .child(SharedString::from(shown))
                .child(
                    div()
                        .text_color(rgba(th.text_dim_rgba))
                        .child(SharedString::from("▾")),
                )
                .on_click(cx.listener({
                    let flag = flag.clone();
                    move |_this: &mut Root<C>, _ev, _window, cx| {
                        let (o, at) = flag.get();
                        flag.set((!o, at));
                        cx.notify();
                    }
                }))
                // An inert measuring layer: its paint callback sees the
                // control's laid-out bounds and records them for the
                // overlay to anchor on. No id, no listeners — no
                // hitbox, so clicks pass straight through to the
                // control.
                .child(div().absolute().inset_0().child({
                    let flag = flag.clone();
                    canvas(
                        |_, _, _| (),
                        move |bounds: Bounds<Pixels>, _, _window: &mut Window, _| {
                            let (o, _) = flag.get();
                            flag.set((
                                o,
                                (
                                    bounds.origin.x.as_f32(),
                                    bounds.origin.y.as_f32(),
                                    bounds.size.width.as_f32(),
                                    bounds.size.height.as_f32(),
                                ),
                            ));
                        },
                    )
                    .size_full()
                }));
            if open {
                let mut panel = div()
                    .bg(rgb(th.panel))
                    .border_1()
                    .border_color(rgb(th.border))
                    .rounded_md()
                    .p_1()
                    .flex()
                    .flex_col()
                    .min_w(px(160.));
                for (i, opt) in options.iter().enumerate() {
                    let f = on_select.clone();
                    let flag = flag.clone();
                    pass.next_id += 1;
                    let mut row = div()
                        .id(pass.next_id)
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .text_color(rgb(th.text))
                        .hover(|s| s.bg(rgb(th.surface_hover)))
                        .child(SharedString::from(opt.as_str().to_string()))
                        .on_click(cx.listener(
                            move |this: &mut Root<C>, _ev, _window, cx| {
                                let (_, at) = flag.get();
                                flag.set((false, at));
                                match f.clone() {
                                    Some(f) => this.apply(cx, move |w| f(w, i as i64)),
                                    None => cx.notify(),
                                }
                            },
                        ));
                    if i as i64 == *selected {
                        row = row.text_color(rgb(th.accent));
                    }
                    panel = panel.child(row);
                }
                // The wrapper carries no id, listeners or hover style,
                // so it creates no hitbox: clicks around the panel
                // fall through to the content beneath (no scrim, no
                // occlude — a Select is lighter than a Modal).
                //
                // Anchored under the control via its recorded bounds
                // (a click opened it, so the control has painted and
                // the bounds are fresh); the panel matches the
                // control's width, the native-select look. A zeroed
                // record — never painted — falls back to centered.
                let (ax, ay, aw, ah) = at;
                let wrapper = if aw > 0.0 {
                    div()
                        .absolute()
                        .left(px(ax))
                        .top(px(ay + ah + 4.0))
                        .child(panel.w(px(aw)))
                } else {
                    div()
                        .absolute()
                        .inset_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(panel)
                };
                pass.overlays.push(deferred(wrapper).into_any_element());
            }
            control.into_any_element()
        }
        // Every option visible: one radio row each — a 14px ring, a
        // 6px accent dot when selected, the label — the whole row
        // clickable.
        Element::RadioGroup {
            options,
            selected,
            on_select,
        } => {
            pass.next_id += 1;
            let mut d = with_a11y(div().id(pass.next_id), el, sem)
                .flex()
                .flex_col()
                .gap(px(6.));
            for (i, opt) in options.iter().enumerate() {
                let f = on_select.clone();
                pass.next_id += 1;
                let mut ring = div()
                    .w(px(14.))
                    .h(px(14.))
                    .flex_none()
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(th.border))
                    .flex()
                    .items_center()
                    .justify_center();
                if i as i64 == *selected {
                    ring = ring.child(
                        div().w(px(6.)).h(px(6.)).rounded_full().bg(rgb(th.accent)),
                    );
                }
                d = d.child(
                    div()
                        .id(pass.next_id)
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .text_color(rgb(th.text))
                        .child(ring)
                        .child(SharedString::from(opt.as_str().to_string()))
                        .on_click(cx.listener(
                            move |this: &mut Root<C>, _ev, _window, cx| {
                                if let Some(f) = f.clone() {
                                    this.apply(cx, move |w| f(w, i as i64));
                                }
                            },
                        )),
                );
            }
            d.into_any_element()
        }
        // The horizontal chooser: padded clickable labels, the active
        // one in accent with a 2px underline (inactive tabs carry a
        // transparent underline so the strip keeps one height).
        Element::TabBar {
            labels,
            active,
            on_select,
        } => {
            pass.next_id += 1;
            let mut d = with_a11y(div().id(pass.next_id), el, sem)
                .flex()
                .flex_row()
                .gap(px(2.));
            for (i, label) in labels.iter().enumerate() {
                let f = on_select.clone();
                pass.next_id += 1;
                let is_active = i as i64 == *active;
                let mut tab = div()
                    .id(pass.next_id)
                    .px_3()
                    .py_1()
                    .cursor_pointer()
                    .border_b_2()
                    .child(SharedString::from(label.as_str().to_string()))
                    .on_click(cx.listener(
                        move |this: &mut Root<C>, _ev, _window, cx| {
                            if let Some(f) = f.clone() {
                                this.apply(cx, move |w| f(w, i as i64));
                            }
                        },
                    ));
                tab = if is_active {
                    tab.text_color(rgb(th.accent)).border_color(rgb(th.accent))
                } else {
                    tab.text_color(rgb(th.text)).border_color(hsla(0., 0., 0., 0.))
                };
                d = d.child(tab);
            }
            d.into_any_element()
        }
        // The joined pill chooser: a rounded surface (theme `surface`)
        // holding one segment per option, the selected one painted
        // solid in `accent` with window-colored text (the Checkbox/
        // Switch "readable on the accent" rule) and the others
        // hovering like a Button rests on the same surface tone. No
        // engine-side state — unlike Select's popover, every segment
        // is always visible, so a dump's `selected` is the whole
        // story.
        Element::Segmented {
            options,
            selected,
            on_select,
        } => {
            pass.next_id += 1;
            let mut d = with_a11y(div().id(pass.next_id), el, sem)
                .flex()
                .flex_row()
                .bg(rgb(th.surface))
                .rounded_full()
                .p(px(2.))
                .gap(px(2.));
            for (i, opt) in options.iter().enumerate() {
                let f = on_select.clone();
                pass.next_id += 1;
                let is_selected = i as i64 == *selected;
                let mut seg = div()
                    .id(pass.next_id)
                    .px_3()
                    .py_1()
                    .rounded_full()
                    .cursor_pointer()
                    .child(SharedString::from(opt.as_str().to_string()))
                    .on_click(cx.listener(
                        move |this: &mut Root<C>, _ev, _window, cx| {
                            if let Some(f) = f.clone() {
                                this.apply(cx, move |w| f(w, i as i64));
                            }
                        },
                    ));
                seg = if is_selected {
                    seg.bg(rgb(th.accent)).text_color(rgb(th.window_bg))
                } else {
                    seg.text_color(rgb(th.text))
                        .hover(|s| s.bg(rgb(th.surface_hover)))
                };
                d = d.child(seg);
            }
            d.into_any_element()
        }
    }
}

/// The app-wide animation clock — cute_ui's `PaintCtx::elapsedMs()`.
/// It must be a process clock, not per-element: `request_animation_frame`
/// notifies the root, so `render_el` rebuilds the whole tree every
/// frame, and an `Instant` captured while building the element would
/// read ~0 ms on every single paint — a spinner frozen at its start
/// angle.
static ANIM_CLOCK: std::sync::LazyLock<std::time::Instant> =
    std::sync::LazyLock::new(std::time::Instant::now);

const TAU: f32 = std::f32::consts::TAU;
/// cute_ui's spinner geometry: a 120° sweep, 3px stroke (2.5px here,
/// matching pixie's lighter chart strokes), polylined finely enough
/// that no segment reads as a straight edge.
const SPINNER_SWEEP: f32 = TAU / 3.0;
const SPINNER_STROKE: f32 = 2.5;
const SPINNER_ARC_SEGMENTS: usize = 24;
const SPINNER_RING_SEGMENTS: usize = 64;

/// One stroked arc as a polyline: `segments` chords from `from`
/// through `sweep` radians around (`cx`, `cy`). Screen axes, so
/// positive angles run clockwise from 3 o'clock. `None` when lyon
/// tessellates the stroke to nothing — a degenerate path paints
/// blank rather than panicking the frame (the LineChart rule).
fn spinner_arc(
    cx: f32,
    cy: f32,
    r: f32,
    from: f32,
    sweep: f32,
    segments: usize,
) -> Option<gpui::Path<Pixels>> {
    let at = |a: f32| point(px(cx + r * a.cos()), px(cy + r * a.sin()));
    let mut pb = PathBuilder::stroke(px(SPINNER_STROKE));
    pb.move_to(at(from));
    for k in 1..=segments {
        pb.line_to(at(from + sweep * k as f32 / segments as f32));
    }
    pb.build().ok()
}

/// Where a chart's labels sit: on bar-slot centers (BarChart, cute_ui's
/// `slot * (i + 0.5)`) or on sample x positions (LineChart, its
/// `sample_x` — first label at the left edge, last at the right).
#[derive(Clone, Copy)]
enum LabelAnchor {
    Slots,
    Samples,
}

/// The largest value in a chart's data, or `None` when there is
/// nothing to normalize by (empty data, or every value <= 0) — the
/// charts then paint nothing rather than dividing by zero.
fn chart_max(values: &[f32]) -> Option<f32> {
    let max = values.iter().copied().fold(f32::NAN, f32::max);
    (max > 0.0).then_some(max)
}

/// The shared chart frame: cute_ui's plot chrome (surface fill, 1px
/// border, 6px radius, 12px padding) wrapped around a plot box and its
/// label strip. `width`/`height` size the frame; `0.0` keeps the
/// full-bleed, 120px-plot geometry the charts shipped with. Labels are
/// ordinary text divs — only the plot itself is custom-painted.
fn chart_box(
    plot: gpui::AnyElement,
    labels: &List<Str>,
    samples: usize,
    width: f64,
    height: f64,
    anchor: LabelAnchor,
    th: &'static Theme,
) -> gpui::AnyElement {
    // The SCOPED theme, not the engine mirror: a `theme:` rider must
    // reskin the plot chrome along with everything else (the bars and
    // strokes already draw from `th`).
    let mut frame = div()
        .flex()
        .flex_col()
        .gap_1()
        .bg(rgb(th.panel))
        .border_1()
        .border_color(rgb(th.border))
        .rounded_md()
        .p_3();
    frame = if width > 0.0 {
        frame.w(px(width as f32))
    } else {
        frame.w_full()
    };
    let mut plot_box = div().w_full();
    if height > 0.0 {
        frame = frame.h(px(height as f32));
        // With the frame pinned, the plot takes whatever the label
        // strip leaves instead of overflowing it.
        plot_box = plot_box.flex_1().min_h(px(0.));
    } else {
        plot_box = plot_box.h(px(120.));
    }
    let mut frame = frame.child(plot_box.child(plot));
    // cute_ui stops at `n == 0` before it draws any label: with nothing
    // plotted there is no slot or sample to sit under.
    if !labels.is_empty() && samples > 0 {
        frame = frame.child(label_strip(labels, samples, anchor, th));
    }
    frame.into_any_element()
}

/// The label strip under a plot. `Slots` gives every sample an equal
/// cell, so the text centers on its bar; `Samples` absolute-positions
/// each label at its sample's x fraction — `left` is a fraction of the
/// strip, and a half-cell negative margin turns that anchor from the
/// label's left edge into its center.
fn label_strip(labels: &List<Str>, samples: usize, anchor: LabelAnchor, th: &'static Theme) -> gpui::Div {
    // A short `labels` labels only the leading samples (cute_ui's
    // `i < n && i < labels.size()`).
    let texts: Vec<String> = labels.iter().map(|l| l.as_str().to_string()).collect();
    // Explicitly the SCOPED text color: inheritance would reach back
    // to the window root, which a subtree `theme:` scope does not own.
    let mut strip = div().w_full().flex_none().text_xs().text_color(rgb(th.text));
    match anchor {
        LabelAnchor::Slots => {
            strip = strip.flex().flex_row();
            for i in 0..samples {
                let t = texts.get(i).cloned().unwrap_or_default();
                strip = strip.child(div().flex_1().text_center().child(SharedString::from(t)));
            }
        }
        LabelAnchor::Samples => {
            // Absolute children contribute no height, so the strip
            // states its own (one `text_xs` line).
            strip = strip.relative().h(px(16.));
            let cell = 44.0f32;
            for (i, t) in texts.iter().take(samples).enumerate() {
                let frac = if samples == 1 {
                    0.5
                } else {
                    i as f32 / (samples - 1) as f32
                };
                strip = strip.child(
                    div()
                        .absolute()
                        .left(relative(frac))
                        .ml(px(-cell / 2.0))
                        .w(px(cell))
                        .text_center()
                        // The cell is a positioning box, not a column:
                        // a long label overhangs it rather than
                        // wrapping out of the one-line strip.
                        .whitespace_nowrap()
                        .child(SharedString::from(t.clone())),
                );
            }
        }
    }
    strip
}

/// Rung-2 hookup: the generated main hands the engine its source path
/// plus a reload closure. On file change the engine calls `reload`
/// with the live World; `true` means the edit was absorbed in-process
/// (re-render), `false` means it needs a real rebuild (the outer
/// `pixie watch` sees the same change and takes it).
pub struct ReloadWatch {
    pub path: std::path::PathBuf,
    pub reload: Box<dyn Fn(&mut World) -> bool>,
}

/// Resolves an `Image`/`Svg` `source` path the way cute_ui resolves
/// asset paths against `QCoreApplication::applicationDirPath` (DESIGN
/// §11.15): an absolute path passes through unchanged; a relative path
/// is tried, in order, against the process's current working directory
/// and then the running executable's directory, and the first
/// candidate that exists on disk wins. When neither exists, the
/// cwd-joined path comes back — the same path a bare relative source
/// resolved to before this helper existed, so a still-missing asset
/// fails exactly as it always has (silent-empty paint, never a panic).
/// The one call site every `source`-bearing widget/asset loader routes
/// through: the `Image` arm, the `Svg` arm, and `PixieAssets::load`.
fn resolve_asset(source: &str) -> std::path::PathBuf {
    let cwd = std::env::current_dir().ok();
    // Symlink-resolve where possible (e.g. a `cargo run` binary can be
    // reached through a symlink into a hashed build dir) — best effort:
    // fall back to the raw path rather than give up the exe-dir leg.
    let exe_dir = std::env::current_exe()
        .ok()
        .map(|exe| exe.canonicalize().unwrap_or(exe))
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));
    resolve_asset_chain(source, cwd.as_deref(), exe_dir.as_deref())
}

/// The pure resolution chain behind `resolve_asset`, taking the cwd and
/// exe-dir as parameters instead of reading process state directly.
/// `std::env::current_exe()` has no test-time override, so this is the
/// seam the unit tests below drive with temp dirs standing in for both
/// — `resolve_asset` itself is exercised only for the state-independent
/// absolute-path case.
fn resolve_asset_chain(
    source: &str,
    cwd: Option<&std::path::Path>,
    exe_dir: Option<&std::path::Path>,
) -> std::path::PathBuf {
    let path = std::path::Path::new(source);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let cwd_candidate = match cwd {
        Some(cwd) => cwd.join(path),
        None => path.to_path_buf(),
    };
    if cwd_candidate.exists() {
        return cwd_candidate;
    }
    if let Some(dir) = exe_dir {
        let exe_candidate = dir.join(path);
        if exe_candidate.exists() {
            return exe_candidate;
        }
    }
    cwd_candidate
}

#[cfg(test)]
mod resolve_asset_tests {
    use super::{resolve_asset, resolve_asset_chain};
    use std::fs;

    #[test]
    fn absolute_path_passes_through_untouched() {
        let cwd = tempfile::tempdir().unwrap();
        let exe_dir = tempfile::tempdir().unwrap();
        let abs = cwd.path().join("art.png");
        let resolved = resolve_asset_chain(
            abs.to_str().unwrap(),
            Some(cwd.path()),
            Some(exe_dir.path()),
        );
        assert_eq!(resolved, abs);
    }

    #[test]
    fn resolve_asset_wrapper_passes_absolute_paths_through_too() {
        // Exercises the real `resolve_asset(&str)` entry point (real
        // cwd, real current_exe) rather than the injectable core —
        // safe under parallel test execution because the absolute-path
        // branch touches no shared process state.
        let abs = std::path::Path::new("/tmp/pixie-resolve-asset-does-not-need-to-exist.png");
        assert_eq!(resolve_asset(abs.to_str().unwrap()), abs.to_path_buf());
    }

    #[test]
    fn relative_path_prefers_cwd_over_exe_dir() {
        let cwd = tempfile::tempdir().unwrap();
        let exe_dir = tempfile::tempdir().unwrap();
        fs::write(cwd.path().join("art.png"), b"cwd").unwrap();
        fs::write(exe_dir.path().join("art.png"), b"exe").unwrap();
        let resolved = resolve_asset_chain("art.png", Some(cwd.path()), Some(exe_dir.path()));
        assert_eq!(resolved, cwd.path().join("art.png"));
    }

    #[test]
    fn relative_path_falls_back_to_exe_dir_when_absent_from_cwd() {
        let cwd = tempfile::tempdir().unwrap();
        let exe_dir = tempfile::tempdir().unwrap();
        fs::write(exe_dir.path().join("art.png"), b"exe").unwrap();
        let resolved = resolve_asset_chain("art.png", Some(cwd.path()), Some(exe_dir.path()));
        assert_eq!(resolved, exe_dir.path().join("art.png"));
    }

    #[test]
    fn relative_path_falls_back_to_cwd_joined_form_when_neither_has_it() {
        let cwd = tempfile::tempdir().unwrap();
        let exe_dir = tempfile::tempdir().unwrap();
        let resolved =
            resolve_asset_chain("missing/art.png", Some(cwd.path()), Some(exe_dir.path()));
        assert_eq!(resolved, cwd.path().join("missing/art.png"));
    }

    #[test]
    fn nested_relative_paths_mirror_the_demo_shape() {
        // examples/gallery and examples/icons pass paths shaped like
        // "examples/gallery/pixie.png".
        let cwd = tempfile::tempdir().unwrap();
        let exe_dir = tempfile::tempdir().unwrap();
        let nested = cwd.path().join("examples/gallery");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("pixie.png"), b"png").unwrap();
        let resolved = resolve_asset_chain(
            "examples/gallery/pixie.png",
            Some(cwd.path()),
            Some(exe_dir.path()),
        );
        assert_eq!(resolved, nested.join("pixie.png"));
    }

    #[test]
    fn missing_cwd_falls_back_to_bare_relative_path() {
        let exe_dir = tempfile::tempdir().unwrap();
        let resolved = resolve_asset_chain("art.png", None, Some(exe_dir.path()));
        assert_eq!(resolved, std::path::PathBuf::from("art.png"));
    }
}

/// Reads asset bytes straight off the filesystem, through the same
/// cwd → exe-dir `resolve_asset` chain `Image`'s `source` uses (DESIGN
/// §11.15). `Image` never needed this directly (`gpui::img` reads a
/// `Resource::Path` straight off disk once handed an already-resolved
/// path), but `svg().path(...)` also round-trips its path string
/// through gpui's installed `AssetSource`, and the default no-op one
/// (`()`) always answers `Ok(None)` — so without this, every `Svg`
/// would silently paint nothing. The `Svg` render arm already hands
/// this a resolved path, so resolving again here is a no-op
/// passthrough for that caller; resolving independently keeps this the
/// one place any future `AssetSource` consumer can rely on without
/// re-deriving the chain itself.
struct PixieAssets;

impl gpui::AssetSource for PixieAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        match std::fs::read(resolve_asset(path)) {
            Ok(bytes) => Ok(Some(std::borrow::Cow::Owned(bytes))),
            // A missing/unreadable asset is a blank icon, never a
            // paint-time crash.
            Err(_) => Ok(None),
        }
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

/// Open a window on the view and run until close. `runtime` must wrap
/// the World already holding the mounted view (the generated `main()`
/// does the mounting and the wrapping).
pub fn run_app<C: Component>(
    runtime: Runtime,
    view: Handle<C>,
    title: &str,
    watch: Option<ReloadWatch>,
    win: Option<(f64, f64)>,
) {
    // Startup theme: `PIXIE_THEME=light` (anything else, or unset, is
    // dark — the palette the engine always had). It has to land BEFORE
    // the first build now: the tree carries resolved colors (§8.37).
    if std::env::var("PIXIE_THEME").is_ok_and(|v| v == "light") {
        set_theme_light(true);
    }
    let light = THEME_LIGHT_ON.load(std::sync::atomic::Ordering::Relaxed);
    let tree = runtime.with(|w| {
        let _ = w.take_dirty_views();
        pixie_kernel::theme::set_light(w, light);
        pixie_kernel::anim::set_now(w, ANIM_CLOCK.elapsed().as_secs_f64() * 1000.0);
        pixie_kernel::build_prepared(w, view)
    });
    let title: SharedString = title.to_string().into();
    gpui_platform::application().with_assets(PixieAssets).run(move |cx: &mut App| {
        // Awaited binding calls run on gpui's background thread pool.
        let bg = cx.background_executor().clone();
        pixie_kernel::set_worker_spawner(move |f| {
            bg.spawn(async move { f() }).detach();
        });
        text_input::bind_keys(cx);
        // cute_ui's Cmd+T: flip the theme live. Every color is read
        // per paint, so one refresh restyles the whole window.
        cx.bind_keys([gpui::KeyBinding::new("cmd-t", ToggleTheme, None)]);
        cx.on_action(|_: &ToggleTheme, cx: &mut App| {
            set_theme_light(!THEME_LIGHT_ON.load(std::sync::atomic::Ordering::Relaxed));
            cx.refresh_windows();
        });
        // The app's requested window size (pixie.toml `[window]`,
        // yokan `ui.run(width=, height=)`); the historical 420x560
        // when nothing asks. Floor at 200 so a typo cannot produce an
        // ungrabbable window.
        let (win_w, win_h) = win
            .map(|(w, h)| (w.max(200.0) as f32, h.max(200.0) as f32))
            .unwrap_or((420.0, 560.0));
        let bounds = Bounds::centered(None, size(px(win_w), px(win_h)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(title.clone()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |_, cx| {
                    let images = cx.new(|_| PixieImageCache::new());
                    cx.new(move |_| Root {
                        runtime,
                        view,
                        tree,
                        inputs: HashMap::new(),
                        scrolls: HashMap::new(),
                        selects: HashMap::new(),
                        pumping: false,
                        images,
                    })
                },
            )
            .unwrap();
        if let Some(watch) = watch {
            let mut last_mtime = std::fs::metadata(&watch.path)
                .ok()
                .and_then(|m| m.modified().ok());
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    let mtime = std::fs::metadata(&watch.path)
                        .ok()
                        .and_then(|m| m.modified().ok());
                    if mtime == last_mtime {
                        continue;
                    }
                    last_mtime = mtime;
                    let alive = window.update(cx, |root, _window, cx| {
                        let view = root.view;
                        let handled = root.runtime.with(|w| (watch.reload)(w));
                        if handled {
                            root.tree = root.runtime.with(|w| {
                                pixie_kernel::build_prepared(w, view)
                            });
                            cx.notify();
                        }
                    });
                    if alive.is_err() {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Spinner's arc is geometry, not an `Animation`: it must
    /// tessellate at every phase (a degenerate stroke paints nothing,
    /// silently) and it must MOVE with the phase. Both are checkable
    /// without a window, so a dark screen cannot hide a dead arc.
    #[test]
    fn spinner_arc_tessellates_and_rotates() {
        let (cx, cy, r) = (100.0f32, 100.0f32, 14.0f32);
        let ring = spinner_arc(cx, cy, r, 0.0, TAU, SPINNER_RING_SEGMENTS)
            .expect("the background ring tessellates");
        let mut prev: Option<Bounds<Pixels>> = None;
        for step in 0..8 {
            let phase = step as f32 / 8.0;
            let arc = spinner_arc(cx, cy, r, phase * TAU, SPINNER_SWEEP, SPINNER_ARC_SEGMENTS)
                .unwrap_or_else(|| panic!("the arc tessellates at phase {phase}"));
            // A 120° slice covers less of the box than the full ring.
            assert!(
                arc.bounds.size.width < ring.bounds.size.width
                    || arc.bounds.size.height < ring.bounds.size.height,
                "phase {phase}: the arc should not span the whole ring"
            );
            if let Some(p) = prev {
                assert_ne!(
                    p.origin, arc.bounds.origin,
                    "phase {phase}: the arc did not move"
                );
            }
            prev = Some(arc.bounds);
        }
    }

    /// Screen axes (y grows down), so a quarter turn walks the arc's
    /// start from 3 o'clock to 6 to 9 to 12 — the direction the eye
    /// reads as clockwise, and the convention the reduced-motion phase
    /// (0.75 = parked at 12 o'clock) relies on.
    #[test]
    fn spinner_arc_starts_where_the_phase_says() {
        let r = 10.0f32;
        let start = |phase: f32| {
            let a = phase * TAU;
            (r * a.cos(), r * a.sin())
        };
        assert!(start(0.0).0 > 9.9, "phase 0 starts at 3 o'clock");
        assert!(start(0.25).1 > 9.9, "phase 0.25 starts at 6 o'clock");
        assert!(start(0.5).0 < -9.9, "phase 0.5 starts at 9 o'clock");
        assert!(start(0.75).1 < -9.9, "phase 0.75 starts at 12 o'clock");
    }
}
