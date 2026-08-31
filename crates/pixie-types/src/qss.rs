//! QSS shorthand vocabulary — shape-only lookup shared with codegen.
//!
//! `cute-codegen::qss` owns the lowering (kebab key, value formatter,
//! per-pseudo bucket rendering). The type checker only needs to know
//! "is this key part of the shorthand vocabulary, and if so what
//! value shape is it expecting". Both modules look up the same
//! `shape_for` so the vocabularies stay in sync without a circular
//! dep — codegen pulls this from `cute-types`, the type checker
//! consumes it directly.

/// Coarse shape of values a shorthand key accepts. Keeps the type
/// checker and the codegen formatter aligned without exposing a
/// dependency on Cute's full Type lattice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QssValueShape {
    /// `borderRadius`, `padding*`, `fontSize`, ... Numeric literals
    /// auto-suffix `px`; bare strings (`"32px"`, `"1em"`) pass through.
    Length,
    /// `color`, `background`, `borderColor`, ... String only — Cute
    /// has no first-class `Color` type, so accept the QSS literal
    /// (`"#fff"`, `"rgb(...)"`, `"red"`) verbatim.
    Color,
    /// `fontWeight`. Numeric literals (`500`) emit raw, strings
    /// (`"bold"`) pass through. Distinct from Length because the
    /// number must NOT acquire a `px` suffix.
    Numeric,
    /// `border`, `borderStyle`, `fontFamily`, `fontStyle`. Composite
    /// or enumerated values that Cute can't model as a primitive —
    /// String literal only.
    Str,
    /// `textAlign`. Maps strings to `qproperty-alignment` values.
    Align,
}

impl QssValueShape {
    pub fn render_expected(self) -> &'static str {
        match self {
            QssValueShape::Length | QssValueShape::Numeric => "Int, Float, or String",
            QssValueShape::Color | QssValueShape::Str | QssValueShape::Align => "String",
        }
    }
}

/// Map a Cute camelCase key (optionally with a pseudo-class prefix
/// `hover.`, `pressed.`, ...) to its expected value shape. Returns
/// `None` for keys outside the shorthand vocabulary so the caller
/// can route them through the regular `setX(...)` / parent-class
/// property check.
pub fn shape_for(key: &str) -> Option<QssValueShape> {
    let base = match key.split_once('.') {
        Some((prefix, rest)) => {
            if !is_pseudo_prefix(prefix) {
                return None;
            }
            rest
        }
        None => key,
    };
    Some(match base {
        "color" | "background" | "backgroundColor" | "borderColor" => QssValueShape::Color,
        "borderRadius" | "borderWidth" | "padding" | "paddingLeft" | "paddingRight"
        | "paddingTop" | "paddingBottom" | "margin" | "marginLeft" | "marginRight"
        | "marginTop" | "marginBottom" | "fontSize" => QssValueShape::Length,
        "fontWeight" => QssValueShape::Numeric,
        "border" | "borderStyle" | "fontFamily" | "fontStyle" => QssValueShape::Str,
        "textAlign" => QssValueShape::Align,
        _ => return None,
    })
}

/// Pseudo-class prefixes recognised in dotted keys (`hover.X`).
/// Anything else with a dot (`font.bold`) is left to the regular
/// property path.
pub fn is_pseudo_prefix(s: &str) -> bool {
    matches!(s, "hover" | "pressed" | "focus" | "disabled" | "checked")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_key_lookup() {
        assert_eq!(shape_for("color"), Some(QssValueShape::Color));
        assert_eq!(shape_for("borderRadius"), Some(QssValueShape::Length));
        assert_eq!(shape_for("fontWeight"), Some(QssValueShape::Numeric));
        assert_eq!(shape_for("border"), Some(QssValueShape::Str));
        assert_eq!(shape_for("textAlign"), Some(QssValueShape::Align));
    }

    #[test]
    fn pseudo_prefix_strips_to_base_shape() {
        assert_eq!(shape_for("hover.background"), Some(QssValueShape::Color));
        assert_eq!(
            shape_for("pressed.borderRadius"),
            Some(QssValueShape::Length)
        );
    }

    #[test]
    fn unknown_key_is_none() {
        assert_eq!(shape_for("colour"), None);
        assert_eq!(shape_for("notAThing"), None);
    }

    #[test]
    fn non_pseudo_dotted_is_none() {
        // `font.bold` is a QML-only idiom; not part of the QSS
        // shorthand vocabulary.
        assert_eq!(shape_for("font.bold"), None);
    }
}
