//! Semantic color tokens, scoped to a subtree (§8.37).
//!
//! Theme used to be a process global in the engine: one `static`, one
//! atomic, read fresh at every paint. That works exactly as long as an
//! app wants ONE theme. It cannot express a light panel inside a dark
//! window, which is the thing Qt's palette inheritance has done for
//! thirty years and the smallest real demand for downward context.
//!
//! So the palette moved here and the resolution moved into the tree.
//! `build_prepared` runs `resolve` between `build` and the animation
//! settle, walking with an active theme and replacing every token NAME
//! in a color slot with the hex it stands for. Three things fall out:
//!
//! - A `theme:` rider scopes a subtree, because the walk carries the
//!   active theme rather than reading a global.
//! - The tier gate sees themed colors, because they are in the tree.
//! - Token colors ANIMATE. §8.35 can only tween two hex endpoints, and
//!   after this pass that is what a token is — so a theme flip under
//!   `animate:` crossfades, which is what §11.19 was asking for.

use crate::{Element, Str, World};

/// Every color pixie paints, by meaning rather than by value. Solid
/// tokens are `0xrrggbb`; the `_rgba` ones carry alpha.
#[derive(Clone, Copy)]
pub struct Theme {
    pub window_bg: u32,
    /// Recessed panel fill (chart frames, image fallback).
    pub panel: u32,
    /// Text-field editing surface.
    pub field_bg: u32,
    /// Raised control rest color (buttons) + its hover/press pair.
    pub surface: u32,
    pub surface_hover: u32,
    pub surface_pressed: u32,
    pub border: u32,
    pub text: u32,
    pub text_dim_rgba: u32,
    pub accent: u32,
    pub selection_rgba: u32,
    pub scrim_rgba: u32,
    pub scrollbar: u32,
    pub scrollbar_active: u32,
}

/// The palette the engine has always painted (Catppuccin-Mocha-ish).
pub const DARK: Theme = Theme {
    window_bg: 0x1e1e2e,
    panel: 0x181825,
    field_bg: 0x11111b,
    surface: 0x45475a,
    surface_hover: 0x585b70,
    surface_pressed: 0x6c7086,
    border: 0x313244,
    text: 0xcdd6f4,
    text_dim_rgba: 0xffffff40,
    accent: 0x89b4fa,
    selection_rgba: 0x89b4fa33,
    scrim_rgba: 0x00000099,
    scrollbar: 0x45475a,
    scrollbar_active: 0x585b70,
};

/// The light twin (Latte-ish), token for token.
pub const LIGHT: Theme = Theme {
    window_bg: 0xeff1f5,
    panel: 0xe6e9ef,
    field_bg: 0xffffff,
    surface: 0xccd0da,
    surface_hover: 0xbcc0cc,
    surface_pressed: 0xacb0be,
    border: 0xbcc0cc,
    text: 0x4c4f69,
    text_dim_rgba: 0x4c4f6980,
    accent: 0x1e66f5,
    selection_rgba: 0x1e66f533,
    scrim_rgba: 0x00000066,
    scrollbar: 0xbcc0cc,
    scrollbar_active: 0x8c8fa1,
};

/// The theme names a `theme:` rider accepts.
pub const NAMES: &[&str] = &["dark", "light"];

pub fn by_name(name: &str) -> Option<&'static Theme> {
    match name {
        "dark" => Some(&DARK),
        "light" => Some(&LIGHT),
        _ => None,
    }
}

impl Theme {
    /// The token vocabulary a color string may name. `None` for
    /// anything else — a hex literal, or a typo, which degrades to
    /// the engine's default rather than aborting a frame (cute's
    /// QColor contract).
    pub fn token(&self, name: &str) -> Option<u32> {
        Some(match name {
            "windowBg" => self.window_bg,
            "panel" => self.panel,
            "fieldBg" => self.field_bg,
            "surface" => self.surface,
            "surfaceHover" => self.surface_hover,
            "surfacePressed" => self.surface_pressed,
            "border" => self.border,
            "text" => self.text,
            "accent" => self.accent,
            "scrollbar" => self.scrollbar,
            "scrollbarActive" => self.scrollbar_active,
            // The alpha-carrying half.
            "textDim" => return Some(self.text_dim_rgba | 0),
            "selection" => return Some(self.selection_rgba),
            "scrim" => return Some(self.scrim_rgba),
            _ => return None,
        })
    }

    /// Whether a token name carries alpha, so `resolve` knows to emit
    /// eight hex digits instead of six.
    fn token_has_alpha(name: &str) -> bool {
        matches!(name, "textDim" | "selection" | "scrim")
    }

    fn token_hex(&self, name: &str) -> Option<Str> {
        let v = self.token(name)?;
        Some(if Theme::token_has_alpha(name) {
            Str::from(format!("#{v:08x}"))
        } else {
            Str::from(format!("#{:06x}", v & 0x00ff_ffff))
        })
    }
}

/// The root theme, in the World rather than in a `static`: it is an
/// input to the tree now, so flipping it is an ordinary rebuild.
#[derive(Default)]
pub struct ThemeStore {
    light: bool,
}

fn store(w: &mut World) -> crate::Handle<ThemeStore> {
    w.singleton::<ThemeStore>(ThemeStore::default)
}

pub fn set_light(w: &mut World, on: bool) {
    let h = store(w);
    w.get_mut(h).light = on;
}

pub fn is_light(w: &World) -> bool {
    match w.try_singleton_ref::<ThemeStore>() {
        Some(h) => w.get(h).light,
        None => false,
    }
}

/// The theme at the ROOT of the tree. Engine chrome outside any
/// element (the window background) paints with this; everything
/// inside the tree uses whatever `theme:` scope it sits in.
pub fn root(w: &World) -> &'static Theme {
    if is_light(w) { &LIGHT } else { &DARK }
}

/// The color slots a theme token may appear in. Kept next to the
/// palette rather than inside the animation module because both need
/// it and they must agree on the list: a color the theme can set but
/// animation cannot tween would be a silent inconsistency.
fn color_slots(el: &mut Element) -> Vec<&mut Str> {
    match el {
        Element::Text {
            color,
            background,
            border_color,
            ..
        } => vec![color, background, border_color],
        Element::Button {
            background,
            hover_background,
            active_background,
            color,
            border_color,
            ..
        } => vec![
            background,
            hover_background,
            active_background,
            color,
            border_color,
        ],
        Element::Column {
            background,
            border_color,
            ..
        }
        | Element::Row {
            background,
            border_color,
            ..
        }
        | Element::Grid {
            background,
            border_color,
            ..
        } => vec![background, border_color],
        _ => Vec::new(),
    }
}

/// Replace every token NAME in a color slot with the hex the active
/// theme gives it, switching theme at each `theme:` scope. Runs on
/// every rebuild, before the animation settle.
pub fn resolve(w: &mut World, mut el: Element) -> Element {
    let active = root(w);
    walk(active, &mut el);
    el
}

fn walk(active: &'static Theme, el: &mut Element) {
    let active = match el {
        Element::Themed { theme, .. } => by_name(theme.as_str()).unwrap_or(active),
        _ => active,
    };
    for slot in color_slots(el) {
        if let Some(hex) = active.token_hex(slot.as_str()) {
            *slot = hex;
        }
    }
    if let Some(children) = children_of(el) {
        for c in children {
            walk(active, c);
        }
    }
}

fn children_of(el: &mut Element) -> Option<&mut Vec<Element>> {
    match el {
        Element::Column { children, .. }
        | Element::Row { children, .. }
        | Element::Grid { children, .. }
        | Element::GridCell { children, .. }
        | Element::Anim { children, .. }
        | Element::Semantics { children, .. }
        | Element::Themed { children, .. }
        | Element::ListView { children, .. }
        | Element::ScrollView { children, .. }
        | Element::Modal { children, .. } => Some(children),
        Element::Stack(cs) | Element::HScrollView(cs) | Element::DataTable(cs) => Some(cs),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(color: &str) -> Element {
        let mut el = Element::text("x");
        if let Element::Text { color: c, .. } = &mut el {
            *c = Str::from(color);
        }
        el
    }

    fn themed(name: &str, children: Vec<Element>) -> Element {
        Element::Themed {
            theme: Str::from(name),
            children,
        }
    }

    fn column(children: Vec<Element>) -> Element {
        Element::Column {
            spacing: 0.0,
            padding: 0.0,
            background: Str::new(),
            grow: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::new(),
            children,
        }
    }

    fn color_of(el: &Element) -> String {
        match el {
            Element::Text { color, .. } => color.as_str().to_string(),
            _ => panic!("not a Text"),
        }
    }

    #[test]
    fn tokens_resolve_against_the_root_theme() {
        let mut w = World::default();
        let out = resolve(&mut w, column(vec![text("accent")]));
        let Element::Column { children, .. } = &out else {
            panic!()
        };
        assert_eq!(color_of(&children[0]), "#89b4fa");

        set_light(&mut w, true);
        let out = resolve(&mut w, column(vec![text("accent")]));
        let Element::Column { children, .. } = &out else {
            panic!()
        };
        assert_eq!(color_of(&children[0]), "#1e66f5");
    }

    /// The whole point: one subtree on a different palette.
    #[test]
    fn a_scope_overrides_the_root() {
        let mut w = World::default();
        let out = resolve(
            &mut w,
            column(vec![text("text"), themed("light", vec![text("text")])]),
        );
        let Element::Column { children, .. } = &out else {
            panic!()
        };
        assert_eq!(color_of(&children[0]), "#cdd6f4", "root stays dark");
        let Element::Themed { children: inner, .. } = &children[1] else {
            panic!()
        };
        assert_eq!(color_of(&inner[0]), "#4c4f69", "the scope went light");
    }

    /// `theme:` takes an expression, so the name can be app state —
    /// and app state can hold a typo. An unknown palette keeps the
    /// inherited one rather than blanking the subtree, the same
    /// contract a bad color string has.
    #[test]
    fn an_unknown_palette_keeps_the_inherited_one() {
        let mut w = World::default();
        let out = resolve(&mut w, themed("nonesuch", vec![text("text")]));
        let Element::Themed { children, .. } = &out else {
            panic!()
        };
        assert_eq!(color_of(&children[0]), "#cdd6f4", "still the root's dark");
    }

    #[test]
    fn hex_and_unknown_names_pass_through_untouched() {
        let mut w = World::default();
        let out = resolve(&mut w, column(vec![text("#ff0000"), text("nonesuch")]));
        let Element::Column { children, .. } = &out else {
            panic!()
        };
        assert_eq!(color_of(&children[0]), "#ff0000");
        assert_eq!(color_of(&children[1]), "nonesuch");
    }

    #[test]
    fn alpha_tokens_keep_their_alpha() {
        assert_eq!(DARK.token_hex("textDim").unwrap().as_str(), "#ffffff40");
        assert_eq!(DARK.token_hex("accent").unwrap().as_str(), "#89b4fa");
        assert!(DARK.token_hex("nonesuch").is_none());
    }
}
