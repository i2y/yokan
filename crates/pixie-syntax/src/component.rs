//! Component splice — user-defined view components (§8.29).
//!
//! Every `view` other than the root (`Main`, or the sole view) is a
//! COMPONENT: a named, parameterized view fragment usable as an
//! element wherever catalog elements go. The splice inlines each use
//! site at compile time — like the style splice, ONE implementation
//! feeds both execution tiers, the engine vocabulary never grows, and
//! rung-2 hot reload rides the existing machinery.
//!
//! ```text
//!     view Card(title: String) {
//!       state open : Bool = false
//!       Column {
//!         Text { text: title }
//!         Button { text: "toggle"; onClick: { open = !open } }
//!         if open {
//!           Slot { }          // ← use-site children splice in here
//!         }
//!       }
//!     }
//!
//!     view Main {
//!       Column {
//!         Card { title: "a"; Text { text: "body a" } }
//!         Card { title: "b"; Text { text: "body b" } }
//!       }
//!     }
//! ```
//!
//! Semantics:
//! - Use-site properties bind the declared params (defaults honored);
//!   the value EXPRESSIONS substitute into the body, so anything an
//!   element property accepts works as an argument.
//! - `state` / `let` fields in a component hoist into the root view's
//!   field list under a per-instance name (`__c0_open`, `__c1_open`,
//!   …) — every use site gets independent state, the Flutter
//!   StatefulWidget shape. Adding/removing a stateful use site
//!   changes the hoisted set, which the fingerprint sees (state
//!   edits classify rung 1, body edits stay rung 2).
//! - One `Slot { }` per component marks where use-site children land.
//! - Components expand recursively; cycles are an error. Stateful
//!   components inside `for` repeaters are gated (per-item state is
//!   M2).

use std::collections::HashMap;

use crate::ast::{
    Block, CaseArm, Element, ElementMember, Expr, ExprKind, Ident, Item, Module, Pattern,
    StateField, StateFieldKind, Stmt, StrPart, UseKind, ViewDecl,
};
use crate::span::Span;

#[derive(Debug)]
pub struct ComponentError {
    pub span: Span,
    pub message: String,
}

fn err<T>(span: Span, message: impl Into<String>) -> Result<T, ComponentError> {
    Err(ComponentError {
        span,
        message: message.into(),
    })
}

/// The engine catalog — a component may not shadow these (mirrors the
/// `lower_element` vocabulary in pixie-codegen; grows with it).
const CATALOG: &[&str] = &[
    "Column", "Row", "Stack", "Text", "Button", "TextField", "ListView", "ScrollView",
    "HScrollView", "Image", "Svg", "DataTable", "Modal", "BarChart", "LineChart", "ProgressBar",
    "Spinner", "Checkbox", "Switch", "Slider", "Slot",
];

/// Sanity checks every component declaration must pass regardless of
/// where it is used from.
fn check_decl(v: &ViewDecl) -> Result<(), ComponentError> {
    if CATALOG.contains(&v.name.name.as_str()) {
        return err(
            v.span,
            format!("component `{}` shadows a built-in element", v.name.name),
        );
    }
    let mut seen: Vec<&str> = Vec::new();
    for p in &v.params {
        if seen.contains(&p.name.name.as_str()) {
            return err(
                v.span,
                format!("component `{}` declares `{}` twice", v.name.name, p.name.name),
            );
        }
        seen.push(&p.name.name);
    }
    for sf in &v.state_fields {
        if seen.contains(&sf.name.name.as_str()) {
            return err(
                sf.span,
                format!(
                    "`{}` names both a param and a state field of component `{}`",
                    sf.name.name, v.name.name
                ),
            );
        }
        seen.push(&sf.name.name);
    }
    Ok(())
}

/// Single-module form — the reload path's entry splice with no
/// imports, and the unit-test surface.
pub fn splice_module(module: &Module) -> Result<Module, ComponentError> {
    splice_module_with(module, &[], true)
}

/// Inline every component use site into the root view and drop the
/// component declarations. `foreign` carries the views of each
/// directly imported module (§8.29 cross-module leg); `entry` marks
/// the program's entry module — a dependency module has no root, so
/// ALL its views are components (consumed by importers) and are
/// dropped here.
pub fn splice_module_with(
    module: &Module,
    foreign: &[(String, Vec<ViewDecl>)],
    entry: bool,
) -> Result<Module, ComponentError> {
    let views: Vec<&ViewDecl> = module
        .items
        .iter()
        .filter_map(|i| match i {
            Item::View(v) => Some(v),
            _ => None,
        })
        .collect();
    let has_main = views.iter().any(|v| v.name.name == "Main");
    // The mounted root: `Main`, or (entry only) a sole view of any
    // name — the pre-component compatibility shape.
    let root_name: Option<String> = if has_main {
        Some("Main".to_string())
    } else if entry && views.len() == 1 {
        Some(views[0].name.name.clone())
    } else if entry && views.len() > 1 {
        return err(
            views[1].span,
            "multiple `view`s need a root named `Main` — the others become components",
        );
    } else {
        None
    };
    // No fast path: even a single-view module walks its root — the
    // walk is what validates stray `Slot`s and qualified element
    // names, and a clone-walk of a component-less tree is cheap.

    // Local components: every view that is not the root.
    let mut local: HashMap<String, ViewDecl> = HashMap::new();
    for v in &views {
        if Some(&v.name.name) == root_name.as_ref() {
            continue;
        }
        check_decl(v)?;
        if local.insert(v.name.name.clone(), (*v).clone()).is_some() {
            return err(v.span, format!("component `{}` is declared twice", v.name.name));
        }
    }

    // Import resolution from the module's own `use` items:
    // whole-module imports map a qualifier to a module; selective
    // imports bind a bare local name to (module, original) when the
    // target module has a pub view of that name.
    let mut spaces: HashMap<String, Vec<ViewDecl>> = HashMap::new();
    for (name, vs) in foreign {
        for v in vs {
            check_decl(v)?;
        }
        spaces.insert(name.clone(), vs.clone());
    }
    let mut qualifiers: HashMap<String, String> = HashMap::new();
    let mut selective: HashMap<String, (String, String)> = HashMap::new();
    for item in &module.items {
        let Item::Use(u) = item else { continue };
        let target = u
            .path
            .iter()
            .map(|i| i.name.clone())
            .collect::<Vec<_>>()
            .join(".");
        match &u.kind {
            UseKind::Module(alias) => {
                let q = alias
                    .as_ref()
                    .map(|a| a.name.clone())
                    .unwrap_or_else(|| u.path.last().expect("use path").name.clone());
                qualifiers.insert(q, target.clone());
            }
            UseKind::Names(names) => {
                for n in names {
                    let is_view = spaces
                        .get(&target)
                        .is_some_and(|vs| vs.iter().any(|v| v.name.name == n.name.name && v.is_pub));
                    if !is_view {
                        continue; // classes/fns — the module system's business
                    }
                    let local_name = n.alias.as_ref().unwrap_or(&n.name).name.clone();
                    if local.contains_key(&local_name) {
                        return err(
                            n.name.span,
                            format!(
                                "imported component `{local_name}` collides with a local `view {local_name}` — rename one"
                            ),
                        );
                    }
                    selective.insert(local_name, (target.clone(), n.name.name.clone()));
                }
            }
        }
    }

    let mut ex = Expander {
        local,
        spaces,
        qualifiers,
        selective,
        home: Vec::new(),
        counter: 0,
        stack: Vec::new(),
        hoisted: Vec::new(),
        repeater_depth: 0,
        repeater_iter: Vec::new(),
    };
    let mut new_items = Vec::with_capacity(module.items.len());
    for item in &module.items {
        match item {
            Item::View(v) if Some(&v.name.name) == root_name.as_ref() => {
                if !v.params.is_empty() {
                    return err(v.span, "the root view takes no parameters");
                }
                let root = ex.expand_element(&v.root)?;
                let mut state_fields = v.state_fields.clone();
                state_fields.append(&mut ex.hoisted);
                new_items.push(Item::View(ViewDecl {
                    root,
                    state_fields,
                    ..v.clone()
                }));
            }
            Item::View(_) => {} // component declarations are consumed
            other => new_items.push(other.clone()),
        }
    }
    Ok(Module {
        items: new_items,
        span: module.span,
    })
}

struct Expander {
    /// The splicing module's own components (root excluded).
    local: HashMap<String, ViewDecl>,
    /// Imported modules' views, by module name.
    spaces: HashMap<String, Vec<ViewDecl>>,
    /// Whole-import qualifier → module name (`use ui as U` → U → ui).
    qualifiers: HashMap<String, String>,
    /// Selectively imported view name → (module, original name).
    selective: HashMap<String, (String, String)>,
    /// Namespace stack: empty = the splicing module; a pushed module
    /// name means we are inside a foreign component's body, where
    /// bare names resolve against that module's OWN views.
    home: Vec<String>,
    /// Per-instance id source — pre-order, deterministic.
    counter: usize,
    /// Expansion chain for cycle detection, keyed (module, name).
    stack: Vec<(String, String)>,
    /// State fields lifted out of component instances, destined for
    /// the root view's field list.
    hoisted: Vec<StateField>,
    /// Depth of surrounding `for` repeaters — nonzero means a
    /// stateful instance goes per-row instead of per-view.
    repeater_depth: usize,
    /// The iter expression of each surrounding repeater, outermost
    /// first. A stateful instance records ALL of them in its row-seat
    /// marker: `prepare` sizes one seat dimension per entry, and the
    /// fingerprint makes an iter edit a rung-1 rebuild.
    repeater_iter: Vec<Expr>,
}

impl Expander {
    /// Resolve an element name to a component declaration, honoring
    /// the current namespace. Returns (home module, decl).
    fn resolve(&self, e: &Element) -> Result<Option<(String, ViewDecl)>, ComponentError> {
        let inside_foreign = !self.home.is_empty();
        if let Some(q) = e.module_path.first() {
            if e.module_path.len() > 1 {
                return err(e.span, "nested module qualifiers on elements are M2");
            }
            if inside_foreign {
                return err(
                    e.span,
                    "a component's body can reference its own module's components only \
                     (qualified use inside a `pub view` is M2)",
                );
            }
            let Some(module) = self.qualifiers.get(&q.name) else {
                return err(
                    q.span,
                    format!("`{}` is not an imported module (add `use {}`)", q.name, q.name),
                );
            };
            let Some(vs) = self.spaces.get(module) else {
                return err(q.span, format!("module `{module}` has no views"));
            };
            let Some(v) = vs.iter().find(|v| v.name.name == e.name.name) else {
                return err(
                    e.span,
                    format!("module `{module}` has no `view {}`", e.name.name),
                );
            };
            if !v.is_pub {
                return err(
                    e.span,
                    format!("`view {}` in `{module}` is private — mark it `pub view`", e.name.name),
                );
            }
            return Ok(Some((module.clone(), v.clone())));
        }
        if inside_foreign {
            let module = self.home.last().expect("inside foreign").clone();
            let hit = self
                .spaces
                .get(&module)
                .and_then(|vs| vs.iter().find(|v| v.name.name == e.name.name))
                .cloned();
            return Ok(hit.map(|v| (module, v)));
        }
        if let Some(v) = self.local.get(&e.name.name) {
            return Ok(Some((String::new(), v.clone())));
        }
        if let Some((module, orig)) = self.selective.get(&e.name.name) {
            let v = self
                .spaces
                .get(module)
                .and_then(|vs| vs.iter().find(|v| v.name.name == *orig))
                .cloned()
                .ok_or_else(|| ComponentError {
                    span: e.span,
                    message: format!("module `{module}` has no `view {orig}`"),
                })?;
            return Ok(Some((module.clone(), v)));
        }
        Ok(None)
    }

    fn expand_element(&mut self, e: &Element) -> Result<Element, ComponentError> {
        if e.name.name == "Slot" {
            return err(e.span, "`Slot` is only meaningful inside a component body");
        }
        if let Some((module, decl)) = self.resolve(e)? {
            return self.expand_component(e, &module, &decl);
        }
        if !e.module_path.is_empty() {
            // resolve() only returns None for bare names.
            unreachable!("qualified elements resolve or error");
        }
        let mut members = Vec::with_capacity(e.members.len());
        for m in &e.members {
            members.push(self.expand_member(m)?);
        }
        Ok(Element {
            module_path: e.module_path.clone(),
            name: e.name.clone(),
            members,
            span: e.span,
        })
    }

    fn expand_member(&mut self, m: &ElementMember) -> Result<ElementMember, ComponentError> {
        Ok(match m {
            ElementMember::Child(c) => ElementMember::Child(self.expand_element(c)?),
            ElementMember::Property { key, value, span } => ElementMember::Property {
                key: key.clone(),
                value: self.expand_expr(value)?,
                span: *span,
            },
            ElementMember::Stmt(s) => ElementMember::Stmt(self.expand_stmt(s)?),
        })
    }

    fn expand_stmt(&mut self, s: &Stmt) -> Result<Stmt, ComponentError> {
        Ok(match s {
            Stmt::For {
                binding,
                index,
                iter,
                body,
                span,
            } => {
                self.repeater_depth += 1;
                self.repeater_iter.push(iter.clone());
                let body = self.expand_block(body);
                self.repeater_iter.pop();
                self.repeater_depth -= 1;
                Stmt::For {
                    binding: binding.clone(),
                    index: index.clone(),
                    iter: iter.clone(),
                    body: body?,
                    span: *span,
                }
            }
            Stmt::Expr(e) => Stmt::Expr(self.expand_expr(e)?),
            other => other.clone(),
        })
    }

    fn expand_block(&mut self, b: &Block) -> Result<Block, ComponentError> {
        let mut stmts = Vec::with_capacity(b.stmts.len());
        for s in &b.stmts {
            stmts.push(self.expand_stmt(s)?);
        }
        let trailing = match &b.trailing {
            Some(t) => Some(Box::new(self.expand_expr(t)?)),
            None => None,
        };
        Ok(Block {
            stmts,
            trailing,
            span: b.span,
        })
    }

    fn expand_expr(&mut self, e: &Expr) -> Result<Expr, ComponentError> {
        Ok(match &e.kind {
            ExprKind::Element(el) => Expr {
                kind: ExprKind::Element(self.expand_element(el)?),
                span: e.span,
            },
            ExprKind::If {
                cond,
                then_b,
                else_b,
                let_binding,
            } => Expr {
                kind: ExprKind::If {
                    cond: cond.clone(),
                    then_b: self.expand_block(then_b)?,
                    else_b: match else_b {
                        Some(b) => Some(self.expand_block(b)?),
                        None => None,
                    },
                    let_binding: let_binding.clone(),
                },
                span: e.span,
            },
            ExprKind::Case { scrutinee, arms } => {
                let mut new_arms = Vec::with_capacity(arms.len());
                for a in arms {
                    new_arms.push(CaseArm {
                        pattern: a.pattern.clone(),
                        body: self.expand_block(&a.body)?,
                        span: a.span,
                    });
                }
                Expr {
                    kind: ExprKind::Case {
                        scrutinee: scrutinee.clone(),
                        arms: new_arms,
                    },
                    span: e.span,
                }
            }
            ExprKind::Block(b) => Expr {
                kind: ExprKind::Block(self.expand_block(b)?),
                span: e.span,
            },
            _ => e.clone(),
        })
    }

    fn expand_component(
        &mut self,
        use_site: &Element,
        module: &str,
        decl: &ViewDecl,
    ) -> Result<Element, ComponentError> {
        let name = decl.name.name.clone();
        let key = (module.to_string(), name.clone());
        if self.stack.contains(&key) {
            let mut chain = self
                .stack
                .iter()
                .map(|(_, n)| n.as_str())
                .collect::<Vec<_>>()
                .join(" → ");
            chain.push_str(" → ");
            chain.push_str(&name);
            return err(use_site.span, format!("component cycle: {chain}"));
        }

        // Split the use site: properties bind params, everything in
        // element position is `Slot` content.
        let mut props: HashMap<String, (Expr, Span)> = HashMap::new();
        let mut children: Vec<ElementMember> = Vec::new();
        for m in &use_site.members {
            match m {
                ElementMember::Property { key, value, span } => {
                    if !decl.params.iter().any(|p| p.name.name == *key) {
                        let have: Vec<&str> =
                            decl.params.iter().map(|p| p.name.name.as_str()).collect();
                        return err(
                            *span,
                            if have.is_empty() {
                                format!("component `{name}` takes no properties (got `{key}:`)")
                            } else {
                                format!(
                                    "component `{name}` has no `{key}:` (params: {})",
                                    have.join(", ")
                                )
                            },
                        );
                    }
                    if props.insert(key.clone(), (value.clone(), *span)).is_some() {
                        return err(*span, format!("`{key}:` given twice"));
                    }
                }
                other => children.push(other.clone()),
            }
        }

        // Bind every declared param: explicit argument or default.
        let mut subst: HashMap<String, Expr> = HashMap::new();
        for p in &decl.params {
            match props.remove(&p.name.name) {
                Some((v, _)) => {
                    subst.insert(p.name.name.clone(), v);
                }
                None => match &p.default {
                    Some(d) => {
                        subst.insert(p.name.name.clone(), d.clone());
                    }
                    None => {
                        return err(
                            use_site.span,
                            format!("component `{name}` needs `{}:`", p.name.name),
                        )
                    }
                },
            }
        }

        // Per-instance state: hoist under a fresh name, and rewrite
        // body references through the same substitution map. Inside a
        // `for` repeater, state goes PER ROW (§8.30): a row-seat
        // marker is hoisted instead and the instance is wrapped in a
        // `__PixieRowScope` both lowerers resolve at build time.
        let id = self.counter;
        self.counter += 1;
        let desugared_holder: Option<String> = decl.state_fields.iter().find_map(|sf| {
            if sf.name.name != "__pixie_state" {
                return None;
            }
            match &sf.init_expr.kind {
                ExprKind::Call { callee, .. } => match &callee.kind {
                    ExprKind::Ident(c) => Some(c.clone()),
                    _ => None,
                },
                _ => None,
            }
        });
        let has_cells = decl
            .state_fields
            .iter()
            .any(|sf| matches!(sf.kind, StateFieldKind::Property { .. }));
        let has_user_objects = decl.state_fields.iter().any(|sf| {
            matches!(sf.kind, StateFieldKind::Object) && sf.name.name != "__pixie_state"
        });
        let stateful = has_cells || desugared_holder.is_some();
        let mut row_mode = false;
        if self.repeater_depth > 0 {
            if has_user_objects {
                return err(
                    use_site.span,
                    format!(
                        "component `{name}` holds `let` object state — per-row object \
                         graphs inside a repeater are M2"
                    ),
                );
            }
            if stateful {
                // Nested repeaters and lazy (virtualized) rows both
                // work now (§8.34): the seat is keyed by the whole
                // index PATH of the enclosing repeaters, and every
                // driving list's length is known in `prepare`.
                row_mode = true;
            }
        }
        // Two shapes reach here. In the compiled pipeline the driver's
        // view-state desugar ALREADY ran: `state` cells became a
        // holder class + one Object field `__pixie_state`, refs
        // rewritten to members — hoisting just renames the field. In
        // the reload pipeline the parse is raw: perform the same
        // shape here (holder field named identically, cell refs →
        // member reads) so both tiers agree on every name. The
        // holder's init expr is only meaningful compiled-side; the
        // interpreter resolves fields from the binary's registration.
        let cells: Vec<&StateField> = decl
            .state_fields
            .iter()
            .filter(|sf| matches!(sf.kind, StateFieldKind::Property { .. }))
            .collect();
        if row_mode {
            let holder_class = desugared_holder
                .clone()
                .unwrap_or_else(|| format!("__{}State", name));
            let seat = format!("__c{id}___pixie_rows");
            let row_local = format!("__c{id}___pixie_row");
            for sf in &cells {
                subst.insert(
                    sf.name.name.clone(),
                    Expr {
                        kind: ExprKind::Member {
                            receiver: Box::new(Expr {
                                kind: ExprKind::Ident(row_local.clone()),
                                span: sf.span,
                            }),
                            name: sf.name.clone(),
                        },
                        span: sf.span,
                    },
                );
            }
            if desugared_holder.is_some() {
                subst.insert(
                    "__pixie_state".to_string(),
                    Expr {
                        kind: ExprKind::Ident(row_local.clone()),
                        span: decl.span,
                    },
                );
            }
            // Every enclosing repeater's iter, outermost first: the
            // emitter sizes one seat dimension per entry, and the
            // fingerprint hashes them, so redirecting ANY of the
            // lists is a rung-1 rebuild.
            let mut marker_args = vec![Expr {
                kind: ExprKind::Ident(holder_class.clone()),
                span: decl.span,
            }];
            marker_args.extend(self.repeater_iter.iter().cloned());
            let depth = self.repeater_iter.len();
            self.hoisted.push(StateField {
                name: Ident {
                    name: seat.clone(),
                    span: decl.span,
                },
                kind: StateFieldKind::Object,
                init_expr: Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            kind: ExprKind::Ident("__pixie_row_seat".to_string()),
                            span: decl.span,
                        }),
                        args: marker_args,
                        block: None,
                        type_args: Vec::new(),
                    },
                    span: decl.span,
                },
                span: decl.span,
            });
            // Expand the body with the row substitutions, then wrap.
            let mut expanded_children: Vec<ElementMember> = Vec::with_capacity(children.len());
            for c in &children {
                expanded_children.push(self.expand_member(c)?);
            }
            let mut body = subst_element(&decl.root, &subst, &name)?;
            if body.name.name == "Slot" {
                return err(decl.root.span, format!("component `{name}` cannot be just a `Slot`"));
            }
            let slots = splice_slot(&mut body, &mut Some(expanded_children), &name)?;
            if slots == 0 && !children.is_empty() {
                return err(
                    use_site.span,
                    format!("component `{name}` takes no children (its body has no `Slot`)"),
                );
            }
            self.stack.push(key);
            let pushed_home = !module.is_empty();
            if pushed_home {
                self.home.push(module.to_string());
            }
            let out = self.expand_element(&body);
            if pushed_home {
                self.home.pop();
            }
            self.stack.pop();
            let out = out?;
            let mk_str = |s: &str, span| Expr {
                kind: ExprKind::Str(vec![StrPart::Text(s.to_string())]),
                span,
            };
            return Ok(Element {
                module_path: Vec::new(),
                name: Ident {
                    name: "__PixieRowScope".to_string(),
                    span: use_site.span,
                },
                members: vec![
                    ElementMember::Property {
                        key: "__seat".to_string(),
                        value: mk_str(&seat, use_site.span),
                        span: use_site.span,
                    },
                    ElementMember::Property {
                        key: "__row".to_string(),
                        value: mk_str(&row_local, use_site.span),
                        span: use_site.span,
                    },
                    ElementMember::Property {
                        key: "__holder".to_string(),
                        value: mk_str(&holder_class, use_site.span),
                        span: use_site.span,
                    },
                    ElementMember::Property {
                        key: "__depth".to_string(),
                        value: mk_str(&depth.to_string(), use_site.span),
                        span: use_site.span,
                    },
                    ElementMember::Child(out),
                ],
                span: use_site.span,
            });
        }
        if !cells.is_empty() {
            let holder = format!("__c{id}___pixie_state");
            for sf in &cells {
                subst.insert(
                    sf.name.name.clone(),
                    Expr {
                        kind: ExprKind::Member {
                            receiver: Box::new(Expr {
                                kind: ExprKind::Ident(holder.clone()),
                                span: sf.span,
                            }),
                            name: sf.name.clone(),
                        },
                        span: sf.span,
                    },
                );
            }
            self.hoisted.push(StateField {
                name: Ident {
                    name: holder,
                    span: decl.span,
                },
                kind: StateFieldKind::Object,
                init_expr: Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            kind: ExprKind::Ident(format!("__{}State", name)),
                            span: decl.span,
                        }),
                        args: Vec::new(),
                        block: None,
                        type_args: Vec::new(),
                    },
                    span: decl.span,
                },
                span: decl.span,
            });
        }
        for sf in &decl.state_fields {
            if matches!(sf.kind, StateFieldKind::Property { .. }) {
                continue;
            }
            let hoisted_name = format!("__c{id}_{}", sf.name.name);
            subst.insert(
                sf.name.name.clone(),
                Expr {
                    kind: ExprKind::Ident(hoisted_name.clone()),
                    span: sf.span,
                },
            );
            self.hoisted.push(StateField {
                name: Ident {
                    name: hoisted_name,
                    span: sf.name.span,
                },
                kind: sf.kind.clone(),
                init_expr: subst_expr(&sf.init_expr, &subst, &name)?,
                span: sf.span,
            });
        }

        // Use-site children belong to the CALLER's namespace — expand
        // them here, before switching into the component's own module
        // for its body (a foreign Card can slot an entry-local
        // Counter). Then substitute, splice the expanded content into
        // `Slot`, and expand what the body itself uses.
        let mut expanded_children: Vec<ElementMember> = Vec::with_capacity(children.len());
        for c in &children {
            expanded_children.push(self.expand_member(c)?);
        }
        let mut body = subst_element(&decl.root, &subst, &name)?;
        if body.name.name == "Slot" {
            return err(decl.root.span, format!("component `{name}` cannot be just a `Slot`"));
        }
        let slots = splice_slot(&mut body, &mut Some(expanded_children), &name)?;
        if slots == 0 && !children.is_empty() {
            return err(
                use_site.span,
                format!("component `{name}` takes no children (its body has no `Slot`)"),
            );
        }
        self.stack.push(key);
        let pushed_home = !module.is_empty();
        if pushed_home {
            self.home.push(module.to_string());
        }
        let out = self.expand_element(&body);
        if pushed_home {
            self.home.pop();
        }
        self.stack.pop();
        out
    }
}

/// Replace the single `Slot { }` child in a component body with the
/// use-site children. Returns how many slots were found (0 or 1;
/// a second one errors).
fn splice_slot(
    e: &mut Element,
    content: &mut Option<Vec<ElementMember>>,
    comp: &str,
) -> Result<usize, ComponentError> {
    let mut found = 0;
    let mut members: Vec<ElementMember> = Vec::new();
    for m in e.members.drain(..) {
        match m {
            ElementMember::Child(c) if c.name.name == "Slot" => {
                if !c.members.is_empty() {
                    return err(c.span, "`Slot { }` takes nothing inside");
                }
                found += 1;
                if found > 1 {
                    return err(c.span, format!("component `{comp}` has more than one `Slot`"));
                }
                members.extend(content.take().expect("slot content taken once"));
            }
            ElementMember::Child(mut c) => {
                found += splice_slot_guard(&mut c, content, comp, found)?;
                members.push(ElementMember::Child(c));
            }
            ElementMember::Stmt(mut s) => {
                found += slot_in_stmt(&mut s, content, comp, found)?;
                members.push(ElementMember::Stmt(s));
            }
            other => members.push(other),
        }
    }
    e.members = members;
    Ok(found)
}

fn splice_slot_guard(
    c: &mut Element,
    content: &mut Option<Vec<ElementMember>>,
    comp: &str,
    already: usize,
) -> Result<usize, ComponentError> {
    let found = splice_slot(c, content, comp)?;
    if already + found > 1 {
        return err(c.span, format!("component `{comp}` has more than one `Slot`"));
    }
    Ok(found)
}

fn slot_in_stmt(
    s: &mut Stmt,
    content: &mut Option<Vec<ElementMember>>,
    comp: &str,
    already: usize,
) -> Result<usize, ComponentError> {
    Ok(match s {
        Stmt::Expr(e) => slot_in_expr(e, content, comp, already)?,
        Stmt::For { body, .. } => slot_in_block(body, content, comp, already)?,
        _ => 0,
    })
}

fn slot_in_expr(
    e: &mut Expr,
    content: &mut Option<Vec<ElementMember>>,
    comp: &str,
    already: usize,
) -> Result<usize, ComponentError> {
    Ok(match &mut e.kind {
        ExprKind::Element(el) => {
            if el.name.name == "Slot" {
                return err(
                    el.span,
                    "`Slot` must sit in element position inside a container",
                );
            }
            splice_slot_guard(el, content, comp, already)?
        }
        ExprKind::If { then_b, else_b, .. } => {
            let mut n = slot_in_block(then_b, content, comp, already)?;
            if let Some(b) = else_b {
                n += slot_in_block(b, content, comp, already + n)?;
            }
            n
        }
        ExprKind::Case { arms, .. } => {
            let mut n = 0;
            for a in arms.iter_mut() {
                n += slot_in_block(&mut a.body, content, comp, already + n)?;
            }
            n
        }
        _ => 0,
    })
}

fn slot_in_block(
    b: &mut Block,
    content: &mut Option<Vec<ElementMember>>,
    comp: &str,
    already: usize,
) -> Result<usize, ComponentError> {
    let mut n = 0;
    for s in b.stmts.iter_mut() {
        n += slot_in_stmt(s, content, comp, already + n)?;
    }
    if let Some(t) = &mut b.trailing {
        n += slot_in_expr(t, content, comp, already + n)?;
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Substitution: rewrite `Ident(param-or-state)` occurrences through
// the whole body. Binders (for / let / var / lambda / patterns) that
// reuse a substituted name are a named error — shadowing a component
// parameter inside its own body is always a bug.

fn subst_element(
    e: &Element,
    map: &HashMap<String, Expr>,
    comp: &str,
) -> Result<Element, ComponentError> {
    let mut members = Vec::with_capacity(e.members.len());
    for m in &e.members {
        members.push(match m {
            ElementMember::Property { key, value, span } => {
                // TextField handlers bind an implicit `text`; it
                // shadows a same-named param/state inside the handler
                // (mirrors the view-state desugar's scoping). The
                // toggles' `onToggle` binds an implicit `checked`
                // the same way.
                let value = if (key == "onTextChanged" || key == "onSubmitted")
                    && map.contains_key("text")
                {
                    let mut scoped = map.clone();
                    scoped.remove("text");
                    subst_expr(value, &scoped, comp)?
                } else if key == "onToggle" && map.contains_key("checked") {
                    let mut scoped = map.clone();
                    scoped.remove("checked");
                    subst_expr(value, &scoped, comp)?
                } else {
                    subst_expr(value, map, comp)?
                };
                ElementMember::Property {
                    key: key.clone(),
                    value,
                    span: *span,
                }
            }
            ElementMember::Child(c) => ElementMember::Child(subst_element(c, map, comp)?),
            ElementMember::Stmt(s) => ElementMember::Stmt(subst_stmt(s, map, comp)?),
        });
    }
    Ok(Element {
        module_path: e.module_path.clone(),
        name: e.name.clone(),
        members,
        span: e.span,
    })
}

fn check_binder(name: &Ident, map: &HashMap<String, Expr>, comp: &str) -> Result<(), ComponentError> {
    if map.contains_key(&name.name) {
        return err(
            name.span,
            format!(
                "`{}` shadows a param/state of component `{comp}` — rename the local",
                name.name
            ),
        );
    }
    Ok(())
}

fn pattern_binders<'p>(p: &'p Pattern, out: &mut Vec<&'p Ident>) {
    match p {
        Pattern::Bind { name, .. } => out.push(name),
        Pattern::Ctor { args, .. } => {
            for a in args {
                pattern_binders(a, out);
            }
        }
        Pattern::Literal { .. } | Pattern::Wild { .. } => {}
    }
}

fn subst_stmt(s: &Stmt, map: &HashMap<String, Expr>, comp: &str) -> Result<Stmt, ComponentError> {
    Ok(match s {
        Stmt::Let {
            name,
            ty,
            value,
            span,
            block_id,
        } => {
            check_binder(name, map, comp)?;
            Stmt::Let {
                name: name.clone(),
                ty: ty.clone(),
                value: subst_expr(value, map, comp)?,
                span: *span,
                block_id: *block_id,
            }
        }
        Stmt::Var {
            name,
            ty,
            value,
            span,
            block_id,
        } => {
            check_binder(name, map, comp)?;
            Stmt::Var {
                name: name.clone(),
                ty: ty.clone(),
                value: subst_expr(value, map, comp)?,
                span: *span,
                block_id: *block_id,
            }
        }
        Stmt::Expr(e) => Stmt::Expr(subst_expr(e, map, comp)?),
        Stmt::Return { value, span } => Stmt::Return {
            value: match value {
                Some(v) => Some(subst_expr(v, map, comp)?),
                None => None,
            },
            span: *span,
        },
        Stmt::Emit { signal, args, span } => Stmt::Emit {
            signal: signal.clone(),
            args: args
                .iter()
                .map(|a| subst_expr(a, map, comp))
                .collect::<Result<_, _>>()?,
            span: *span,
        },
        Stmt::Assign {
            target,
            op,
            value,
            span,
        } => Stmt::Assign {
            target: subst_expr(target, map, comp)?,
            op: *op,
            value: subst_expr(value, map, comp)?,
            span: *span,
        },
        Stmt::For {
            binding,
            index,
            iter,
            body,
            span,
        } => {
            check_binder(binding, map, comp)?;
            Stmt::For {
                binding: binding.clone(),
                index: index.clone(),
                iter: subst_expr(iter, map, comp)?,
                body: subst_block(body, map, comp)?,
                span: *span,
            }
        }
        Stmt::While { cond, body, span } => Stmt::While {
            cond: subst_expr(cond, map, comp)?,
            body: subst_block(body, map, comp)?,
            span: *span,
        },
        Stmt::Break { span } => Stmt::Break { span: *span },
        Stmt::Continue { span } => Stmt::Continue { span: *span },
        Stmt::Batch { body, span } => Stmt::Batch {
            body: subst_block(body, map, comp)?,
            span: *span,
        },
    })
}

fn subst_block(b: &Block, map: &HashMap<String, Expr>, comp: &str) -> Result<Block, ComponentError> {
    let mut stmts = Vec::with_capacity(b.stmts.len());
    for s in &b.stmts {
        stmts.push(subst_stmt(s, map, comp)?);
    }
    let trailing = match &b.trailing {
        Some(t) => Some(Box::new(subst_expr(t, map, comp)?)),
        None => None,
    };
    Ok(Block {
        stmts,
        trailing,
        span: b.span,
    })
}

fn subst_expr(e: &Expr, map: &HashMap<String, Expr>, comp: &str) -> Result<Expr, ComponentError> {
    use ExprKind as K;
    let kind = match &e.kind {
        K::Ident(n) => match map.get(n) {
            Some(replacement) => return Ok(replacement.clone()),
            None => K::Ident(n.clone()),
        },
        K::Str(parts) => {
            // Substitute inside interpolations, then FOLD: an
            // argument that turned out to be a literal joins the
            // surrounding text (a nested string interpolation
            // flattens its parts into this one). Literal formatting
            // matches the runtime — both are Rust `Display` on the
            // same primitive — so the two tiers keep byte-identical
            // output.
            let mut folded: Vec<StrPart> = Vec::with_capacity(parts.len());
            let push_text = |folded: &mut Vec<StrPart>, t: &str| {
                if let Some(StrPart::Text(last)) = folded.last_mut() {
                    last.push_str(t);
                } else {
                    folded.push(StrPart::Text(t.to_string()));
                }
            };
            for p in parts {
                match p {
                    StrPart::Text(t) => push_text(&mut folded, t),
                    StrPart::Interp(x) => {
                        let x2 = subst_expr(x, map, comp)?;
                        match &x2.kind {
                            K::Str(inner) => {
                                for ip in inner {
                                    match ip {
                                        StrPart::Text(t) => push_text(&mut folded, t),
                                        other => folded.push(other.clone()),
                                    }
                                }
                            }
                            K::Int(v) => push_text(&mut folded, &v.to_string()),
                            K::Float(v) => push_text(&mut folded, &v.to_string()),
                            K::Bool(v) => push_text(&mut folded, &v.to_string()),
                            _ => folded.push(StrPart::Interp(Box::new(x2))),
                        }
                    }
                    StrPart::InterpFmt { expr, format_spec } => {
                        folded.push(StrPart::InterpFmt {
                            expr: Box::new(subst_expr(expr, map, comp)?),
                            format_spec: format_spec.clone(),
                        });
                    }
                }
            }
            K::Str(folded)
        }
        K::Call {
            callee,
            args,
            block,
            type_args,
        } => K::Call {
            callee: Box::new(subst_expr(callee, map, comp)?),
            args: args
                .iter()
                .map(|a| subst_expr(a, map, comp))
                .collect::<Result<_, _>>()?,
            block: match block {
                Some(b) => Some(Box::new(subst_expr(b, map, comp)?)),
                None => None,
            },
            type_args: type_args.clone(),
        },
        K::MethodCall {
            receiver,
            method,
            args,
            block,
            type_args,
        } => K::MethodCall {
            receiver: Box::new(subst_expr(receiver, map, comp)?),
            method: method.clone(),
            args: args
                .iter()
                .map(|a| subst_expr(a, map, comp))
                .collect::<Result<_, _>>()?,
            block: match block {
                Some(b) => Some(Box::new(subst_expr(b, map, comp)?)),
                None => None,
            },
            type_args: type_args.clone(),
        },
        K::Member { receiver, name } => K::Member {
            receiver: Box::new(subst_expr(receiver, map, comp)?),
            name: name.clone(),
        },
        K::SafeMember { receiver, name } => K::SafeMember {
            receiver: Box::new(subst_expr(receiver, map, comp)?),
            name: name.clone(),
        },
        K::SafeMethodCall {
            receiver,
            method,
            args,
            block,
            type_args,
        } => K::SafeMethodCall {
            receiver: Box::new(subst_expr(receiver, map, comp)?),
            method: method.clone(),
            args: args
                .iter()
                .map(|a| subst_expr(a, map, comp))
                .collect::<Result<_, _>>()?,
            block: match block {
                Some(b) => Some(Box::new(subst_expr(b, map, comp)?)),
                None => None,
            },
            type_args: type_args.clone(),
        },
        K::Index { receiver, index } => K::Index {
            receiver: Box::new(subst_expr(receiver, map, comp)?),
            index: Box::new(subst_expr(index, map, comp)?),
        },
        K::Block(b) => K::Block(subst_block(b, map, comp)?),
        K::Lambda { params, body } => {
            for p in params {
                check_binder(&p.name, map, comp)?;
            }
            K::Lambda {
                params: params.clone(),
                body: subst_block(body, map, comp)?,
            }
        }
        K::Unary { op, expr } => K::Unary {
            op: *op,
            expr: Box::new(subst_expr(expr, map, comp)?),
        },
        K::Binary { op, lhs, rhs } => K::Binary {
            op: *op,
            lhs: Box::new(subst_expr(lhs, map, comp)?),
            rhs: Box::new(subst_expr(rhs, map, comp)?),
        },
        K::Try(inner) => K::Try(Box::new(subst_expr(inner, map, comp)?)),
        K::If {
            cond,
            then_b,
            else_b,
            let_binding,
        } => {
            if let Some((pat, _)) = let_binding {
                let mut binders = Vec::new();
                pattern_binders(pat, &mut binders);
                for b in binders {
                    check_binder(b, map, comp)?;
                }
            }
            K::If {
                cond: Box::new(subst_expr(cond, map, comp)?),
                then_b: subst_block(then_b, map, comp)?,
                else_b: match else_b {
                    Some(b) => Some(subst_block(b, map, comp)?),
                    None => None,
                },
                let_binding: match let_binding {
                    Some((pat, init)) => {
                        Some((pat.clone(), Box::new(subst_expr(init, map, comp)?)))
                    }
                    None => None,
                },
            }
        }
        K::Case { scrutinee, arms } => {
            let mut new_arms = Vec::with_capacity(arms.len());
            for a in arms {
                let mut binders = Vec::new();
                pattern_binders(&a.pattern, &mut binders);
                for b in binders {
                    check_binder(b, map, comp)?;
                }
                new_arms.push(CaseArm {
                    pattern: a.pattern.clone(),
                    body: subst_block(&a.body, map, comp)?,
                    span: a.span,
                });
            }
            K::Case {
                scrutinee: Box::new(subst_expr(scrutinee, map, comp)?),
                arms: new_arms,
            }
        }
        K::Await(inner) => K::Await(Box::new(subst_expr(inner, map, comp)?)),
        K::Kwarg { key, value } => K::Kwarg {
            key: key.clone(),
            value: Box::new(subst_expr(value, map, comp)?),
        },
        K::Element(el) => K::Element(subst_element(el, map, comp)?),
        K::Array(items) => K::Array(
            items
                .iter()
                .map(|x| subst_expr(x, map, comp))
                .collect::<Result<_, _>>()?,
        ),
        K::Map(pairs) => K::Map(
            pairs
                .iter()
                .map(|(k, v)| Ok((subst_expr(k, map, comp)?, subst_expr(v, map, comp)?)))
                .collect::<Result<_, ComponentError>>()?,
        ),
        K::Range {
            start,
            end,
            inclusive,
        } => K::Range {
            start: Box::new(subst_expr(start, map, comp)?),
            end: Box::new(subst_expr(end, map, comp)?),
            inclusive: *inclusive,
        },
        K::Int(_) | K::Float(_) | K::Bool(_) | K::Nil | K::Sym(_) | K::AtIdent(_) | K::SelfRef
        | K::Path(_) => e.kind.clone(),
    };
    Ok(Expr { kind, span: e.span })
}
