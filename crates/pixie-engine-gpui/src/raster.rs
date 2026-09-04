//! The canvas rasterizer: a list of drawing commands into pixels.
//!
//! Every other element in the catalog is laid out and painted by gpui.
//! A canvas cannot be: gpui's paths are antialiased and its image
//! sampler is `mag_filter::linear` (written into the Metal shader), so
//! a grid of virtual pixels drawn with either would arrive soft, which
//! is the one thing dot art must not be. So the commands are painted
//! into a buffer here — integer coordinates, no blending — and the
//! result is handed to `Window::paint_image` as a single image sized
//! in DEVICE pixels, where the linear sampler has nothing to
//! interpolate.
//!
//! The buffer is BGRA because that is what `RenderImage` holds.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::RenderImage;
use pixie_kernel::Op;

/// Everything a frame's pixels depend on. Two frames with equal specs
/// are the same picture, which is what lets a canvas that did not
/// change keep the image — and the atlas tile — it already had.
#[derive(Clone, PartialEq)]
pub struct Spec {
    /// Virtual pixels across and down.
    pub w: i64,
    pub h: i64,
    /// Logical pixels per virtual pixel.
    pub scale: i64,
    /// The palette index the surface is cleared to.
    pub bg: i64,
    /// The palette, already resolved to BGRA (theme tokens included),
    /// so a theme flip is a different spec.
    pub palette: Vec<[u8; 4]>,
    pub ops: Vec<Op>,
    /// Device pixels per logical pixel, rounded to an integer: the
    /// upscale has to be a whole number or the nearest-neighbor
    /// expansion would not land on device pixels. macOS gives 1 or 2.
    pub dpr: i64,
}

/// What one canvas element keeps between frames, keyed by its path in
/// the element tree (the `scrolls` rule).
pub struct Cached {
    pub spec: Spec,
    pub image: Arc<RenderImage>,
}

pub type Store = HashMap<Vec<usize>, Cached>;

/// Decoded sprite sheets, by the source string the command names.
///
/// Not gpui's image cache: that one decodes on a background thread and
/// answers on a later frame, and a rasterizer needs the pixels now. A
/// source that cannot be read is remembered as absent so a missing
/// file costs one failed open, not one per frame.
#[derive(Default)]
pub struct Sprites {
    sheets: HashMap<String, Option<Sheet>>,
}

struct Sheet {
    w: i64,
    h: i64,
    /// BGRA, straight from the file: alpha 0 is transparent, which is
    /// what an asset with a real alpha channel already says. `colkey`
    /// is the other way to say it, for sheets that have none.
    px: Vec<[u8; 4]>,
}

impl Sprites {
    fn get(&mut self, source: &str, resolve: fn(&str) -> PathBuf) -> Option<&Sheet> {
        if !self.sheets.contains_key(source) {
            let sheet = decode(&resolve(source));
            self.sheets.insert(source.to_string(), sheet);
        }
        self.sheets.get(source).and_then(|s| s.as_ref())
    }
}

fn decode(path: &std::path::Path) -> Option<Sheet> {
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let px = img
        .pixels()
        .map(|p| {
            let [r, g, b, a] = p.0;
            [b, g, r, a]
        })
        .collect();
    Some(Sheet {
        w: w as i64,
        h: h as i64,
        px,
    })
}

/// The surface a frame is painted into: virtual pixels, opaque.
struct Surface {
    w: i64,
    h: i64,
    px: Vec<[u8; 4]>,
}

impl Surface {
    fn new(w: i64, h: i64, bg: [u8; 4]) -> Self {
        Surface {
            w,
            h,
            px: vec![bg; (w * h) as usize],
        }
    }

    #[inline]
    fn set(&mut self, x: i64, y: i64, c: [u8; 4]) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return;
        }
        self.px[(y * self.w + x) as usize] = c;
    }

    fn rect(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 4]) {
        for dy in 0..h {
            for dx in 0..w {
                self.set(x + dx, y + dy, c);
            }
        }
    }

    fn rect_outline(&mut self, x: i64, y: i64, w: i64, h: i64, c: [u8; 4]) {
        if w <= 0 || h <= 0 {
            return;
        }
        for dx in 0..w {
            self.set(x + dx, y, c);
            self.set(x + dx, y + h - 1, c);
        }
        for dy in 0..h {
            self.set(x, y + dy, c);
            self.set(x + w - 1, y + dy, c);
        }
    }

    /// Bresenham, so a line is the same pixels whichever end it is
    /// drawn from.
    fn line(&mut self, x1: i64, y1: i64, x2: i64, y2: i64, c: [u8; 4]) {
        let (dx, dy) = ((x2 - x1).abs(), -(y2 - y1).abs());
        let (sx, sy) = (if x1 < x2 { 1 } else { -1 }, if y1 < y2 { 1 } else { -1 });
        let (mut x, mut y) = (x1, y1);
        let mut err = dx + dy;
        loop {
            self.set(x, y, c);
            if x == x2 && y == y2 {
                return;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// A filled disc by the distance test — the shape a pixel machine
    /// draws, and symmetric by construction.
    fn circle(&mut self, cx: i64, cy: i64, r: i64, c: [u8; 4]) {
        if r < 0 {
            return;
        }
        let rr = r * r;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= rr {
                    self.set(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// The ring of the same disc: a pixel is on the outline when it is
    /// inside and at least one of its four neighbours is not.
    fn circle_outline(&mut self, cx: i64, cy: i64, r: i64, c: [u8; 4]) {
        if r < 0 {
            return;
        }
        let rr = r * r;
        let inside = |dx: i64, dy: i64| dx * dx + dy * dy <= rr;
        for dy in -r..=r {
            for dx in -r..=r {
                if inside(dx, dy)
                    && !(inside(dx - 1, dy)
                        && inside(dx + 1, dy)
                        && inside(dx, dy - 1)
                        && inside(dx, dy + 1))
                {
                    self.set(cx + dx, cy + dy, c);
                }
            }
        }
    }

    /// A filled triangle by the half-plane test over its bounding box.
    /// Small canvases make the box cheap, and the test is exact in
    /// integers — no edge lands differently from one frame to the next.
    #[allow(clippy::too_many_arguments)]
    fn triangle(
        &mut self,
        x1: i64,
        y1: i64,
        x2: i64,
        y2: i64,
        x3: i64,
        y3: i64,
        c: [u8; 4],
    ) {
        let lo_x = x1.min(x2).min(x3).max(0);
        let hi_x = x1.max(x2).max(x3).min(self.w - 1);
        let lo_y = y1.min(y2).min(y3).max(0);
        let hi_y = y1.max(y2).max(y3).min(self.h - 1);
        let edge = |ax: i64, ay: i64, bx: i64, by: i64, px: i64, py: i64| {
            (bx - ax) * (py - ay) - (by - ay) * (px - ax)
        };
        for y in lo_y..=hi_y {
            for x in lo_x..=hi_x {
                let a = edge(x1, y1, x2, y2, x, y);
                let b = edge(x2, y2, x3, y3, x, y);
                let d = edge(x3, y3, x1, y1, x, y);
                let neg = a < 0 || b < 0 || d < 0;
                let pos = a > 0 || b > 0 || d > 0;
                if !(neg && pos) {
                    self.set(x, y, c);
                }
            }
        }
    }
}

/// Paint one frame and hand back the image, in device pixels.
/// `None` when the canvas has no area to paint.
pub fn render(
    spec: &Spec,
    sprites: &mut Sprites,
    resolve: fn(&str) -> PathBuf,
) -> Option<Arc<RenderImage>> {
    if spec.w <= 0 || spec.h <= 0 {
        return None;
    }
    let color = |i: i64| -> [u8; 4] {
        if spec.palette.is_empty() {
            // The same magenta `pixie_kernel::palette_color` answers,
            // so what the dump says and what the window shows agree.
            return [0xff, 0x00, 0xff, 0xff];
        }
        let last = spec.palette.len() - 1;
        spec.palette[if i < 0 { 0 } else { (i as usize).min(last) }]
    };
    let mut s = Surface::new(spec.w, spec.h, color(spec.bg));
    for op in &spec.ops {
        match op {
            Op::Pixel { x, y, color: c } => s.set(*x, *y, color(*c)),
            Op::Line {
                x1,
                y1,
                x2,
                y2,
                color: c,
            } => s.line(*x1, *y1, *x2, *y2, color(*c)),
            Op::Rect { x, y, w, h, color: c } => s.rect(*x, *y, *w, *h, color(*c)),
            Op::RectOutline { x, y, w, h, color: c } => {
                s.rect_outline(*x, *y, *w, *h, color(*c))
            }
            Op::Circle { x, y, r, color: c } => s.circle(*x, *y, *r, color(*c)),
            Op::CircleOutline { x, y, r, color: c } => s.circle_outline(*x, *y, *r, color(*c)),
            Op::Triangle {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                color: c,
            } => s.triangle(*x1, *y1, *x2, *y2, *x3, *y3, color(*c)),
            Op::TriangleOutline {
                x1,
                y1,
                x2,
                y2,
                x3,
                y3,
                color: c,
            } => {
                let c = color(*c);
                s.line(*x1, *y1, *x2, *y2, c);
                s.line(*x2, *y2, *x3, *y3, c);
                s.line(*x3, *y3, *x1, *y1, c);
            }
            Op::Sprite {
                x,
                y,
                source,
                u,
                v,
                w,
                h,
                colkey,
                flip_x,
                flip_y,
            } => {
                let key = (*colkey >= 0).then(|| color(*colkey));
                let Some(sheet) = sprites.get(source.as_str(), resolve) else {
                    // A source that is not there paints NOTHING. A
                    // placeholder box is right for an `Image` in a
                    // layout and wrong in the middle of a frame.
                    continue;
                };
                for dy in 0..*h {
                    for dx in 0..*w {
                        let sx = u + if *flip_x { w - 1 - dx } else { dx };
                        let sy = v + if *flip_y { h - 1 - dy } else { dy };
                        if sx < 0 || sy < 0 || sx >= sheet.w || sy >= sheet.h {
                            continue;
                        }
                        let p = sheet.px[(sy * sheet.w + sx) as usize];
                        if p[3] == 0 {
                            continue;
                        }
                        if key.is_some_and(|k| k[0] == p[0] && k[1] == p[1] && k[2] == p[2]) {
                            continue;
                        }
                        s.set(x + dx, y + dy, [p[0], p[1], p[2], 0xff]);
                    }
                }
            }
            Op::PixelText { x, y, text, color: c } => {
                let c = color(*c);
                for (i, ch) in text.as_str().chars().enumerate() {
                    let ox = x + i as i64 * crate::font::CELL_W;
                    for row in 0..crate::font::CELL_H {
                        for col in 0..crate::font::CELL_W {
                            if crate::font::dot(ch, col, row) {
                                s.set(ox + col, y + row, c);
                            }
                        }
                    }
                }
            }
        }
    }
    Some(Arc::new(RenderImage::new(smallvec::smallvec![
        image::Frame::new(expand(&s, spec.scale.max(1) * spec.dpr.max(1))?)
    ])))
}

/// Nearest-neighbor, by construction: every virtual pixel becomes a
/// square of `f` device pixels. One row is written once and copied,
/// so the cost is a memcpy rather than a per-pixel loop.
fn expand(s: &Surface, f: i64) -> Option<image::RgbaImage> {
    let (dw, dh) = ((s.w * f) as usize, (s.h * f) as usize);
    let mut out: Vec<u8> = vec![0; dw * dh * 4];
    let mut row: Vec<u8> = vec![0; dw * 4];
    for y in 0..s.h as usize {
        for x in 0..s.w as usize {
            let c = s.px[y * s.w as usize + x];
            for k in 0..f as usize {
                let at = (x * f as usize + k) * 4;
                row[at..at + 4].copy_from_slice(&c);
            }
        }
        for k in 0..f as usize {
            let at = (y * f as usize + k) * dw * 4;
            out[at..at + dw * 4].copy_from_slice(&row);
        }
    }
    image::RgbaImage::from_raw(dw as u32, dh as u32, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixie_kernel::Str;

    fn spec(ops: Vec<Op>) -> Spec {
        Spec {
            w: 4,
            h: 3,
            scale: 1,
            bg: 0,
            palette: vec![[0, 0, 0, 255], [255, 255, 255, 255]],
            ops,
            dpr: 1,
        }
    }

    fn pixels(spec: &Spec) -> Vec<[u8; 4]> {
        let img = render(spec, &mut Sprites::default(), |s| PathBuf::from(s)).unwrap();
        img.as_bytes(0)
            .unwrap()
            .chunks(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect()
    }

    #[test]
    fn the_background_fills_and_a_command_paints_over_it() {
        let px = pixels(&spec(vec![Op::Pixel {
            x: 1,
            y: 1,
            color: 1,
        }]));
        assert_eq!(px.len(), 12);
        assert_eq!(px[0], [0, 0, 0, 255]);
        assert_eq!(px[5], [255, 255, 255, 255], "the pixel at (1,1)");
    }

    /// Out of bounds is dropped, not wrapped: a command that leaves the
    /// canvas leaves it.
    #[test]
    fn a_command_outside_the_grid_paints_nothing() {
        let px = pixels(&spec(vec![
            Op::Pixel {
                x: 9,
                y: 0,
                color: 1,
            },
            Op::Pixel {
                x: -1,
                y: 0,
                color: 1,
            },
        ]));
        assert!(px.iter().all(|p| *p == [0, 0, 0, 255]));
    }

    #[test]
    fn scale_repeats_every_virtual_pixel() {
        let mut s = spec(vec![Op::Pixel {
            x: 0,
            y: 0,
            color: 1,
        }]);
        s.scale = 3;
        let px = pixels(&s);
        assert_eq!(px.len(), 12 * 9);
        // The whole 3x3 block is the one virtual pixel.
        for y in 0..3usize {
            for x in 0..3usize {
                assert_eq!(px[y * 12 + x], [255, 255, 255, 255]);
            }
        }
        assert_eq!(px[3], [0, 0, 0, 255], "and the next block is not");
    }

    /// An index past the end paints the last color rather than
    /// vanishing, and an empty palette paints magenta — the rule
    /// `pixie_kernel::palette_color` states for the dump.
    #[test]
    fn colors_clamp_and_an_empty_palette_is_magenta() {
        let px = pixels(&spec(vec![Op::Pixel {
            x: 0,
            y: 0,
            color: 7,
        }]));
        assert_eq!(px[0], [255, 255, 255, 255]);

        let mut s = spec(vec![Op::Pixel {
            x: 0,
            y: 0,
            color: 0,
        }]);
        s.palette = Vec::new();
        assert_eq!(pixels(&s)[0], [255, 0, 255, 255]);
    }

    #[test]
    #[ignore = "measurement, not an assertion"]
    fn measure_a_game_sized_frame() {
        let mut ops = Vec::new();
        for i in 0..200i64 {
            ops.push(Op::Pixel { x: i * 7 % 160, y: i * 11 % 120, color: (i % 4) as i64 });
            ops.push(Op::Rect { x: i * 7 % 160, y: i * 11 % 120, w: 8, h: 8, color: (i % 4) as i64 });
        }
        ops.push(Op::PixelText { x: 2, y: 2, text: Str::from("SCORE 1250"), color: 3 });
        let spec = Spec {
            w: 160,
            h: 120,
            scale: 4,
            bg: 0,
            palette: vec![[0, 0, 0, 255], [255, 255, 255, 255], [40, 60, 90, 255], [200, 60, 90, 255]],
            ops,
            dpr: 2,
        };
        let mut sprites = Sprites::default();
        let n = 200;
        let t = std::time::Instant::now();
        for _ in 0..n {
            let img = render(&spec, &mut sprites, |s| PathBuf::from(s)).unwrap();
            std::hint::black_box(img.as_bytes(0).unwrap().len());
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / n as f64;
        println!("401 commands, 160x120 at scale 4, dpr 2 (1280x960): {ms:.3} ms/frame");
    }

    #[test]
    fn a_missing_sprite_paints_nothing() {
        let px = pixels(&spec(vec![Op::Sprite {
            x: 0,
            y: 0,
            source: Str::from("no-such-file.png"),
            u: 0,
            v: 0,
            w: 2,
            h: 2,
            colkey: -1,
            flip_x: false,
            flip_y: false,
        }]));
        assert!(px.iter().all(|p| *p == [0, 0, 0, 255]));
    }

    #[test]
    fn text_paints_its_glyphs() {
        let mut s = spec(vec![Op::PixelText {
            x: 0,
            y: 0,
            text: Str::from("L"),
            color: 1,
        }]);
        s.w = 8;
        s.h = 8;
        let px = pixels(&s);
        assert_eq!(px[0], [255, 255, 255, 255], "the stem's top");
        assert_eq!(px[1], [0, 0, 0, 255], "and nothing beside it");
        assert_eq!(px[4 * 8 + 2], [255, 255, 255, 255], "the foot's end");
    }
}
