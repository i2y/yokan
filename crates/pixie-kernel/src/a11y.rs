//! Accessibility (§8.36).
//!
//! D8 decided the shape years before there was anything to hang on
//! it: the engine seam carries an AccessKit-shaped node tree, and the
//! depth of v0 support stays open. What was missing was the half that
//! is NOT the platform's — where a role and an accessible name come
//! from. They come from here.
//!
//! Two sources, in that order:
//!
//! - **Derived.** A Button is a button, its label is its name; a
//!   TextField is a text input; a ProgressBar reports its value.
//!   Most of a tree needs no authoring at all, and a language that
//!   made every app spell this out would simply get apps that don't.
//! - **Authored.** `role:` / `label:` are universal riders (the
//!   §8.33 pattern) for what cannot be derived: alt text on an image
//!   or an icon, a Text that is really a heading, a Column that is
//!   really a list.
//!
//! Pure layout containers report NOTHING and hand their children to
//! the nearest ancestor that does. A screen reader announcing "group,
//! group, group" is worse than one that stays quiet, and AccessKit
//! agrees — a node with no role is not reported.

use crate::{Element, Str};

/// The v0 role vocabulary — the subset the 18-element catalog can
/// actually produce, spelled in the language's own words rather than
/// as a re-export of `accesskit::Role` (the engine maps them).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Button,
    Label,
    Heading,
    TextInput,
    Image,
    List,
    ListItem,
    Table,
    Dialog,
    Progress,
    Slider,
    Group,
    CheckBox,
    Switch,
    /// The choosers (§ the chooser contract): a closed dropdown, a
    /// radio row group, a tab strip — AccessKit's `ComboBox` /
    /// `RadioGroup` / `TabList`, spelled in pixie's words.
    ComboBox,
    RadioGroup,
    TabList,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        Some(match s {
            "button" => Role::Button,
            "label" => Role::Label,
            "heading" => Role::Heading,
            "textInput" => Role::TextInput,
            "image" => Role::Image,
            "list" => Role::List,
            "listItem" => Role::ListItem,
            "table" => Role::Table,
            "dialog" => Role::Dialog,
            "progress" => Role::Progress,
            "slider" => Role::Slider,
            "group" => Role::Group,
            "checkbox" => Role::CheckBox,
            "switch" => Role::Switch,
            "comboBox" => Role::ComboBox,
            "radioGroup" => Role::RadioGroup,
            "tabList" => Role::TabList,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Role::Button => "button",
            Role::Label => "label",
            Role::Heading => "heading",
            Role::TextInput => "textInput",
            Role::Image => "image",
            Role::List => "list",
            Role::ListItem => "listItem",
            Role::Table => "table",
            Role::Dialog => "dialog",
            Role::Progress => "progress",
            Role::Slider => "slider",
            Role::Group => "group",
            Role::CheckBox => "checkbox",
            Role::Switch => "switch",
            Role::ComboBox => "comboBox",
            Role::RadioGroup => "radioGroup",
            Role::TabList => "tabList",
        }
    }

    /// Every role in the vocabulary, for the cross-tier table test.
    pub const ALL: &'static [Role] = &[
        Role::Button,
        Role::Label,
        Role::Heading,
        Role::TextInput,
        Role::Image,
        Role::List,
        Role::ListItem,
        Role::Table,
        Role::Dialog,
        Role::Progress,
        Role::Slider,
        Role::Group,
        Role::CheckBox,
        Role::Switch,
        Role::ComboBox,
        Role::RadioGroup,
        Role::TabList,
    ];
}

/// One reported node. Deliberately not `accesskit::Node`: the kernel
/// must be able to compute and DUMP this without a platform, which is
/// what lets the tier gate check an app's accessibility tree the same
/// way it checks its element tree.
#[derive(Debug, PartialEq)]
pub struct Node {
    pub role: Role,
    pub name: Str,
    /// The current value a screen reader reads out — a text field's
    /// contents, a progress bar's fraction. Empty when there is none.
    pub value: Str,
    pub children: Vec<Node>,
}

impl Node {
    pub fn dump(&self) -> String {
        let mut out = self.role.name().to_string();
        if !self.name.as_str().is_empty() {
            out.push_str(&format!(" \"{}\"", self.name));
        }
        if !self.value.as_str().is_empty() {
            out.push_str(&format!(" ={}", self.value));
        }
        if !self.children.is_empty() {
            let inner: Vec<String> = self.children.iter().map(|c| c.dump()).collect();
            out.push_str(&format!("[{}]", inner.join(", ")));
        }
        out
    }
}

/// The engine seam D2/D8 named. One member today, and it is the one
/// D8 called for: an engine receives a frame's accessibility tree.
///
/// GPUI's lower half does not use it — at the pinned rev its
/// accessibility is per-element (`div().id(..).role(..).aria_label(..)`),
/// so the engine reads `role_of` / `name_of` / `value_of` directly
/// while painting. That is the same derivation this tree is built
/// from, so the dump below says exactly what GPUI is being told. A
/// parts-stack engine (D2's mapped fallback: winit + wgpu + text +
/// AccessKit) pushes the whole tree instead, which is why the seam is
/// shaped this way rather than as a bag of per-element getters.
pub trait Engine {
    fn push_accessibility(&mut self, tree: Node);
}

/// The role an element reports on its own, before any `role:` rider.
/// `None` means "not reported" — layout containers and the internal
/// wrappers, whose children rise to the nearest reported ancestor.
pub fn role_of(el: &Element) -> Option<Role> {
    match el {
        Element::Text { .. } => Some(Role::Label),
        Element::Button { .. } => Some(Role::Button),
        Element::TextField { .. } => Some(Role::TextInput),
        Element::Image { .. } | Element::Svg { .. } => Some(Role::Image),
        Element::ListView { .. } => Some(Role::List),
        Element::DataTable(_) => Some(Role::Table),
        // A closed Modal is not a dialog that exists and is hidden —
        // it is not there at all, and announcing it would be a lie.
        Element::Modal { open, .. } => open.then_some(Role::Dialog),
        Element::ProgressBar { .. } | Element::Spinner { .. } => Some(Role::Progress),
        // A chart is a picture as far as assistive technology goes.
        // Its data is not readable without a `label:` — which is the
        // honest report, not a gap to paper over.
        Element::BarChart { .. } | Element::LineChart { .. } => Some(Role::Image),
        Element::Checkbox { .. } => Some(Role::CheckBox),
        Element::Switch { .. } => Some(Role::Switch),
        Element::Slider { .. } => Some(Role::Slider),
        Element::Select { .. } => Some(Role::ComboBox),
        Element::RadioGroup { .. } => Some(Role::RadioGroup),
        Element::TabBar { .. } => Some(Role::TabList),
        _ => None,
    }
}

/// The accessible name an element carries on its own.
pub fn name_of(el: &Element) -> Str {
    match el {
        Element::Text { text, .. } => text.clone(),
        Element::Button { label, .. } => label.clone(),
        // The placeholder is the field's NAME ("Search"); what the
        // user typed is its value, below.
        Element::TextField { placeholder, .. } => placeholder.clone(),
        Element::Checkbox { label, .. } | Element::Switch { label, .. } => label.clone(),
        // A chooser derives its name from the current choice's text —
        // the one piece of it a screen reader can usefully lead with.
        // Out of range (or empty) derives nothing, honestly.
        Element::Select {
            options, selected, ..
        }
        | Element::RadioGroup {
            options, selected, ..
        } => options.get(*selected).unwrap_or_else(Str::new),
        Element::TabBar { labels, active, .. } => labels.get(*active).unwrap_or_else(Str::new),
        _ => Str::new(),
    }
}

/// The value read out after the name, for elements that have one.
pub fn value_of(el: &Element) -> Str {
    match el {
        Element::TextField { value, .. } => value.clone(),
        Element::ProgressBar { value } => Str::from(format!("{value}")),
        Element::Checkbox { checked, .. } | Element::Switch { checked, .. } => {
            Str::from(format!("{checked}"))
        }
        // No name to derive (a Slider has no label prop) — but the
        // number a screen reader reads out is this.
        Element::Slider { value, .. } => Str::from(format!("{value}")),
        _ => Str::new(),
    }
}

/// Whether an element is a semantic wrapper — the authored riders.
fn authored(el: &Element) -> Option<(&Str, &Str, &Vec<Element>)> {
    match el {
        Element::Semantics {
            role,
            label,
            children,
        } => Some((role, label, children)),
        _ => None,
    }
}

/// The reported nodes for one subtree. A container with no role of
/// its own contributes its children directly, so the tree stays as
/// shallow as the app's meaning actually is.
pub fn nodes(el: &Element) -> Vec<Node> {
    if let Some((role, label, children)) = authored(el) {
        let Some(inner) = children.first() else {
            return Vec::new();
        };
        // The rider overrides what the wrapped element would have
        // said; anything it leaves out falls back to the derivation,
        // so `label:` alone on a Button keeps role `button`.
        // Through any animation/theme wrapper between the riders and
        // the element they describe.
        let inner = inner.inner();
        let derived = role_of(inner);
        let role = match Role::parse(role.as_str()) {
            Some(r) => Some(r),
            None => derived,
        };
        let name = if label.as_str().is_empty() {
            name_of(inner)
        } else {
            label.clone()
        };
        let Some(role) = role else {
            // Riders on a pure container: the label names the group.
            return vec![Node {
                role: Role::Group,
                name,
                value: Str::new(),
                children: child_nodes(inner),
            }];
        };
        return vec![Node {
            role,
            name,
            value: value_of(inner),
            children: child_nodes(inner),
        }];
    }
    match role_of(el) {
        Some(role) => vec![Node {
            role,
            name: name_of(el),
            value: value_of(el),
            children: child_nodes(el),
        }],
        None => child_nodes(el),
    }
}

/// The reported nodes UNDER an element, skipping the element itself.
fn child_nodes(el: &Element) -> Vec<Node> {
    let mut out = Vec::new();
    for c in children_of(el) {
        out.extend(nodes(c));
    }
    out
}

/// This frame's whole tree, rooted at a window-level group.
pub fn tree(el: &Element) -> Node {
    Node {
        role: Role::Group,
        name: Str::new(),
        value: Str::new(),
        children: nodes(el),
    }
}

/// The one child list of a container. Lazy ListView rows are absent
/// for the same reason the animation pass skips them: materializing
/// a virtualized list to describe it defeats the virtualization. A
/// lazy list still reports as a `list` — with no items until the
/// engine's own per-element path walks the visible ones.
fn children_of(el: &Element) -> &[Element] {
    match el {
        Element::Column { children, .. }
        | Element::Row { children, .. }
        | Element::Grid { children, .. }
        | Element::GridCell { children, .. }
        | Element::Anim { children, .. }
        | Element::Semantics { children, .. }
        | Element::Tooltip { children, .. }
        | Element::Themed { children, .. }
        | Element::ListView { children, .. }
        | Element::ScrollView { children, .. }
        | Element::Modal { children, .. } => children,
        Element::Stack(cs) | Element::HScrollView(cs) | Element::DataTable(cs) => cs,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    fn text(s: &str) -> Element {
        Element::Text {
            text: Str::from(s),
            font_size: 0.0,
            color: Str::new(),
            align: Str::new(),
            grow: 0.0,
        }
    }

    fn button(s: &str) -> Element {
        Element::Button {
            label: Str::from(s),
            background: Str::new(),
            hover_background: Str::new(),
            active_background: Str::new(),
            width: 0.0,
            height: 0.0,
            font_size: 0.0,
            color: Str::new(),
            grow: 0.0,
            basis: 0.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: Str::new(),
            on_click: Rc::new(|_| {}),
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

    fn semantics(role: &str, label: &str, child: Element) -> Element {
        Element::Semantics {
            role: Str::from(role),
            label: Str::from(label),
            children: vec![child],
        }
    }

    /// Layout is not meaning: a Column reports nothing and its
    /// children rise to the window.
    #[test]
    fn containers_do_not_announce_themselves() {
        let t = tree(&column(vec![column(vec![button("save"), text("hi")])]));
        assert_eq!(t.dump(), r#"group[button "save", label "hi"]"#);
    }

    #[test]
    fn a_progress_bar_reports_its_value() {
        let t = tree(&column(vec![Element::ProgressBar { value: 0.25 }]));
        assert_eq!(t.dump(), "group[progress =0.25]");
    }

    /// An icon has no name to derive — `label:` is the only way it
    /// ever gets one.
    #[test]
    fn an_image_is_nameless_until_it_is_labelled() {
        let img = Element::Svg {
            source: Str::from("save.svg"),
            width: 0.0,
            height: 0.0,
        };
        assert_eq!(tree(&column(vec![img])).dump(), "group[image]");

        let img = Element::Svg {
            source: Str::from("save.svg"),
            width: 0.0,
            height: 0.0,
        };
        let t = tree(&column(vec![semantics("", "Save", img)]));
        assert_eq!(t.dump(), r#"group[image "Save"]"#);
    }

    /// `role:` alone re-labels the kind and keeps the derived name.
    #[test]
    fn a_role_rider_overrides_only_the_role() {
        let t = tree(&column(vec![semantics("heading", "", text("Reports"))]));
        assert_eq!(t.dump(), r#"group[heading "Reports"]"#);
    }

    /// Riders on a pure container name a group rather than vanishing.
    #[test]
    fn a_rider_on_a_container_names_a_group() {
        let t = tree(&column(vec![semantics(
            "",
            "toolbar",
            column(vec![button("cut")]),
        )]));
        assert_eq!(t.dump(), r#"group[group "toolbar"[button "cut"]]"#);
    }

    /// A closed Modal is absent, not hidden.
    #[test]
    fn a_closed_modal_is_not_a_dialog() {
        let shut = Element::Modal {
            open: false,
            children: vec![text("body")],
        };
        assert_eq!(tree(&shut).dump(), r#"group[label "body"]"#);
        let open = Element::Modal {
            open: true,
            children: vec![text("body")],
        };
        assert_eq!(tree(&open).dump(), r#"group[dialog[label "body"]]"#);
    }

    #[test]
    fn every_role_round_trips_through_its_name() {
        for r in Role::ALL {
            assert_eq!(Role::parse(r.name()), Some(*r));
        }
        assert_eq!(Role::parse("nonesuch"), None);
    }

    /// A toggle's label is its name and its checked state is its
    /// value — both derived, no rider needed.
    #[test]
    fn toggles_report_their_label_and_state() {
        let t = tree(&column(vec![
            Element::Checkbox {
                label: Str::from("Dark mode"),
                checked: false,
                on_toggle: None,
            },
            Element::Switch {
                label: Str::from("Wi-Fi"),
                checked: true,
                on_toggle: None,
            },
        ]));
        assert_eq!(
            t.dump(),
            r#"group[checkbox "Dark mode" =false, switch "Wi-Fi" =true]"#
        );
    }
}
