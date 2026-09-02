#!/usr/bin/env python3
"""Translate a yokan dialect app to .pix and PROVE the native build
behaves identically, by replaying one PIXIE_SCRIPT through both tiers
and byte-diffing the element-tree dumps.

  python3 yokan_gate.py translate demo/counter.py
  python3 yokan_gate.py gate demo/counter.py --script "click:+1,input:Momo"
  python3 yokan_gate.py gate demo/counter.py --script "..." --release

The claim discipline: never
"Python compiles" — always "THIS app translates, and the gate proved
the binary agrees byte for byte". Out-of-dialect constructs fail with
the .py line, never silently degrade.
"""

import argparse
import copy
import ast
import re
import importlib.util
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def repo() -> str:
    """The pixie repo, needed only to compile the native tier —
    `translate` never asks. Resolution: $PIXIE_REPO, then the dev
    tree this file sits in, then upward from the cwd (so the
    wheel-installed `yokan` command works inside a checkout)."""
    cands = [os.environ.get("PIXIE_REPO"), os.path.abspath(os.path.join(HERE, "..", ".."))]
    d = os.getcwd()
    while True:
        cands.append(d)
        up = os.path.dirname(d)
        if up == d:
            break
        d = up
    for c in cands:
        if c and os.path.isfile(os.path.join(c, "crates", "pixie-cli", "Cargo.toml")):
            return c
    sys.exit("compiling the native tier needs the pixie repo — set PIXIE_REPO=/path/to/pixie")


SOURCES: dict = {}   # parsed file -> its lines, for refusal excerpts


def parse_source(path: str) -> ast.Module:
    """Parse a module and stamp every node with its file, so a
    refusal raised deep inside a flattened import can still say
    which file it came from."""
    src = open(path).read()
    tree = ast.parse(src, filename=path)
    for n in ast.walk(tree):
        n._yokan_file = path
    SOURCES[path] = src.splitlines()
    return tree


def module_consts(trees) -> tuple[dict, set]:
    """Module-level names bound to a pure value, and the statements
    that bind them.

    This runs BEFORE the scan, because the scan already translates
    what it reads (a store's methods, a state's initial value), so a
    constant has to be resolved by then. The test is therefore
    syntactic: literals, containers of them, arithmetic over them, and
    Enum members. It is wider than the scan's own `_is_const_expr` —
    but a binding this accepts and the scan does not is refused at the
    binding as a module-level statement, so guessing wide cannot
    change what an app does.
    """
    enum_names = set()
    for tree in trees:
        for st in tree.body:
            if isinstance(st, ast.ClassDef) and any(
                (isinstance(b, ast.Name) and b.id.endswith("Enum"))
                or (isinstance(b, ast.Attribute) and b.attr.endswith("Enum"))
                for b in st.bases
            ):
                enum_names.add(st.name)

    consts: dict = {}
    decls: set = set()

    def pure(e) -> bool:
        if isinstance(e, ast.Constant):
            return True
        if isinstance(e, (ast.List, ast.Tuple, ast.Set)):
            return all(pure(x) for x in e.elts)
        if isinstance(e, ast.Dict):
            return all(k is not None and pure(k) for k in e.keys) and all(pure(v) for v in e.values)
        if isinstance(e, ast.UnaryOp):
            return pure(e.operand)
        if isinstance(e, ast.BinOp):
            return pure(e.left) and pure(e.right)
        if isinstance(e, ast.BoolOp):
            return all(pure(v) for v in e.values)
        if isinstance(e, ast.Compare):
            return pure(e.left) and all(pure(c) for c in e.comparators)
        if isinstance(e, ast.JoinedStr):
            return all(
                isinstance(v, ast.Constant)
                or (isinstance(v, ast.FormattedValue) and pure(v.value))
                for v in e.values
            )
        if isinstance(e, ast.Name):
            return e.id in consts
        if isinstance(e, ast.Attribute):
            return isinstance(e.value, ast.Name) and e.value.id in enum_names
        return False

    for tree in trees:
        for st in tree.body:
            if isinstance(st, ast.Assign):
                targets, val = st.targets, st.value
            elif isinstance(st, ast.AnnAssign) and st.value is not None:
                targets, val = [st.target], st.value
            else:
                continue
            if not all(isinstance(t, ast.Name) for t in targets) or not pure(val):
                continue
            # Resolve now, in source order: a constant built from an
            # earlier one takes the value that was current there.
            val = ConstInliner(consts, set()).visit(copy.deepcopy(val))
            for t in targets:
                consts[t.id] = val
            decls.add(id(st))
    return consts, decls


class OptionalSpelling(ast.NodeTransformer):
    """`Optional[T]` is `T | None`. typing's older spelling says the
    same thing, so it is read as the one the tour documents, in every
    annotation at once."""

    def visit_Subscript(self, node):
        self.generic_visit(node)
        named = (
            (isinstance(node.value, ast.Name) and node.value.id == "Optional")
            or (isinstance(node.value, ast.Attribute) and node.value.attr == "Optional")
        )
        if not named or isinstance(node.slice, ast.Tuple):
            return node
        out = ast.BinOp(left=node.slice, op=ast.BitOr(), right=ast.Constant(value=None))
        for n in (out, out.right):
            ast.copy_location(n, node)
            n._yokan_file = getattr(node, "_yokan_file", None)
        return out


class _CompArgs(ast.NodeTransformer):
    """Write a component's by-reference arguments into its body: the
    handler and the cell belong to the caller, so the specialized view
    reads them where the parameter was named."""

    def __init__(self, subs: dict):
        self.subs = subs

    def visit_Name(self, node):
        if not isinstance(node.ctx, ast.Load) or node.id not in self.subs:
            return node
        val = copy.deepcopy(self.subs[node.id])
        for n in ast.walk(val):
            ast.copy_location(n, node)
            n._yokan_file = getattr(node, "_yokan_file", None)
        return val


class ConstInliner(ast.NodeTransformer):
    """Write a module constant's value where its name is read.

    A module-level literal binding is a declaration: CPython evaluates
    it once at import and the compiled app has no import step, so the
    name and the value are interchangeable. Locals, parameters and loop
    variables shadow it, exactly as they do in Python — a name assigned
    anywhere inside a function belongs to that function.
    """

    def __init__(self, consts: dict, decls: set):
        self.consts = consts
        self.decls = decls
        self.shadow = []

    @staticmethod
    def _bound(node) -> set:
        names = {a.arg for a in [*node.args.posonlyargs, *node.args.args, *node.args.kwonlyargs]}
        if node.args.vararg:
            names.add(node.args.vararg.arg)
        if node.args.kwarg:
            names.add(node.args.kwarg.arg)
        body = node.body if isinstance(node.body, list) else [node.body]
        for stmt in body:
            for n in ast.walk(stmt):
                if isinstance(n, ast.Name) and isinstance(n.ctx, (ast.Store, ast.Del)):
                    names.add(n.id)
                elif isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                    names.add(n.name)
                elif isinstance(n, ast.ExceptHandler) and n.name:
                    names.add(n.name)
        return names

    def _scoped(self, node):
        self.shadow.append(self._bound(node))
        try:
            self.generic_visit(node)
        finally:
            self.shadow.pop()
        return node

    visit_FunctionDef = _scoped
    visit_AsyncFunctionDef = _scoped
    visit_Lambda = _scoped

    def _decl(self, node):
        # The bindings themselves were resolved when they were
        # collected; walking them again would apply a later value.
        return node if id(node) in self.decls else self.generic_visit(node)

    visit_Assign = _decl
    visit_AnnAssign = _decl

    def visit_Name(self, node):
        if not isinstance(node.ctx, ast.Load) or node.id not in self.consts:
            return node
        if any(node.id in scope for scope in self.shadow):
            return node
        val = copy.deepcopy(self.consts[node.id])
        for n in ast.walk(val):
            ast.copy_location(n, node)
            n._yokan_file = getattr(node, "_yokan_file", None)
        return val


def at(new, old):
    """`ast.copy_location` plus the file stamp — a synthesized node
    still has to be able to say which file it came from."""
    ast.copy_location(new, old)
    new._yokan_file = getattr(old, "_yokan_file", None)
    return new


def py_ty(t) -> str:
    """The user-facing spelling of an internal type name: `Int` is
    `int`, `List<String>` is `list[str]`, `Int?` is `int | None`."""
    if t is None:
        return "?"
    if t.endswith("?"):
        return f"{py_ty(t[:-1])} | None"
    m = re.fullmatch(r"List<(.+)>", t)
    if m:
        return f"list[{py_ty(m.group(1))}]"
    m = re.fullmatch(r"Map<(\w+), (.+)>", t)
    if m:
        return f"dict[{py_ty(m.group(1))}, {py_ty(m.group(2))}]"
    return {"Int": "int", "Float": "float", "String": "str", "Bool": "bool"}.get(t, t)


class RawPix(str):
    """Pixie source that is already an expression: a property writes it
    as it is, where a plain str would be quoted."""


def pixstr(v) -> str:
    """A string-valued property's source: a literal is quoted, an
    expression the view re-reads is written as it is."""
    return str(v) if isinstance(v, RawPix) else f'"{esc(v)}"'


class Untranslatable(Exception):
    """A refusal: where (file, line, column) and why. `str(e)` is
    the one-line form; `render()` adds the source excerpt."""

    def __init__(self, node, msg):
        self.msg = msg
        self.file = getattr(node, "_yokan_file", None)
        self.line = getattr(node, "lineno", None)
        self.col = getattr(node, "col_offset", None)
        super().__init__(f"{self.where()}: {msg}")

    def where(self) -> str:
        if self.file is None:
            return f"line {self.line if self.line is not None else '?'}"
        rel = os.path.relpath(self.file)
        path = rel if not rel.startswith("..") else self.file
        if self.line is None:
            return path
        if self.col is None:
            return f"{path}:{self.line}"
        return f"{path}:{self.line}:{self.col + 1}"

    def render(self, prefix: str = "not in the dialect") -> str:
        # Most messages name the boundary themselves ("`print(...)` is
        # not in the dialect yet — …"); prefixing those said it twice.
        head = (f"{self.where()}: {self.msg}" if "in the dialect" in self.msg
                else f"{self.where()}: {prefix} — {self.msg}")
        lines = SOURCES.get(self.file or "")
        if not lines or self.line is None or not (1 <= self.line <= len(lines)):
            return head
        src = lines[self.line - 1]
        caret = " " * (self.col or 0) + "^"
        return f"{head}\n    {src}\n    {caret}"


def esc(s: str) -> str:
    if "#{" in s:
        raise ValueError("literal contains `#{` (pixie interpolation opener)")
    return s.replace("\\", "\\\\").replace('"', '\\"')


class Translator:
    """One pass over the module ast → .pix text in the hand-written
    demos' idiom: store App + synthesized handler fns + view Main."""

    def __init__(self, tree: ast.Module, modules: dict | None = None):
        self.tree = tree
        self.modules = modules or {}   # name -> ast.Module (local imports, flattened)
        self.components = {}           # def name -> (CompName, [(param, ty)], [lines])
        self.comp_params = None        # {param: ty} while translating a component body
        self.comp_defs = set()         # defs decorated with @ui.component
        self.comp_slotted = set()      # ...with slots=True (take children)
        self.in_slotted_comp = False   # translating a slotted comp body
        self.escapes = {}              # @ui.py fns: name -> {params, ret, src}
        self.handler_locals = set()    # plain-Name assigns in the current def handler
        self.inline_handler = False    # inside a component's inline onClick block
        self.uses_stdlib = False    # any yokan.fs call (the module lane)
        self.fs_name = None            # `from yokan import fs` binding, if any
        self.stdlib_mods = {}        # local name -> stdlib module ("fs", ...)
        self.dead_locals = {}          # name -> why it cannot be read here
        self.helpers = {}              # pure fn name -> (params, ret, body expr)
        self.helper_params = None      # param name -> pix type while inside one
        self.nonneg_loop_vars = set()  # range() loop vars provably >= 0
        self.text_hole = False         # inside an f-string hole (reads only)
        self.structs = {}              # frozen dataclass -> [(field, pixty, default_lit)]
        self.models = {}               # @ui.model -> {"fields", "methods", "impls"}
        self.model_instances = {}      # module-level `x = Model()` -> model name
        self.protocols = {}            # Protocol class -> [(method, params, ret)]
        self.replace_name = None       # `from dataclasses import replace` binding
        self.dataclass_names = set()   # local names bound to dataclasses.dataclass
        self.protocol_names = set()    # local names bound to typing.Protocol
        self.model_scope = None        # (model name, {field: pixty}) inside a method
        self.typed_locals = {}         # method params in scope -> pix type
        self.unfrozen = set()          # dataclasses WITHOUT frozen=True (for the message)
        self.styles = {}               # python name -> (PixName, lines|merge)
        self.enums = {}                # Enum class -> [member names]
        self.enum_bases = set()        # local names bound to enum.Enum
        self.view_bindings = {}        # view binding name -> pixty (if-let / case arms)
        self.unions = {}               # `type Shape = A | B` -> [variant struct names]
        self.union_of = {}             # variant struct -> union name
        self.stores = {}               # @ui.store name -> {"fields", "methods"}
        self.on_start = None           # ("store", S, m) | ("def", name) for tier A
        self.window = {}               # ui.run title=/width=/height= -> pixie.toml [window]
        self.enum_text_used = set()    # enums rendered in text -> synthesized PyText statics
        self.try_wrappers = []         # typed-except sites -> synthesized python dispatchers
        self.try_seq = 0               # per-handler try-construct counter
        self.comp_locals = None        # {local: ty} while translating a component body
        self.inline_text_param = None  # handler param rendered as the implicit in inline blocks
        self.inline_implicit_name = None  # which implicit name that is ("text"/"checked"/...)
        self.in_wrapped_hole = False   # _hole already wrapped (floatRepr/boolRepr): inner guards stand down
        self.ui = None            # the `import yokan as X` alias
        self.yokan_names = {}    # bare from-imports: local name -> yokan name
        self.crates_name = None   # `from yokan import crates` alias
        self.crate_info = {}      # crate -> {"class","path","fns":{fn:(params,ret) | str reason}}
        self.uses_crates = set()  # crate names actually called
        self.struct_methods = {}  # struct -> {py name: (emitted, [(param, ty)], ret)}
        self.model_names = set()  # pre-pass: every @model class name (mutual/self refs)
        self.struct_self = None   # (struct name) while translating a value method body
        self.state = {}           # key -> ("Int"|"String", pix literal)
        self.cells = {}           # cell name -> pix type ("Int"/"List<String>"/…)
        self.row = None           # (cell, param, loopvar) while translating a row fn
        self.state_node = None    # the dict literal, for tier A
        self.handlers = []        # (name, param: str|None, [pix stmt lines])
        self.defs = {}            # module-level def name -> FunctionDef
        self.consts = {}          # module-level literal constants: name -> value node
        self.pre_lines = None     # lines an expression needs before its statement
        self.prop_stack = []      # @property expansion, for the self-reference guard
        self.ret_ty = None        # pix return type of the body being translated
        self.enum_values = {}     # Enum -> {member: the value Python gives it}
        self.enum_meta_used = set()   # enums whose .name / .value statics are emitted
        self.in_prop = False      # inside a numeric element property (arithmetic lowers there)
        self.comp_specials = {}   # (component, argument text) -> the view built for that call
        self.in_async = False     # translating a task's body: stdlib calls await
        self.async_hit = False    # ...and the function around it is async
        self.async_handlers = set()   # handler names emitted as `async fn`
        self.escape_structs = set()   # value classes crossing an @py signature
        self.timers = []          # module-level every(seconds, fn) declarations
        self.in_try = False       # inside a try body: a raise would be caught there
        self.timer_handlers = {}  # handler name -> period in ms
        self.view = None

    def _scan_import(self, node):
        if isinstance(node, ast.Import):
            for a in node.names:
                if a.name == "yokan":
                    self.ui = a.asname or "yokan"
        elif isinstance(node, ast.ImportFrom) and node.module == "dataclasses":
            for a in node.names:
                if a.name == "dataclass":
                    self.dataclass_names.add(a.asname or "dataclass")
                elif a.name == "replace":
                    self.replace_name = a.asname or "replace"
        elif isinstance(node, ast.ImportFrom) and node.module == "typing":
            for a in node.names:
                if a.name == "Protocol":
                    self.protocol_names.add(a.asname or "Protocol")
        elif isinstance(node, ast.ImportFrom) and node.module == "enum":
            for a in node.names:
                if a.name == "Enum":
                    self.enum_bases.add(a.asname or "Enum")
        elif isinstance(node, ast.ImportFrom) and node.module == "yokan":
            for a in node.names:
                if a.name == "crates":
                    self.crates_name = a.asname or "crates"
                elif a.name in ("fs", "sqlite", "http", "math", "json", "time", "strings", "random", "notify"):
                    self.stdlib_mods[a.asname or a.name] = a.name
                    if a.name == "fs":
                        self.fs_name = a.asname or "fs"
                else:
                    # `from yokan import State, store, value, ...`
                    # — the bare spelling every doc sample uses.
                    self.yokan_names[a.asname or a.name] = a.name

    def _is_state_call(self, node) -> bool:
        if not isinstance(node, ast.Call):
            return False
        f = node.func
        if (
            isinstance(f, ast.Attribute)
            and f.attr == "State"
            and isinstance(f.value, ast.Name)
            and self.ui is not None
            and f.value.id == self.ui
        ):
            return True
        return isinstance(f, ast.Name) and self.yokan_names.get(f.id) == "State"

    def _cell_field(self, call: ast.Call, ann):
        if len(call.args) != 1:
            raise Untranslatable(call, "State(...) takes exactly one argument, the initial value")
        if ann is not None:
            # annotation-first: `count: ui.State[int] = ui.State(0)` —
            # the annotation survives where literal inference dies
            # ([] / None), so it is the preferred spelling.
            if not isinstance(ann, ast.Subscript):
                raise Untranslatable(ann, "annotate the state: `count: State[int] = State(0)`")
            sl = ann.slice
            if isinstance(sl, ast.Name) and sl.id in self.unions:
                init = call.args[0]
                if not (
                    isinstance(init, ast.Call)
                    and isinstance(init.func, ast.Name)
                    and self.union_of.get(init.func.id) == sl.id
                ):
                    raise Untranslatable(init, f"a {sl.id} state starts from one of its variants (`State(Healthy())`)")
                return (sl.id, self._variant_ctor(init))
            if isinstance(sl, ast.Name) and sl.id in self.enums:
                init = call.args[0]
                if not (
                    isinstance(init, ast.Attribute)
                    and isinstance(init.value, ast.Name)
                    and init.value.id == sl.id
                    and init.attr in self.enums[sl.id]
                ):
                    raise Untranslatable(init, f"a {sl.id} state starts from a member (`State({sl.id}.MEMBER)`)")
                return (sl.id, f"{sl.id}.{init.attr}")
            if (
                isinstance(sl, ast.BinOp)
                and isinstance(sl.op, ast.BitOr)
            ):
                base = None
                for side, other in ((sl.left, sl.right), (sl.right, sl.left)):
                    if isinstance(other, ast.Constant) and other.value is None and isinstance(side, ast.Name):
                        base = self.HELPER_TY.get(side.id)
                        if base is None and side.id in self.model_names:
                            init = call.args[0]
                            if not (isinstance(init, ast.Constant) and init.value is None):
                                raise Untranslatable(init, "a model reference starts as None — wire it in a handler")
                            return (f"{side.id}?", "nil")
                if base is None:
                    wider = self._pix_ty(sl)
                    if wider is None:
                        raise Untranslatable(ann, "an optional state is a scalar, a list, a dict, a value class, an Enum or a model reference, each `| None`")
                    return (wider, self._literal_of(call.args[0], wider))
                init = call.args[0]
                if isinstance(init, ast.Constant) and init.value is None:
                    return (f"{base}?", "nil")
                got, lit = self._state_field(init)
                if got != base:
                    raise Untranslatable(init, f"the default is {self._py_ty(got)}, the annotation says `{base} | None`")
                return (f"{base}?", lit)
            if isinstance(sl, ast.Name) and sl.id in self.unfrozen:
                raise Untranslatable(
                    ann,
                    f"`{sl.id}` is a mutable dataclass — aliasing through it would be observable, which a value cannot express; mark it @value (or @dataclass(frozen=True)) and update with `replace`",
                )
            if isinstance(sl, ast.Name) and sl.id in self.structs:
                init = call.args[0]
                if not (isinstance(init, ast.Call) and isinstance(init.func, ast.Name) and init.func.id == sl.id):
                    raise Untranslatable(init, f"a {sl.id} state starts from a {sl.id}(...) literal")
                for a in ast.walk(init):
                    if isinstance(a, ast.Call) and a is not init:
                        raise Untranslatable(a, "a value-class state starts from a literal construction (`State(Point(0, 0))`)")
                return (sl.id, self._struct_ctor(sl.id, init, "store", None))
            if isinstance(sl, ast.Name):
                ty = {"int": "Int", "str": "String", "bool": "Bool", "float": "Float"}.get(sl.id)
                if ty is None:
                    raise Untranslatable(
                        ann,
                        f"State[{sl.id}] is not a state type — the state types are int, str, float, bool, list[...] and dict[str, ...] of those, `T | None`, an Enum, a value class or a sum type",
                    )
                lit = self._state_field(call.args[0])[1]
                field = (ty, lit)
            elif (
                isinstance(sl, ast.Subscript)
                and isinstance(sl.value, ast.Name)
                and sl.value.id == "list"
                and isinstance(sl.slice, ast.Name)
            ):
                inner = self._pix_ty(sl.slice)
                if inner is None:
                    raise Untranslatable(ann, f"list[{self._src(sl.slice)}] is not a state type — list elements are int, float, str, bool, a value class, an Enum, or a list or dict of those")
                field = (f"List<{inner}>", self._list_literal(call.args[0], inner))
            elif (
                isinstance(sl, ast.Subscript)
                and isinstance(sl.value, ast.Name)
                and sl.value.id == "dict"
                and isinstance(sl.slice, ast.Tuple)
                and len(sl.slice.elts) == 2
                and isinstance(sl.slice.elts[0], ast.Name)
                and isinstance(sl.slice.elts[1], ast.Name)
            ):
                key = self._pix_ty(sl.slice.elts[0])
                if key not in ("String", "Int"):
                    raise Untranslatable(ann, "a dict keys by str or int")
                inner = self._pix_ty(sl.slice.elts[1])
                if inner is None:
                    raise Untranslatable(ann, f"dict[{self._src(sl.slice.elts[0])}, {self._src(sl.slice.elts[1])}] is not a state type — dict values are int, float, str, bool, a value class, an Enum, or a list or dict of those")
                field = (f"Map<{key}, {inner}>", self._dict_literal(call.args[0], inner))
            else:
                wider = self._pix_ty(sl)
                if wider is None:
                    raise Untranslatable(ann, f"State[{self._src(sl)}] is not a state type — the state types are int, str, float, bool, lists and str- or int-keyed dicts of those, value classes, Enums, sum types, and `T | None` of any of them")
                field = (wider, self._literal_of(call.args[0], wider))
        else:
            field = self._state_field(call.args[0])
        # mark as a cell (kept alongside dict-style fields in self.state)
        return field

    # (pix type, rust PARAM type) — pixie's binding adapter hands
    # String arguments across as `&str` (the mathkit convention);
    # returns are owned. `_escape_ret` maps the return side.
    PYTY = {
        "int": ("Int", "i64"),
        "float": ("Float", "f64"),
        "str": ("String", "&str"),
        "bool": ("Bool", "bool"),
    }
    PYRET = {"&str": "String"}

    def _pix_ty(self, ann):
        """The pixie type an annotation names, or None. Nesting goes as
        far as the compiled side takes it: lists of lists, dicts of
        lists, optionals of either."""
        if isinstance(ann, ast.Constant) and isinstance(ann.value, str):
            try:
                ann = ast.parse(ann.value, mode="eval").body
            except SyntaxError:
                return None
        if isinstance(ann, ast.Name):
            t = self.HELPER_TY.get(ann.id)
            if t:
                return t
            if ann.id in self.structs or ann.id in self.enums or ann.id in self.unions:
                return ann.id
            return None
        if isinstance(ann, ast.BinOp) and isinstance(ann.op, ast.BitOr):
            if isinstance(ann.right, ast.Constant) and ann.right.value is None:
                inner = self._pix_ty(ann.left)
                return f"{inner}?" if inner and not inner.endswith("?") else None
            return None
        if isinstance(ann, ast.Subscript) and isinstance(ann.value, ast.Name):
            base = ann.value.id
            if base == "list":
                inner = self._pix_ty(ann.slice)
                return f"List<{inner}>" if inner else None
            if (
                base == "dict"
                and isinstance(ann.slice, ast.Tuple)
                and len(ann.slice.elts) == 2
            ):
                k = self._pix_ty(ann.slice.elts[0])
                v = self._pix_ty(ann.slice.elts[1])
                if k in ("String", "Int") and v:
                    return f"Map<{k}, {v}>"
        return None

    def _literal_of(self, node, ty: str) -> str:
        """The pixie literal a default writes, for any type `_pix_ty`
        reads. A container's items go through the same door, so a list
        of value classes and a dict of lists need no special case."""
        if ty.endswith("?"):
            if isinstance(node, ast.Constant) and node.value is None:
                return "nil"
            return self._literal_of(node, ty[:-1])
        if ty.startswith("List<"):
            return self._list_literal(node, ty[5:-1])
        if ty.startswith("Map<"):
            return self._dict_literal(node, ty.split(", ", 1)[1][:-1])
        if ty in self.unions:
            if not (
                isinstance(node, ast.Call)
                and isinstance(node.func, ast.Name)
                and self.union_of.get(node.func.id) == ty
            ):
                raise Untranslatable(node, f"a {ty} starts from one of its variants")
            return self._variant_ctor(node)
        if ty in self.structs:
            if not (
                isinstance(node, ast.Call)
                and isinstance(node.func, ast.Name)
                and node.func.id == ty
            ):
                raise Untranslatable(node, f"a {ty} starts from a {ty}(...) literal")
            return self._struct_ctor(ty, node, "store", None)
        if ty in self.enums:
            if not (
                isinstance(node, ast.Attribute)
                and isinstance(node.value, ast.Name)
                and node.value.id == ty
                and node.attr in self.enums[ty]
            ):
                raise Untranslatable(node, f"a {ty} starts from a member (`{ty}.MEMBER`)")
            return f"{ty}.{node.attr}"
        got, lit = self._state_field(node)
        if got == "Int" and ty == "Float":
            return repr(float(lit))
        if got != ty:
            raise Untranslatable(node, f"the value is {self._py_ty(got)}, the annotation says {self._py_ty(ty)}")
        return lit

    def _escape_ty(self, ann):
        if isinstance(ann, ast.Name) and ann.id in self.PYTY:
            return self.PYTY[ann.id]
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.value, ast.Name)
            and ann.value.id == "list"
            and isinstance(ann.slice, ast.Name)
            and ann.slice.id in self.PYTY
        ):
            pix, rust = self.PYTY[ann.slice.id]
            rust = "String" if rust == "&str" else rust
            return (f"List<{pix}>", f"Vec<{rust}>")
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.value, ast.Name)
            and ann.value.id == "dict"
            and isinstance(ann.slice, ast.Tuple)
            and len(ann.slice.elts) == 2
            and isinstance(ann.slice.elts[0], ast.Name)
            and isinstance(ann.slice.elts[1], ast.Name)
        ):
            if ann.slice.elts[0].id != "str":
                raise Untranslatable(ann, "a dict crossing @py is keyed by str — int keys are not in the dialect yet")
            v = ann.slice.elts[1].id
            if v not in self.PYTY:
                raise Untranslatable(ann, f"a dict crossing @py holds int, float, str or bool — `{v}` does not cross yet")
            pix, rust = self.PYTY[v]
            rust = "String" if rust == "&str" else rust
            return (f"Map<String, {pix}>", f"std::collections::HashMap<String, {rust}>")
        if isinstance(ann, ast.Name) and ann.id in self.structs:
            # A value class crosses as a struct with the same fields:
            # the generated crate declares it, and the escape module
            # gets a dataclass of the same shape to build one in
            # Python. Nested value classes do not cross yet.
            for _f, t, _d in self.structs[ann.id]:
                if t not in ("Int", "Float", "String", "Bool"):
                    raise Untranslatable(
                        ann,
                        f"`{ann.id}` crosses @py when its fields are int, float, str or bool — "
                        f"`{py_ty(t)}` inside it does not cross yet",
                    )
            self.escape_structs.add(ann.id)
            return (ann.id, f"crate::{ann.id}")
        opt = self._optional_of(ann)
        if opt is not None:
            if not (isinstance(opt, ast.Name) and opt.id in self.PYTY):
                raise Untranslatable(ann, "an optional crossing @py holds int, float, str or bool")
            pix, rust = self.PYTY[opt.id]
            rust = "String" if rust == "&str" else rust
            return (f"{pix}?", f"Option<{rust}>")
        raise Untranslatable(ann, "an @py function's parameters and return are int/float/str/bool, `list[...]` or `dict[str, ...]` of those, or `T | None` — a value class does not cross yet")

    @staticmethod
    def _optional_of(ann):
        """The `T` of a `T | None` annotation, or None."""
        if (
            isinstance(ann, ast.BinOp)
            and isinstance(ann.op, ast.BitOr)
            and isinstance(ann.right, ast.Constant)
            and ann.right.value is None
        ):
            return ann.left
        return None

    HELPER_TY = {"int": "Int", "float": "Float", "str": "String", "bool": "Bool"}

    # python kwarg -> (pixie style key, kind)
    STYLE_KEYS = {
        "spacing": ("spacing", "num"), "padding": ("padding", "num"),
        "background": ("background", "str"), "grow": ("grow", "num"),
        "size": ("fontSize", "num"), "color": ("color", "str"),
        "align": ("align", "str"), "width": ("width", "num"),
        "height": ("height", "num"), "basis": ("basis", "num"),
        "hover_background": ("hover.background", "str"),
        "active_background": ("active.background", "str"),
        "border_radius": ("borderRadius", "num"),
        "border_width": ("borderWidth", "num"),
        "border_color": ("borderColor", "str"),
    }

    EASINGS = ("linear", "in", "out", "inOut")

    # Scalar-returning standard-library calls the type reader knows.
    STDLIB_RET = {
        ("strings", "to_int"): "Int",
        ("strings", "to_float"): "Float",
    }

    def _anim_props(self, kw) -> list[str]:
        """§8.35's riders as kwargs: animate= (ms), easing=,
        enter=/exit= (bools) — stripped into Element::Anim by both
        lowerers; interpolation runs in the shared kernel, so
        `advance:` frames dump identically."""
        props = []
        if "animate" in kw:
            v = kw.pop("animate")
            if not (isinstance(v, ast.Constant) and type(v.value) in (int, float) and type(v.value) is not bool):
                raise Untranslatable(v, "animate= takes a duration in ms (number literal)")
            props.append(f"animate: {v.value}")
        if "easing" in kw:
            v = kw.pop("easing")
            if not (isinstance(v, ast.Constant) and v.value in self.EASINGS):
                raise Untranslatable(v, 'easing= is one of "linear", "in", "out", "inOut"')
            props.append(f'easing: "{v.value}"')
        for flag in ("enter", "exit"):
            if flag in kw:
                v = kw.pop(flag)
                if not (isinstance(v, ast.Constant) and v.value is True):
                    raise Untranslatable(v, f"{flag}= takes True (omit it otherwise)")
                props.append(f"{flag}: true")
        return props

    def _optional_test(self, test):
        """Recognize the narrowing shapes over an optional cell:
        `(v := cell()) is not None` / `cell() is not None` /
        `cell() is None`. Returns (cell, binding|None, none_first)
        or None. The walrus IS the some-binding — Python's spelling
        of `if let some(v)`."""
        if not (
            isinstance(test, ast.Compare)
            and len(test.ops) == 1
            and isinstance(test.ops[0], (ast.Is, ast.IsNot))
            and isinstance(test.comparators[0], ast.Constant)
            and test.comparators[0].value is None
        ):
            return None
        left = test.left
        binding = None
        if isinstance(left, ast.NamedExpr) and isinstance(left.target, ast.Name):
            binding = left.target.id
            left = left.value
        cell = None
        c = self._cell_read(left)
        if c is not None and self._ty(c).endswith("?"):
            cell = ("cell", c)
        elif (
            isinstance(left, ast.Attribute)
            and isinstance(left.value, ast.Name)
            and left.value.id == "self"
            and self.model_scope is not None
            and self.model_scope[1].get(left.attr, "").endswith("?")
        ):
            cell = ("selffield", left.attr)
        elif (
            isinstance(left, ast.Attribute)
            and isinstance(left.value, ast.Name)
            and left.value.id in self.stores
            and self.stores[left.value.id]["field_tys"].get(left.attr, "").endswith("?")
        ):
            cell = ("storefield", f"{left.value.id}.{left.attr}")
        elif (
            isinstance(left, ast.Attribute)
            and isinstance(left.value, ast.Name)
            and self.typed_locals.get(left.value.id) in self.models
        ):
            mi = self.typed_locals[left.value.id]
            fty9 = {f: t for f, t, _ in self.models[mi]["fields"]}.get(left.attr, "")
            if fty9.endswith("?"):
                cell = ("localfield", f"{left.value.id}.{left.attr}")
        elif (
            isinstance(left, ast.Attribute)
            and isinstance(left.value, ast.Name)
            and left.value.id in self.model_instances
        ):
            mi = self.model_instances[left.value.id]
            fty9 = {f: t for f, t, _ in self.models[mi]["fields"]}.get(left.attr, "")
            if fty9.endswith("?"):
                cell = ("instfield", f"{left.value.id}.{left.attr}")
        elif (
            isinstance(left, ast.Name)
            and left.id in self.handler_locals
            and self.typed_locals.get(left.id, "").endswith("?")
        ):
            cell = ("local", left.id)
        if cell is None:
            return None
        none_first = isinstance(test.ops[0], ast.Is)
        if none_first and binding is not None:
            raise Untranslatable(test, "narrow with `if (v := x()) is not None:` — the binding goes on the `is not None` side")
        return (cell, binding, none_first)

    def _row_source_read(self, node) -> bool:
        """True when `node` is the row source read: `xs()` for a cell
        row, `Store.field` for a store-field row."""
        if self.row is None:
            return False
        key = self.row[0]
        if isinstance(key, tuple):
            return (
                isinstance(node, ast.Attribute)
                and isinstance(node.value, ast.Name)
                and node.value.id == key[1]
                and node.attr == key[2]
            )
        return self._cell_read(node) == key

    def _row_elem_ty(self) -> str:
        key = self.row[0]
        if isinstance(key, tuple):
            return self.stores[key[1]]["field_tys"].get(key[2], "")
        return self.cells.get(key, "")

    def _style_name(self, py_name: str) -> str:
        return py_name[:1].upper() + py_name[1:]

    def _take_style(self, name: str, call: ast.Call):
        if call.args:
            raise Untranslatable(call, 'style(...) takes keyword arguments only (`style(size=18, color="accent")`)')
        lines = []
        for kw in call.keywords:
            if kw.arg is None:
                raise Untranslatable(call, "compose styles with `|`, not `**`")
            spec = self.STYLE_KEYS.get(kw.arg)
            if spec is None:
                raise Untranslatable(kw.value, f"`{kw.arg}` is not a style key — the keys are {', '.join(self.STYLE_KEYS)}")
            key, kind = spec
            v = kw.value
            if kind == "num":
                if not (isinstance(v, ast.Constant) and type(v.value) in (int, float) and type(v.value) is not bool):
                    raise Untranslatable(v, f"`{kw.arg}` takes a number literal")
                lines.append(f"  {key}: {float(v.value)!r}")
            else:
                if not (isinstance(v, ast.Constant) and type(v.value) is str):
                    raise Untranslatable(v, f"`{kw.arg}` takes a string literal (hex or a theme token)")
                lines.append(f'  {key}: "{esc(v.value)}"')
        if not lines:
            raise Untranslatable(call, "an empty style styles nothing")
        pix = self._style_name(name)
        self.styles[name] = (pix, [f"style {pix} {{", *lines, "}"])

    def _take_style_merge(self, name: str, node: ast.BinOp):
        parts = []
        def walk(n):
            if isinstance(n, ast.BinOp) and isinstance(n.op, ast.BitOr):
                walk(n.left)
                walk(n.right)
            elif isinstance(n, ast.Name) and n.id in self.styles:
                parts.append(self.styles[n.id][0])
            else:
                raise Untranslatable(n, "style composition takes named styles (`a | b`)")
        walk(node)
        pix = self._style_name(name)
        self.styles[name] = (pix, [f"style {pix} = {' + '.join(parts)}"])

    def _take_store(self, node: ast.ClassDef):
        """`@ui.store` ↔ pixie store: a named singleton with state
        AND methods — the grouping bundles lack (no methods) and
        models don't give (instances, not singletons). The decorator
        returns the instance, so the class NAME is the store."""
        fields = []
        methods = []
        for st in node.body:
            if isinstance(st, ast.AnnAssign) and isinstance(st.target, ast.Name):
                if st.value is None:
                    raise Untranslatable(st, "a store field needs a default")
                self._check_name(st.target, st.target.id, "store field")
                fields.append((st.target.id, *self._store_field_ty(st.annotation, st.value)))
            elif isinstance(st, ast.FunctionDef):
                methods.append(st)
            elif isinstance(st, (ast.Pass, ast.Expr)):
                continue
            else:
                raise Untranslatable(st, "a @store body holds annotated fields and methods")
        if not fields:
            raise Untranslatable(node, "a store needs at least one field")
        self.stores[node.name] = {
            "fields": fields,
            "field_tys": {f: t for f, t, _ in fields},
            "methods": [],
            "method_names": set(),
            "ret_tys": {},         # method -> pix return type
            "params": {},          # method -> [(name, pix type)]
            "defaults": {},        # method -> {name: default literal}
            "props": {},           # @property -> the expression it stands for
            "statics": {},         # @staticmethod -> the emitted static's name
        }
        field_tys = self.stores[node.name]["field_tys"]
        for m in methods:
            decos = [d.id if isinstance(d, ast.Name) else None for d in m.decorator_list]
            if "classmethod" in decos:
                raise Untranslatable(m.decorator_list[decos.index("classmethod")], "a `@classmethod` on a store is not in the dialect — the class name IS the instance here, so a class method has nothing of its own; write a `@staticmethod`")
            if "staticmethod" in decos:
                self._take_store_static(node.name, m)
                continue
            if "property" in decos:
                self._take_store_prop(node.name, m)
                continue
            if decos:
                raise Untranslatable(m.decorator_list[0], f"a store method takes `@property` or `@staticmethod` — `@{decos[0] or ast.unparse(m.decorator_list[0])}` is not in the dialect")
            if not m.args.args or m.args.args[0].arg != "self":
                raise Untranslatable(m, "a store method takes `self` first")
            params = []
            for a in m.args.args[1:]:
                if a.annotation is None:
                    raise Untranslatable(a, "annotate every store method parameter")
                self._check_name(a, a.arg, "parameter")
                ty = self._param_ty(a.annotation)
                if ty is None:
                    raise Untranslatable(
                        a.annotation,
                        "store method parameters take int/float/str/bool, list[...] of those, a value class, or an enum",
                    )
                params.append((a.arg, ty))
            self.stores[node.name]["params"][m.name] = params
            self.stores[node.name]["defaults"][m.name] = self._defaults_of(m)
            ret = self._method_ret(m, "store")
            if ret is not None:
                # Registered before the body is walked, so a method
                # that calls itself sees its own type.
                self.stores[node.name]["ret_tys"][m.name] = ret
            prev_scope = self.model_scope
            prev_locals, prev_dead = self.handler_locals, self.dead_locals
            self.model_scope = (node.name, field_tys)
            self.handler_locals = set(p for p, _ in params)
            prev_typed = self.typed_locals
            self.typed_locals = {p: t for p, t in params}
            self.dead_locals = {}
            prev_ret, self.ret_ty = self.ret_ty, ret
            try:
                stmts = []
                for st in (m.body[:-1] if ret is not None else m.body):
                    stmts += self._stmt(st, None)
                if ret is not None:
                    stmts.append(self.expr(m.body[-1].value, "store", None))
            finally:
                self.ret_ty = prev_ret
                self.model_scope = prev_scope
                self.handler_locals, self.dead_locals = prev_locals, prev_dead
                self.typed_locals = prev_typed
            sig = ", ".join(f"{p}: {t}" for p, t in params)
            emitted = self._smeth(node.name, m.name)
            tail = f" {ret}" if ret is not None else ""
            kw = "async fn" if self.async_hit else "fn"
            self.async_hit = False
            head = f"  {kw} {emitted}({sig}){tail} {{" if params else f"  {kw} {emitted}{tail} {{"
            self.stores[node.name]["methods"].append([head, *[f"    {l}" for l in stmts], "  }"])
            self.stores[node.name]["method_names"].add(m.name)

    def _method_ret(self, m, kind: str):
        """The pixie return type of a store/model method, or None for a
        method that returns nothing. A returning method ends with
        `return <expression>`: pixie answers a method with its last
        expression, and an early return inside a branch has nowhere to
        go."""
        if m.returns is None or (isinstance(m.returns, ast.Constant) and m.returns.value is None):
            return None
        ret = self._param_ty(m.returns)
        if ret is None:
            raise Untranslatable(
                m.returns,
                f"a {kind} method returns int, float, str, bool, a list of those, a value class or an enum — "
                f"`{ast.unparse(m.returns)}` is not in the dialect yet",
            )
        if not m.body or not isinstance(m.body[-1], ast.Return) or m.body[-1].value is None:
            raise Untranslatable(
                m, f"`{m.name}` returns {py_ty(ret)}, so its body ends with `return <expression>` — "
                "an early `return` inside a branch is not in the dialect yet")
        return ret

    @staticmethod
    def _selfless(node, sname: str):
        """A copy of a property's expression with `self` renamed to the
        store, so the same text reads from a view and from a handler."""
        out = copy.deepcopy(node)
        for n in ast.walk(out):
            if isinstance(n, ast.Name) and n.id == "self":
                n.id = sname
        return out

    def _take_store_prop(self, sname: str, m):
        """`@property` on a store: a name for a formula over the
        store's fields. Building a view only reads state, so a property
        is written where it is read — which makes its body a single
        `return <expression>`."""
        if len(m.args.args) != 1 or m.args.args[0].arg != "self":
            raise Untranslatable(m, f"a `@property` takes `self` alone — `{m.name}` has parameters, so it is a method")
        if not (len(m.body) == 1 and isinstance(m.body[0], ast.Return) and m.body[0].value is not None):
            raise Untranslatable(m, f"a `@property` is a single `return <expression>` over the store's fields — `{m.name}` is read where it is written, so a statement has nowhere to run")
        self.stores[sname]["props"][m.name] = self._selfless(m.body[0].value, sname)

    def _take_store_static(self, sname: str, m):
        """`@staticmethod` on a store: a plain function that happens to
        live in the class, so it becomes the static a module-level
        helper becomes — callable from views as well as handlers."""
        fn = copy.copy(m)
        fn.decorator_list = []
        fn.name = f"{sname}_{m.name}"
        self._take_helper(fn)
        self.stores[sname]["statics"][m.name] = fn.name

    ACCESSOR_PREFIXES = ("set_", "push_", "insert_")

    def _accessor_taken(self, field_tys) -> set:
        """Names pixie already generates for a class's props: the
        getter (`x`) plus set_x/push_x/insert_x. A user method with
        one of these names is emitted with a trailing underscore —
        the collision is pixie-internal, the Python surface keeps the
        natural name (writing `set_filter` beside a `filter` field is
        ordinary Python)."""
        taken = set(field_tys)
        for f in field_tys:
            for p in self.ACCESSOR_PREFIXES:
                taken.add(p + f)
        return taken

    def _smeth(self, sname: str, meth: str) -> str:
        if meth in self._accessor_taken(self.stores[sname]["field_tys"]):
            return meth + "_"
        return meth

    def _mmeth(self, mname: str, meth: str) -> str:
        tys = {f for f, _, _ in self.models[mname]["fields"]}
        if meth in self._accessor_taken(tys):
            return meth + "_"
        return meth

    def _store_field_ty(self, ann, default):
        """(pix type, literal) for a store field — the cell type
        grammar, annotated directly (`total: int = 0`)."""
        mref = self._model_ref_ann(ann)
        if mref is not None:
            mname2, weak2 = mref
            if weak2:
                raise Untranslatable(ann, "a `Weak` field lives on a model — a store owns what it holds, so its references are strong")
            if not (isinstance(default, ast.Constant) and default.value is None):
                raise Untranslatable(default, "a model-reference field starts as None — wire it in a handler")
            return (f"{mname2}?", "nil")
        lref = self._model_list_ann(ann)
        if lref is not None:
            if not (isinstance(default, ast.List) and not default.elts):
                raise Untranslatable(default, "a model-list field starts as `[]`")
            return (f"List<{lref}>", "[]")
        if isinstance(ann, ast.Name) and ann.id in self.structs:
            if not (isinstance(default, ast.Call) and isinstance(default.func, ast.Name) and default.func.id == ann.id):
                raise Untranslatable(default, f"a {ann.id} field starts from a {ann.id}(...) literal")
            return ann.id, self._struct_ctor(ann.id, default, "store", None)
        if isinstance(ann, ast.Name) and ann.id in self.unions:
            if not (
                isinstance(default, ast.Call)
                and isinstance(default.func, ast.Name)
                and self.union_of.get(default.func.id) == ann.id
            ):
                raise Untranslatable(default, f"a {ann.id} field starts from one of its variants")
            return ann.id, self._variant_ctor(default)
        if isinstance(ann, ast.Name) and ann.id in self.enums:
            if not (
                isinstance(default, ast.Attribute)
                and isinstance(default.value, ast.Name)
                and default.value.id == ann.id
                and default.attr in self.enums[ann.id]
            ):
                raise Untranslatable(default, f"a {ann.id} field starts from a {ann.id}.MEMBER literal")
            return ann.id, f"{ann.id}.{default.attr}"
        if isinstance(ann, ast.BinOp) and isinstance(ann.op, ast.BitOr):
            base = None
            for side, other in ((ann.left, ann.right), (ann.right, ann.left)):
                if isinstance(other, ast.Constant) and other.value is None and isinstance(side, ast.Name):
                    base = self.HELPER_TY.get(side.id)
            if base is None:
                wider = self._pix_ty(ann)
                if wider is None:
                    raise Untranslatable(ann, "an optional field is a scalar, a list, a dict, a value class, an Enum or a model reference, each `| None`")
                return wider, self._literal_of(default, wider)
            if isinstance(default, ast.Constant) and default.value is None:
                return f"{base}?", "nil"
            got, lit = self._state_field(default)
            if got != base:
                raise Untranslatable(default, f"the default is {self._py_ty(got)}, the annotation says `{base} | None`")
            return f"{base}?", lit
        if isinstance(ann, ast.Name):
            ty = self.HELPER_TY.get(ann.id)
            if ty is None:
                raise Untranslatable(ann, f"`{ann.id}` is not a store field type — int/float/str/bool, list[...] or dict[str, ...] of those, an Enum, a value class, a sum type, `T | None` or a model reference")
            got, lit = self._state_field(default)
            if got != ty:
                raise Untranslatable(default, f"the default is {self._py_ty(got)}, the annotation says {self._py_ty(ty)}")
            return ty, lit
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.value, ast.Name)
            and ann.value.id == "list"
            and isinstance(ann.slice, ast.Name)
        ):
            inner = self._pix_ty(ann.slice)
            if inner is None:
                raise Untranslatable(ann, f"list[{self._src(ann.slice)}] is not a field type — list elements are int, float, str, bool, a value class, an Enum, or a list or dict of those")
            return f"List<{inner}>", self._list_literal(default, inner)
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.value, ast.Name)
            and ann.value.id == "dict"
            and isinstance(ann.slice, ast.Tuple)
            and len(ann.slice.elts) == 2
        ):
            key = self._pix_ty(ann.slice.elts[0])
            if key not in ("String", "Int"):
                raise Untranslatable(ann, "a dict keys by str or int")
            inner = self._pix_ty(ann.slice.elts[1])
            if inner is None:
                raise Untranslatable(ann, f"dict[{self._src(ann.slice.elts[0])}, {self._src(ann.slice.elts[1])}] is not a field type — dict values are int, float, str, bool, a value class, an Enum, or a list or dict of those")
            return f"Map<{key}, {inner}>", self._dict_literal(default, inner)
        wider = self._pix_ty(ann)
        if wider is not None:
            return wider, self._literal_of(default, wider)
        raise Untranslatable(ann, "a store field is int/float/str/bool, lists and str- or int-keyed dicts of those, a value class, an Enum, a sum type, `T | None` of any of them, or a model reference")

    def _model_ref_ann(self, ann):
        """A model-reference annotation: `M | None` → (M, False),
        `Weak[M]` → (M, True), else None. A string annotation (the
        pre-3.14 forward-reference habit) parses to the same shapes."""
        if isinstance(ann, ast.Constant) and isinstance(ann.value, str):
            try:
                ann = ast.parse(ann.value, mode="eval").body
            except SyntaxError:
                return None
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.slice, ast.Name)
            and ann.slice.id in self.model_names
        ):
            v = ann.value
            if (isinstance(v, ast.Name) and self.yokan_names.get(v.id) == "Weak") or (
                isinstance(v, ast.Attribute)
                and v.attr == "Weak"
                and isinstance(v.value, ast.Name)
                and v.value.id == self.ui
            ):
                return (ann.slice.id, True)
        if isinstance(ann, ast.BinOp) and isinstance(ann.op, ast.BitOr):
            for a, b in ((ann.left, ann.right), (ann.right, ann.left)):
                if (
                    isinstance(b, ast.Constant)
                    and b.value is None
                    and isinstance(a, ast.Name)
                    and a.id in self.model_names
                ):
                    return (a.id, False)
        return None

    def _model_list_ann(self, ann):
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.value, ast.Name)
            and ann.value.id == "list"
            and isinstance(ann.slice, ast.Name)
            and ann.slice.id in self.model_names
        ):
            return ann.slice.id
        return None

    def _take_model(self, node: ast.ClassDef):
        """`@ui.model` ↔ pixie class: an observed object. Python
        objects and pixie handles are both references, so identity
        semantics agree from the start; fields become props (defaults
        required), methods become World-threaded pixie methods, and a
        Protocol base routes its methods into an `impl` block."""
        fields = []
        methods = []
        weak_fields = set()
        for st in node.body:
            if isinstance(st, ast.AnnAssign) and isinstance(st.target, ast.Name):
                if st.value is None:
                    raise Untranslatable(st, "a model field needs a default (instances are constructed with them)")
                mref = self._model_ref_ann(st.annotation)
                if mref is not None:
                    mname2, weak2 = mref
                    if not (isinstance(st.value, ast.Constant) and st.value.value is None):
                        raise Untranslatable(st.value, "a model-reference field starts as None — wire it in a handler")
                    fields.append((st.target.id, f"{mname2}?", "nil"))
                    if weak2:
                        weak_fields.add(st.target.id)
                    continue
                lref = self._model_list_ann(st.annotation)
                if lref is not None:
                    if not (isinstance(st.value, ast.List) and not st.value.elts):
                        raise Untranslatable(st.value, "a model-list field starts as `[]`")
                    fields.append((st.target.id, f"List<{lref}>", "[]"))
                    continue
                ty = self._pix_ty(st.annotation)
                if ty is None:
                    raise Untranslatable(
                        st.annotation or st,
                        "a model field is int/float/str/bool, lists and dicts of those, a value class, an Enum, `M | None`, `Weak[M]` or list[M]",
                    )
                self._check_name(st.target, st.target.id, "model field")
                fields.append((st.target.id, ty, self._literal_of(st.value, ty)))
            elif isinstance(st, ast.FunctionDef):
                methods.append(st)
            elif isinstance(st, (ast.Pass, ast.Expr)):
                continue
            else:
                raise Untranslatable(st, "a @model body holds annotated fields and methods")
        if not fields:
            raise Untranslatable(node, "a model needs at least one field")
        traits = [b.id for b in node.bases if isinstance(b, ast.Name) and b.id in self.protocols]
        trait_sigs = {m: (ps, r, t) for t in traits for m, ps, r in self.protocols[t]}
        self.models[node.name] = {"fields": fields, "methods": [], "impls": {}, "traits": traits, "weak": weak_fields, "ret_tys": {}, "params": {}, "defaults": {}}
        field_tys = {f: t for f, t, _ in fields}
        for m in methods:
            if not m.args.args or m.args.args[0].arg != "self":
                raise Untranslatable(m, "a model method takes `self` first")
            params = []
            for a in m.args.args[1:]:
                if a.annotation is None:
                    raise Untranslatable(a, "annotate every model method parameter")
                ty = self._param_ty(a.annotation)
                if ty is None:
                    raise Untranslatable(
                        a.annotation,
                        "model method parameters take int/float/str/bool, list[...] of those, a value class, or an enum",
                    )
                params.append((a.arg, ty))
            self.models[node.name]["params"][m.name] = params
            self.models[node.name]["defaults"][m.name] = self._defaults_of(m)
            prev_scope = self.model_scope
            prev_locals, prev_dead = self.handler_locals, self.dead_locals
            self.model_scope = (node.name, field_tys)
            self.handler_locals = set(p for p, _ in params)
            self.dead_locals = {}
            try:
                if m.name in trait_sigs:
                    want_ps, want_ret, tname = trait_sigs[m.name]
                    if [t for _, t in params] != [t for _, t in want_ps]:
                        raise Untranslatable(m, f"`{m.name}` does not match the protocol's parameters")
                    if not m.body or not isinstance(m.body[-1], ast.Return) or m.body[-1].value is None:
                        raise Untranslatable(m, "a protocol method ends with `return <expr>`")
                    pre = []
                    for st in m.body[:-1]:
                        pre += self._stmt(st, None)
                    body = self.expr(m.body[-1].value, "store", None)
                    sig = ", ".join(f"{p}: {t}" for p, t in params)
                    head = f"  pub fn {m.name}({sig}) {want_ret} {{" if params else f"  pub fn {m.name} {want_ret} {{"
                    self.models[node.name]["impls"].setdefault(tname, []).append(
                        [head, *[f"    {l}" for l in pre], f"    {body}", "  }"]
                    )
                else:
                    ret = self._method_ret(m, "model")
                    if ret is not None:
                        self.models[node.name]["ret_tys"][m.name] = ret
                    stmts = []
                    for st in (m.body[:-1] if ret is not None else m.body):
                        stmts += self._stmt(st, None)
                    if ret is not None:
                        stmts.append(self.expr(m.body[-1].value, "store", None))
                    sig = ", ".join(f"{p}: {t}" for p, t in params)
                    emitted = self._mmeth(node.name, m.name)
                    rtail = f" {ret}" if ret is not None else ""
                    head = f"  pub fn {emitted}({sig}){rtail} {{" if params else f"  pub fn {emitted}{rtail} {{"
                    self.models[node.name]["methods"].append([head, *[f"    {l}" for l in stmts], "  }"])
            finally:
                self.model_scope = prev_scope
                self.handler_locals, self.dead_locals = prev_locals, prev_dead

    def _take_struct(self, node: ast.ClassDef):
        """`@dataclass(frozen=True)` ↔ pixie struct. Frozen is the
        admission ticket: immutability makes Python's reference
        aliasing unobservable, so value semantics agree by
        construction. Updates spell as dataclasses.replace."""
        fields = []
        methods = []
        seen_default = False
        for st in node.body:
            if isinstance(st, ast.AnnAssign) and isinstance(st.target, ast.Name):
                # A field may be another value class — declared
                # EARLIER, so the nesting is grounded (pixie structs
                # nest freely; the twin rule rides this) — a list, a
                # dict, an Enum or an optional of any of them.
                ty = self._pix_ty(st.annotation)
                if ty is None:
                    raise Untranslatable(st.annotation or st, "a value-class field is int, float, str, bool, a value class declared earlier, an Enum, or a list, dict or optional of those")
                self._check_name(st.target, st.target.id, "value-class field")
                lit = None
                if st.value is not None:
                    lit = self._literal_of(st.value, ty)
                    seen_default = True
                elif seen_default:
                    raise Untranslatable(st, "non-default fields precede default fields (Python's own rule)")
                fields.append((st.target.id, ty, lit))
            elif isinstance(st, (ast.Pass, ast.Expr)):
                continue
            elif isinstance(st, ast.FunctionDef):
                methods.append(st)
            else:
                raise Untranslatable(st, "a @value body holds annotated fields and methods")
        self.structs[node.name] = fields
        if methods:
            self._take_struct_methods(node.name, methods)

    DUNDER_OPS = {"__add__": "opAdd", "__sub__": "opSub", "__mul__": "opMul"}

    def _struct_ann_ty(self, ann, own: str):
        """A value-method annotation: the scalar four, or a value
        class — spelled bare or as a string (the forward-reference
        form Python itself needs for the class's own name)."""
        if isinstance(ann, ast.Constant) and isinstance(ann.value, str):
            name = ann.value
            if name == own or name in self.structs:
                return name
            return None
        if isinstance(ann, ast.Name):
            t = self.HELPER_TY.get(ann.id)
            if t:
                return t
            if ann.id == own or ann.id in self.structs:
                return ann.id
        return self._pix_ty(ann)

    def _take_struct_methods(self, sname: str, methods):
        """Methods on a value class, operator dunders included:
        `__add__`/`__sub__`/`__mul__` become the operator's meaning
        (`a + b` calls them in both tiers), and plain methods are
        callable from handlers. Bodies are a single `return expr` —
        a frozen value has nothing to assign."""
        table = {}
        for m in methods:
            if m.name in ("__init__", "__repr__", "__eq__", "__hash__"):
                raise Untranslatable(m, "a value class's constructor and equality are derived — its methods are data methods and the operator dunders `__add__`, `__sub__`, `__mul__`")
            emitted = self.DUNDER_OPS.get(m.name)
            if emitted is None:
                if m.name.startswith("__"):
                    raise Untranslatable(m, "the operator dunders are `__add__`, `__sub__` and `__mul__`")
                emitted = m.name
            if m.returns is None:
                raise Untranslatable(m, "annotate the value method's return type")
            ret = self._struct_ann_ty(m.returns, sname)
            if ret is None:
                raise Untranslatable(m.returns, "a value method returns int, float, str, bool or a value class")
            params = []
            for a in m.args.args[1:]:
                if a.annotation is None:
                    raise Untranslatable(a, "annotate every value method parameter")
                ty = self._struct_ann_ty(a.annotation, sname)
                if ty is None:
                    raise Untranslatable(a.annotation, "a value method's parameters are int, float, str, bool or a value class")
                params.append((a.arg, ty))
            if not (len(m.body) == 1 and isinstance(m.body[0], ast.Return) and m.body[0].value is not None):
                raise Untranslatable(m, "a value method is a single `return <expression>` — an immutable value has nothing to assign to")
            prev_scope, prev_locals, prev_typed = self.struct_self, self.handler_locals, self.typed_locals
            self.struct_self = sname
            self.handler_locals = set(p for p, _ in params)
            self.typed_locals = {p: t for p, t in params}
            try:
                body = self.expr(m.body[0].value, "store", None)
            finally:
                self.struct_self, self.handler_locals, self.typed_locals = prev_scope, prev_locals, prev_typed
            sig = ", ".join(f"{p}: {t}" for p, t in params)
            lines = [f"  fn {emitted}({sig}) {ret} {{" if params else f"  fn {emitted}() {ret} {{",
                     f"    {body}", "  }"]
            table[m.name] = (emitted, params, ret, lines)
        self.struct_methods[sname] = table

    def _take_protocol(self, node: ast.ClassDef):
        methods = []
        for st in node.body:
            if isinstance(st, ast.FunctionDef):
                if st.returns is None or not isinstance(st.returns, ast.Name):
                    raise Untranslatable(st, "annotate the protocol method's return type")
                ret = self.HELPER_TY.get(st.returns.id)
                if ret is None:
                    raise Untranslatable(st.returns, "a protocol method returns int, float, str or bool — other return types are not in the dialect yet")
                params = []
                for a in st.args.args[1:]:  # drop self
                    if a.annotation is None or not isinstance(a.annotation, ast.Name):
                        raise Untranslatable(a, "annotate every protocol parameter")
                    ty = self.HELPER_TY.get(a.annotation.id)
                    if ty is None:
                        raise Untranslatable(a.annotation, "a protocol method's parameters are int, float, str or bool — other parameter types are not in the dialect yet")
                    params.append((a.arg, ty))
                methods.append((st.name, params, ret))
            elif isinstance(st, (ast.Pass, ast.Expr)):
                continue
            else:
                raise Untranslatable(st, "a Protocol declares method stubs only")
        if not methods:
            raise Untranslatable(node, "a Protocol needs at least one method")
        self.protocols[node.name] = methods

    @staticmethod
    def _enum_values(node: ast.ClassDef) -> dict:
        """{member: value} for an Enum body, the way Python assigns
        them: an explicit literal is the value, `auto()` continues from
        the last int (starting at 1)."""
        out, last = {}, 0
        for st in node.body:
            if not (isinstance(st, ast.Assign) and len(st.targets) == 1 and isinstance(st.targets[0], ast.Name)):
                continue
            name, v = st.targets[0].id, st.value
            if isinstance(v, ast.Constant) and type(v.value) in (int, str) and type(v.value) is not bool:
                out[name] = v.value
                if type(v.value) is int:
                    last = v.value
            else:                      # auto(), or anything else the scan let through
                last += 1
                out[name] = last
        return out

    def _union_arm(self, p, uname: str):
        """`case Circle(r):` / `case Rect(w=w):` / `case Dot():` /
        `case _:` → a `when` arm plus its (binding, pixty) list.
        Wildcards fill unbound fields (the native pattern binds all
        positions)."""
        if isinstance(p, ast.MatchAs) and p.pattern is None and p.name is None:
            return "when _", []
        if not (isinstance(p, ast.MatchClass) and isinstance(p.cls, ast.Name) and self.union_of.get(p.cls.id) == uname):
            raise Untranslatable(p, f"a match arm is a variant pattern of {uname} (or `_`)")
        vname = p.cls.id
        fields = self.structs[vname]
        slots = ["_"] * len(fields)
        binds = []
        for i, sub in enumerate(p.patterns):
            if i >= len(fields):
                raise Untranslatable(sub, f"`{vname}` has {len(fields)} field(s)")
            if isinstance(sub, ast.MatchAs) and sub.pattern is None and sub.name is not None:
                slots[i] = sub.name
                binds.append((sub.name, fields[i][1]))
            elif isinstance(sub, ast.MatchAs) and sub.pattern is None and sub.name is None:
                pass
            else:
                raise Untranslatable(sub, "a variant pattern binds names or `_` — nested patterns and literals are not in the dialect yet")
        for kname, sub in zip(p.kwd_attrs, p.kwd_patterns):
            ix = next((j for j, (f, _t, _d) in enumerate(fields) if f == kname), None)
            if ix is None:
                raise Untranslatable(sub, f"`{kname}` is not a field of {vname}")
            if isinstance(sub, ast.MatchAs) and sub.pattern is None and sub.name is not None:
                slots[ix] = sub.name
                binds.append((sub.name, fields[ix][1]))
            else:
                raise Untranslatable(sub, "a variant pattern binds names or `_` — nested patterns and literals are not in the dialect yet")
        if fields:
            return f"when {vname}({', '.join(slots)})", binds
        return f"when {vname}", binds

    def _variant_ctor(self, call: ast.Call, ctx: str = "store", param=None) -> str:
        vname = call.func.id
        uname = self.union_of[vname]
        fields = self.structs[vname]
        by_kw = {}
        for kw in call.keywords:
            if kw.arg is None:
                raise Untranslatable(call, "`**` unpacking is not in the dialect — pass fields by name")
            by_kw[kw.arg] = kw.value
        vals = []
        for i, (f, _t, _d) in enumerate(fields):
            if i < len(call.args):
                vals.append(self.expr(call.args[i], ctx, param))
            elif f in by_kw:
                vals.append(self.expr(by_kw.pop(f), ctx, param))
            else:
                raise Untranslatable(call, f"variant field `{f}` missing")
        if by_kw or len(call.args) > len(fields):
            raise Untranslatable(call, f"`{vname}` takes {len(fields)} field(s)")
        if vals:
            return f"{uname}.{vname}({', '.join(vals)})"
        return f"{uname}.{vname}"

    def _struct_ctor(self, name: str, call: ast.Call, ctx: str, param) -> str:
        """`Point(3, y=4)` → positional pixie construction; omitted
        trailing fields must have defaults (Python's own call rule
        enforces the rest at tier A)."""
        fields = self.structs[name]
        by_kw = {}
        for kw in call.keywords:
            if kw.arg is None:
                raise Untranslatable(call, "`**` unpacking is not in the dialect — pass fields by name")
            by_kw[kw.arg] = kw.value
        def arg_of(node, fty):
            # A container or an optional field takes the literal shape
            # its type names; everything else is an ordinary
            # expression.
            if isinstance(node, (ast.List, ast.Dict)) or (
                isinstance(node, ast.Constant) and node.value is None
            ):
                return self._literal_of(node, fty)
            return self.expr(node, ctx, param)

        vals = []
        for i, (f, _ty, dflt) in enumerate(fields):
            if i < len(call.args):
                if f in by_kw:
                    raise Untranslatable(call, f"field `{f}` given twice")
                vals.append(arg_of(call.args[i], _ty))
            elif f in by_kw:
                vals.append(arg_of(by_kw.pop(f), _ty))
            elif dflt is not None:
                vals.append(None)  # trailing default, may omit
            else:
                raise Untranslatable(call, f"field `{f}` missing (no default)")
        if by_kw:
            raise Untranslatable(call, f"unknown field `{next(iter(by_kw))}`")
        while vals and vals[-1] is None:
            vals.pop()
        if any(v is None for v in vals):
            # a defaulted HOLE before a given value: fill from the default literal
            out = []
            for i, v in enumerate(vals):
                out.append(fields[i][2] if v is None else v)
            vals = out
        return f"{name}({', '.join(vals)})"


    def _take_helper(self, node: ast.FunctionDef):
        """A module-level def called from handler expressions: pure,
        every param annotated, a single `return <expr>` — it becomes a
        pixie free fn, so the computation is NATIVE in the compiled
        tier (an escape would stay interpreted)."""
        # The body is its own scope: a text hole around the CALL SITE
        # must not leak into how the body's locals lower.
        prev_hole = self.text_hole
        self.text_hole = False
        try:
            return self._take_helper_inner(node)
        finally:
            self.text_hole = prev_hole

    def _take_helper_inner(self, node: ast.FunctionDef):
        if node.decorator_list:
            raise Untranslatable(node, "a helper takes no decorators (a def with @component is a component, with @py an escape)")
        if node.returns is None:
            raise Untranslatable(node, "annotate the helper's return type — int, float, str, bool, list[...] of those, a value class or an enum")
        ret = self._param_ty(node.returns)
        if ret is None:
            raise Untranslatable(node.returns, f"a helper returns int, float, str, bool, list[...] of those, a value class or an enum — `{ast.unparse(node.returns)}` does not cross a helper yet")
        params = []
        bounds = []
        for a in node.args.args:
            if a.annotation is None:
                raise Untranslatable(a, f"helper parameter `{a.arg}` needs an annotation of int, float, str, bool, list[...] of those, a value class, an enum or a Protocol")
            if isinstance(a.annotation, ast.Name) and a.annotation.id in self.protocols:
                var = f"P{len(bounds)}"
                bounds.append((var, a.annotation.id))
                params.append((a.arg, var))
                continue
            ty = self._param_ty(a.annotation)
            if ty is None:
                raise Untranslatable(a.annotation, f"helper parameter `{a.arg}` is int, float, str, bool, list[...] of those, a value class, an enum or a Protocol — `{ast.unparse(a.annotation)}` does not cross a helper yet")
            params.append((a.arg, ty))
        if not node.body or not isinstance(node.body[-1], ast.Return) or node.body[-1].value is None:
            raise Untranslatable(node, "a helper's body ends with `return <expression>` — the value it answers")
        self.helpers[node.name] = (params, ret, None, bool(bounds))  # registered first: recursion works
        prev = self.helper_params
        prev_locals, prev_dead, prev_typed = self.handler_locals, self.dead_locals, self.typed_locals
        bound_of = {v: t for v, t in bounds}
        self.helper_params = {p: (bound_of.get(t, t) if t in bound_of else t) for p, t in params}
        self.handler_locals = set(p for p, _ in params)
        self.typed_locals = {p: t for p, t in params if t in ("Int", "Float", "String", "Bool") or t in self.structs or t in self.enums or t.startswith("List<")}
        self.dead_locals = {}
        prev_ret, self.ret_ty = self.ret_ty, ret
        try:
            stmts = []
            for st in node.body[:-1]:
                stmts += self._stmt(st, None)
            tail = self.expr(node.body[-1].value, "store", None)
        finally:
            self.ret_ty = prev_ret
            self.helper_params = prev
            self.handler_locals, self.dead_locals, self.typed_locals = prev_locals, prev_dead, prev_typed
        sig = ", ".join(f"{p}: {t}" for p, t in params)
        if bounds:
            # Protocol-bounded helpers stay FREE generic fns (statics
            # take no bounds) — handler-only, as before.
            gen = "<" + ", ".join(f"{v}: {t}" for v, t in bounds) + ">"
            lines = [f"fn {node.name}{gen}({sig}) {ret} {{", *[f"  {l}" for l in stmts], f"  {tail}", "}"]
        else:
            # Plain helpers become `static fn`s (§8.54): no receiver,
            # no World — the one callable a VIEW may evaluate.
            lines = [f"  pub static fn {node.name}({sig}) {ret} {{", *[f"    {l}" for l in stmts], f"    {tail}", "  }"]
        self.helpers[node.name] = (params, ret, lines, bool(bounds))

    def _take_escape(self, node: ast.FunctionDef):
        if len(node.decorator_list) != 1:
            raise Untranslatable(node, "@py is the sole decorator of an escape")
        if node.returns is None:
            raise Untranslatable(node, "annotate the @py function's return type (int/float/str/bool or list[...])")
        params = []
        for a in node.args.args:
            if a.annotation is None:
                raise Untranslatable(a, "annotate every @py parameter (int/float/str/bool or list[...])")
            params.append((a.arg, self._escape_ty(a.annotation)))
        pix_r, rust_r = self._escape_ty(node.returns)
        ret = (pix_r, self.PYRET.get(rust_r, rust_r).replace("Vec<&str>", "Vec<String>"))
        clean = ast.FunctionDef(
            name=node.name,
            args=node.args,
            body=node.body,
            decorator_list=[],
            returns=node.returns,
            type_params=[],
        )
        ast.copy_location(clean, node)
        ast.fix_missing_locations(clean)
        self.escapes[node.name] = {
            "params": params,
            "ret": ret,
            "src": ast.unparse(clean),
        }

    @staticmethod
    def _camel(name: str) -> str:
        parts = name.split("_")
        return parts[0] + "".join(p.title() for p in parts[1:])

    def _ann_literal(self, ann, default):
        """(type, pix literal) from an annotation + default expr."""
        if isinstance(ann, ast.Name) and ann.id not in self.HELPER_TY and self._pix_ty(ann) is not None:
            ty = self._pix_ty(ann)
            return ty, self._literal_of(default, ty)
        if isinstance(ann, ast.Name):
            ty = {"int": "Int", "str": "String", "bool": "Bool", "float": "Float"}.get(ann.id)
            if ty is None:
                raise Untranslatable(ann, f"`{ann.id}` is not a model field type — int/float/str/bool, list[...] of those, `M | None`, `Weak[M]` or list[M]; dicts and value classes on a model are not in the dialect yet")
            got, lit = self._state_field(default)
            if got == "Int" and ty == "Float":
                got, lit = "Float", repr(float(lit))
            if got != ty:
                raise Untranslatable(default, f"the default is {self._py_ty(got)}, the annotation says {self._py_ty(ty)}")
            return ty, lit
        if (
            isinstance(ann, ast.Subscript)
            and isinstance(ann.value, ast.Name)
            and ann.value.id == "list"
            and isinstance(ann.slice, ast.Name)
        ):
            inner = self._pix_ty(ann.slice)
            if inner is None:
                raise Untranslatable(ann, f"list[{self._src(ann.slice)}] is not a field type — list elements are int, float, str, bool, a value class, an Enum, or a list or dict of those")
            return f"List<{inner}>", self._list_literal(default, inner)
        wider = self._pix_ty(ann)
        if wider is not None:
            return wider, self._literal_of(default, wider)
        raise Untranslatable(ann, "a model field is int/float/str/bool, lists and dicts of those, a value class, an Enum, `M | None`, `Weak[M]` or list[M]")

    # ---- discovery ------------------------------------------------

    RESERVED = {
        "Text", "Button", "TextField", "Column", "Row", "Grid", "GridCell",
        "Stack", "ListView", "ScrollView", "HScrollView", "DataTable",
        "Modal", "Image", "Svg", "BarChart", "LineChart", "ProgressBar",
        "Spinner", "Main", "App", "Slot", "Helpers",
    }

    # Words the compiled side reads as syntax. A state, field, local or
    # parameter carrying one would be written out as itself and turn
    # into a parse error inside generated code, which is the compiler's
    # bug to prevent — so the name is refused here, with a reason.
    KEYWORDS = {
        "class", "struct", "enum", "flags", "prop", "signal", "slot", "fn",
        "init", "deinit", "test", "emit", "case", "when", "error", "async",
        "await", "use", "view", "style", "try", "if", "else", "for", "while",
        "break", "continue", "return", "let", "var", "self", "weak",
        "consuming", "escaping", "pub", "trait", "impl", "store", "suite",
        "static", "true", "false", "nil", "state", "of",
    }

    def _check_name(self, node, name: str, what: str):
        if name in self.KEYWORDS:
            raise Untranslatable(
                node,
                f"`{name}` is a word the compiled side reads as syntax, so a {what} "
                f"cannot carry it — rename it (`{name}_` reads the same in Python)",
            )
        return name

    def scan(self):
        for mod in self.modules.values():
            self._scan_body(mod.body, entry=False)
        self._scan_body(self.tree.body, entry=True)
        if self.ui is None and not self.yokan_names and not self.stdlib_mods and self.crates_name is None:
            raise Untranslatable(self.tree, "the app never imports yokan — `from yokan import State, column, …` (or `import yokan as ui`)")
        if self.view is None:
            raise Untranslatable(self.tree, 'no `run(view, ...)` call under the `if __name__ == "__main__":` guard')
        if self.state_node is not None and self.cells:
            raise Untranslatable(self.tree, "declare state with `State` — a `run(state={...})` dict alongside it is not in the dialect")

    def _scan_body(self, body, entry: bool):
        for node in body:
            if isinstance(node, (ast.Import, ast.ImportFrom)):
                self._scan_import(node)
        for node in body:
            if isinstance(node, ast.ClassDef) and any(
                self._is_deco(d, "model") for d in node.decorator_list
            ):
                self.model_names.add(node.name)
        for node in body:
            if isinstance(node, (ast.Import, ast.ImportFrom)):
                continue
            elif isinstance(node, ast.FunctionDef) and any(
                self._is_deco(d, "py") for d in node.decorator_list
            ):
                self._take_escape(node)
            elif isinstance(node, ast.FunctionDef):
                self.defs[node.name] = node
                if any(
                    self._is_ui(d, "component") for d in node.decorator_list
                ):
                    self.comp_defs.add(node.name)
                for d in node.decorator_list:
                    if isinstance(d, ast.Call) and self._is_ui(d.func, "component"):
                        self.comp_defs.add(node.name)
                        if any(
                            k.arg == "slots"
                            and isinstance(k.value, ast.Constant)
                            and k.value.value is True
                            for k in d.keywords
                        ):
                            self.comp_slotted.add(node.name)
            elif isinstance(node, ast.ClassDef) and any(
                (
                    isinstance(d, ast.Call)
                    and isinstance(d.func, ast.Name)
                    and d.func.id in self.dataclass_names
                    and any(k.arg == "frozen" and isinstance(k.value, ast.Constant) and k.value.value is True for k in d.keywords)
                )
                or self._is_ui(d, "value")
                for d in node.decorator_list
            ):
                if any(
                    (isinstance(b, ast.Name) and b.id in self.enum_bases)
                    or (isinstance(b, ast.Attribute) and b.attr == "Enum")
                    for b in node.bases
                ):
                    # `@value class X(Enum)`: the crate-enum twin —
                    # the decorator registers it for the interpreted
                    # run; here it is an enum like any other.
                    members = [
                        st.targets[0].id
                        for st in node.body
                        if isinstance(st, ast.Assign)
                        and len(st.targets) == 1
                        and isinstance(st.targets[0], ast.Name)
                    ]
                    if not members:
                        raise Untranslatable(node, "an enum needs at least one member")
                    self.enums[node.name] = members
                    self.enum_values[node.name] = self._enum_values(node)
                else:
                    self._take_struct(node)
            elif isinstance(node, ast.ClassDef) and any(
                (isinstance(d, ast.Name) and d.id in self.dataclass_names)
                or (isinstance(d, ast.Call) and isinstance(d.func, ast.Name) and d.func.id in self.dataclass_names)
                for d in node.decorator_list
            ):
                self.unfrozen.add(node.name)
            elif isinstance(node, ast.TypeAlias) and isinstance(node.name, ast.Name):
                parts = []
                def _walk_union(x):
                    if isinstance(x, ast.BinOp) and isinstance(x.op, ast.BitOr):
                        _walk_union(x.left)
                        _walk_union(x.right)
                    elif isinstance(x, ast.Name) and x.id in self.structs:
                        parts.append(x.id)
                    else:
                        raise Untranslatable(x, "a `type` alias joins value classes into a sum type (`type Shape = Circle | Rect`)")
                _walk_union(node.value)
                uname = node.name.id
                for p2 in parts:
                    if p2 in self.union_of:
                        raise Untranslatable(node, f"`{p2}` already belongs to the sum type {self.union_of[p2]}")
                    for _f, _t, dflt in self.structs[p2]:
                        if dflt is not None:
                            raise Untranslatable(node, f"the fields of variant `{p2}` take no defaults — each arm and constructor names every field")
                    self.union_of[p2] = uname
                self.unions[uname] = parts
            elif isinstance(node, ast.ClassDef) and any(
                (isinstance(b, ast.Name) and b.id in self.enum_bases)
                or (isinstance(b, ast.Attribute) and b.attr == "Enum")
                for b in node.bases
            ):
                members = []
                for st in node.body:
                    if isinstance(st, ast.Assign) and len(st.targets) == 1 and isinstance(st.targets[0], ast.Name):
                        members.append(st.targets[0].id)
                    elif isinstance(st, (ast.Pass, ast.Expr)):
                        continue
                    else:
                        raise Untranslatable(st, "an Enum body holds members only (`NAME = auto()`, `NAME = 1`) — methods on an Enum are not in the dialect yet")
                if not members:
                    raise Untranslatable(node, "an enum needs at least one member")
                self.enums[node.name] = members
                self.enum_values[node.name] = self._enum_values(node)
            elif isinstance(node, ast.ClassDef) and any(
                isinstance(b, ast.Name) and b.id in self.protocol_names for b in node.bases
            ):
                self._take_protocol(node)
            elif isinstance(node, ast.ClassDef) and any(
                self._is_deco(d, "store") for d in node.decorator_list
            ):
                self._take_store(node)
            elif isinstance(node, ast.ClassDef) and any(
                self._is_deco(d, "model")
                for d in node.decorator_list
            ):
                self._take_model(node)
            elif (
                isinstance(node, ast.Assign)
                and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and isinstance(node.value, ast.Call)
                and self._is_ui(node.value.func, "style")
            ):
                self._take_style(node.targets[0].id, node.value)
            elif (
                isinstance(node, ast.Assign)
                and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and isinstance(node.value, ast.BinOp)
                and isinstance(node.value.op, ast.BitOr)
                and isinstance(node.targets[0], ast.Name)
                and any(isinstance(x, ast.Name) and x.id in self.styles for x in [node.value.left])
            ):
                self._take_style_merge(node.targets[0].id, node.value)
            elif (
                isinstance(node, ast.Assign)
                and len(node.targets) == 1
                and isinstance(node.targets[0], ast.Name)
                and isinstance(node.value, ast.Call)
                and isinstance(node.value.func, ast.Name)
                and node.value.func.id in self.models
            ):
                call = node.value
                if call.args or call.keywords:
                    raise Untranslatable(
                        call, "a model is constructed with its defaults (`Node()`) — constructor arguments are not in the dialect yet; set fields in a handler"
                    )
                self.model_instances[node.targets[0].id] = call.func.id
            elif isinstance(node, ast.AnnAssign) and self._is_state_call(node.value):
                if not isinstance(node.target, ast.Name):
                    raise Untranslatable(node, "a State is declared as a module-level name (`count: State[int] = State(0)`)")
                if node.target.id in self.cells:
                    raise Untranslatable(node, f"state `{node.target.id}` is declared in two modules — rename one")
                self._check_name(node.target, node.target.id, "state")
                field = self._cell_field(node.value, node.annotation)
                self.state[node.target.id] = field
                self.cells[node.target.id] = field[0]
            elif isinstance(node, ast.Assign) and self._is_state_call(node.value):
                if len(node.targets) != 1 or not isinstance(node.targets[0], ast.Name):
                    raise Untranslatable(node, "a State is declared as a module-level name (`count: State[int] = State(0)`)")
                self._check_name(node.targets[0], node.targets[0].id, "state")
                field = self._cell_field(node.value, None)
                self.state[node.targets[0].id] = field
                self.cells[node.targets[0].id] = field[0]
            elif isinstance(node, ast.If) and self._is_main_guard(node):
                # An imported module's guard body runs in neither run
                # (its __name__ is never "__main__"), so only the
                # entry's guard is read.
                if entry:
                    self._scan_main_guard(node)
            elif isinstance(node, ast.If) and self._is_type_checking_guard(node):
                continue   # dead at runtime in both runs
            elif (
                isinstance(node, ast.Expr)
                and isinstance(node.value, ast.Call)
                and self._is_ui(node.value.func, "every")
            ):
                # A timer is a declaration: it says the app ticks, and
                # both runs start it when the app starts.
                self._take_timer(node.value)
            elif isinstance(node, (ast.Assign, ast.AnnAssign)) and self._is_const_expr(node.value):
                # A literal constant is a declaration: nothing runs.
                for t in (node.targets if isinstance(node, ast.Assign) else [node.target]):
                    if isinstance(t, ast.Name):
                        self.consts[t.id] = node.value
            elif isinstance(node, ast.AnnAssign) and node.value is None:
                continue   # a bare annotation declares nothing that runs
            elif isinstance(node, ast.ClassDef):
                continue   # a plain class: declared here, refused where it is used
            elif isinstance(node, ast.Pass):
                continue
            elif isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant):
                continue   # docstring
            elif isinstance(node, ast.Expr) and self._is_sys_path_call(node.value):
                continue   # import plumbing (`sys.path.insert(0, …)`), effect-free for the app
            else:
                self._refuse_module_stmt(node, under_guard=False)

    # Module level is declarations. The module runs once per import in
    # CPython, again on every live reload, and never in the compiled
    # app, so a statement whose effect depends on which of those
    # happened cannot be verified by the gate; startup work has one
    # place, `run(view, on_start=…)`, which both runs call once.
    DECLS = (
        "imports, State, classes, defs, style(), type aliases, literal "
        "constants and the __main__ guard"
    )

    def _scan_main_guard(self, node: ast.If):
        for stmt in node.body:
            call = stmt.value if isinstance(stmt, ast.Expr) else None
            if isinstance(call, ast.Call) and self._is_ui(call.func, "run"):
                self._take_run(call)
            elif isinstance(call, ast.Call) and self._is_ui(call.func, "every"):
                self._take_timer(call)
            elif isinstance(stmt, ast.Pass) or (
                isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Constant)
            ):
                continue
            else:
                self._refuse_module_stmt(stmt, under_guard=True)
        if node.orelse:
            raise Untranslatable(
                node.orelse[0],
                "the __main__ guard takes no else branch: the compiled app reads the "
                "module's declarations and does not execute it; put startup work in a "
                "def and pass it as run(view, on_start=setup)",
            )

    def _take_timer(self, call: ast.Call):
        """`every(seconds, tick)` at module level: the period and the
        function to run. It is a declaration — the compiled app reads
        it and starts the timer with the app, exactly as importing the
        module does during development."""
        kw = {k.arg: k.value for k in call.keywords if k.arg}
        args = list(call.args)
        secs = args[0] if args else kw.get("seconds")
        fn = args[1] if len(args) > 1 else kw.get("on_tick")
        if secs is None or fn is None:
            raise Untranslatable(call, "every(seconds, on_tick) takes the period and the function to run")
        neg = isinstance(secs, ast.UnaryOp) and isinstance(secs.op, ast.USub)
        c = secs.operand if neg else secs
        if neg or not (isinstance(c, ast.Constant) and type(c.value) in (int, float) and type(c.value) is not bool and c.value > 0):
            raise Untranslatable(secs, "every()'s period is a positive number of seconds, written out (`every(1.0, tick)`)")
        self.timers.append((float(c.value) * 1000.0, fn, call))

    def _refuse_module_stmt(self, node, under_guard: bool):
        call = node.value if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call) else None
        if call is not None and self._is_ui(call.func, "run"):
            raise Untranslatable(
                node,
                "run(...) belongs under the `if __name__ == \"__main__\":` guard — the "
                "gate and the live reload import the module without running it",
            )
        what = self._describe_stmt(node)
        if under_guard:
            raise Untranslatable(
                node,
                f"under the __main__ guard, only run(...) is compiled: {what} there runs "
                "for `python app.py` but not in the compiled app; put startup work in a "
                "def and pass it as run(view, on_start=setup)",
            )
        if isinstance(node, ast.If):
            raise Untranslatable(
                node,
                "a module-level `if` is not compiled: only the `if __name__ == "
                "\"__main__\":` guard is read at module level; make the choice in a "
                "handler or in run(view, on_start=setup)",
            )
        if isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
            raise Untranslatable(
                node,
                f"{what} is computed when the module runs, and the compiled app does "
                "not run it: a module-level assignment declares a State, a style(), a "
                "model instance or a literal constant; compute the value in "
                "run(view, on_start=setup) and keep it in a State",
            )
        raise Untranslatable(
            node,
            f"a module-level {what} is not compiled: the compiled app reads the "
            f"module's declarations ({self.DECLS}) and does not execute it; put "
            "startup work in a def and pass it as run(view, on_start=setup)",
        )

    @staticmethod
    def _describe_stmt(node) -> str:
        if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
            return f"`{ast.unparse(node.value.func)}(...)` call"
        if isinstance(node, ast.Expr):
            return "expression statement"
        if isinstance(node, ast.Assign):
            return f"`{' = '.join(ast.unparse(t) for t in node.targets)} = ...`"
        if isinstance(node, ast.AnnAssign):
            return f"`{ast.unparse(node.target)}: {ast.unparse(node.annotation)} = ...`"
        if isinstance(node, ast.AugAssign):
            return f"`{ast.unparse(node.target)} {ast.unparse(node.op)}= ...`"
        kw = {
            ast.For: "for", ast.While: "while", ast.With: "with", ast.Try: "try",
            ast.TryStar: "try", ast.Raise: "raise", ast.Assert: "assert",
            ast.Delete: "del", ast.Global: "global", ast.Nonlocal: "nonlocal",
            ast.Match: "match", ast.If: "if", ast.AsyncFunctionDef: "async def",
            ast.AsyncFor: "async for", ast.AsyncWith: "async with",
            ast.Import: "import", ast.ImportFrom: "import",
            ast.FunctionDef: "def", ast.ClassDef: "class",
        }.get(type(node))
        return f"`{kw}` statement" if kw else f"{type(node).__name__} statement"

    @staticmethod
    def _py_ty(t) -> str:
        """The user-facing spelling of an internal type, for messages."""
        return py_ty(t)

    @staticmethod
    def _src(node, limit: int = 60) -> str:
        try:
            src = ast.unparse(node).splitlines()[0]
        except Exception:
            return type(node).__name__
        return src if len(src) <= limit else src[: limit - 1] + "…"

    STR_METHODS = (
        "upper", "lower", "strip", "lstrip", "rstrip", "split", "join", "startswith",
        "endswith", "replace", "find", "index", "count", "title", "capitalize",
        "isdigit", "isalpha", "format", "zfill", "center", "ljust", "rjust",
    )
    BUILTIN_HINTS = {
        "str": "`str(x)` is not in the dialect yet — render the value in an f-string (`f\"{x}\"`)",
        "int": "`int(...)` is not in the dialect yet — parse text with `strings.to_int(s, default)` (int of a float is planned)",
        "float": "`float(...)` is not in the dialect yet — parse text with `strings.to_float(s, default)`",
        "bool": "`bool(x)` is not in the dialect yet — write the comparison (`x != 0`)",
        "len": "len() takes a list or dict state read or a store field",
    }

    def _unknown_expr(self, node) -> str:
        if isinstance(node, ast.Name) and node.id in self.consts:
            return (
                f"module constant `{node.id}` cannot be read here yet — write the "
                "literal, or hold the value in a State"
            )
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr in self.STR_METHODS:
            return (
                f"str methods such as `.{node.func.attr}()` are not in the dialect yet — "
                "keep the text as it is, and parse numbers with `strings.to_int`"
            )
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
            hint = self.BUILTIN_HINTS.get(node.func.id)
            if hint:
                return hint
            if node.func.id in ("round", "abs", "min", "max", "sum", "sorted", "reversed",
                                "enumerate", "zip", "list", "dict", "set", "tuple", "range"):
                return f"`{node.func.id}(...)` is not in the dialect yet"
        if isinstance(node, (ast.ListComp, ast.SetComp, ast.DictComp, ast.GeneratorExp)):
            return "a comprehension is not in the dialect yet — build the list with a `for` loop and `xs.set(xs() + [x])`"
        if isinstance(node, ast.IfExp):
            return "a conditional expression (`a if c else b`) is not in the dialect yet — write an `if` / `else` statement"
        if isinstance(node, ast.NamedExpr):
            return "the walrus here is not in the dialect — it narrows an Optional (`if (v := x()) is not None:`); assign a local first"
        if isinstance(node, (ast.List, ast.Tuple, ast.Set, ast.Dict)):
            return "a list or dict literal is not in the dialect here — keep it in a State or a store field (`items.set([...])`)"
        if isinstance(node, ast.Lambda):
            return "a lambda as a value is not in the dialect — handlers take lambdas, expressions do not"
        if (
            isinstance(node, ast.Attribute)
            and node.attr in ("value", "name")
            and isinstance(node.value, ast.Attribute)
            and isinstance(node.value.value, ast.Name)
            and node.value.value.id in self.enums
        ):
            return f"`.{node.attr}` on an Enum member is not in the dialect yet — match on the member instead"
        return (
            f"`{self._src(node)}` is not in the dialect here — expressions are state "
            "reads, fields, locals, literals, arithmetic, comparisons, helper calls "
            "and method calls"
        )

    def _unknown_stmt(self, stmt) -> str:
        if isinstance(stmt, ast.Return):
            if self.helper_params is not None:
                return "an early `return` inside a helper is not in the dialect yet — end the helper with a single `return <expression>`"
            return "a `return` with a value is not in the dialect here — handlers and store methods return None; keep the result in a field or a State"
        if isinstance(stmt, ast.Assign) and isinstance(stmt.value, ast.List):
            return (
                "a local list needs its element type — annotate it "
                "(`out: list[str] = []`), which is what the compiled app reads"
            )
        if isinstance(stmt, (ast.Assign, ast.AnnAssign)) and isinstance(
            stmt.value, (ast.Dict, ast.DictComp)
        ):
            return "a local dict is not in the dialect yet — keep it in a State or a store field"
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Attribute)
            and isinstance(stmt.targets[0].value, ast.Name)
            and stmt.targets[0].value.id in self.stores
        ):
            t = stmt.targets[0]
            return (
                f"a store field is written through a method — `{t.value.id}.{t.attr} = ...` "
                f"from outside the store is not in the dialect; add a method "
                f"(`def set_{t.attr}(self, v) -> None: self.{t.attr} = v`)"
            )
        if isinstance(stmt, ast.Assign) and any(isinstance(t, (ast.Tuple, ast.List)) for t in stmt.targets):
            return "tuple assignment (`a, b = ...`) is not in the dialect yet — assign one name at a time"
        if isinstance(stmt, ast.Global):
            return "`global` is not needed — a handler writes module state through `x.set(v)`"
        if isinstance(stmt, ast.Raise):
            return "`raise` is not in the dialect yet — an uncaught failure already aborts its statement, and the app lives on"
        if isinstance(stmt, ast.Assert):
            return "`assert` is not in the dialect yet"
        if isinstance(stmt, ast.FunctionDef):
            return "a nested def is not in the dialect — define helpers at module level"
        if isinstance(stmt, ast.With):
            return "`with` inside a handler is not in the dialect"
        if isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Call):
            f = stmt.value.func
            if isinstance(f, ast.Name) and f.id == "print":
                return "`print(...)` writes to stdout, which is where a headless run\'s screen dump goes — `log(\"…\")` writes the same line to stderr in both runs"
            if isinstance(f, ast.Attribute) and f.attr in ("append", "extend", "pop", "remove", "insert", "clear", "sort", "reverse"):
                return (
                    f"in-place list methods such as `.{f.attr}()` are not in the dialect — "
                    "append with `items.set(items() + [x])`, clear with `items.set([])`"
                )
            if isinstance(f, ast.Attribute) and f.attr in self.STR_METHODS:
                return self._unknown_expr(stmt.value)
        return (
            f"`{self._src(stmt)}` is not a handler statement the dialect knows yet — "
            "handler statements are state writes (`x.set(v)`, `xs[i] = v`, "
            "`d[k] = v`), store and model calls, if/while/for/match, try, and locals"
        )

    def _unknown_cond(self, node) -> str:
        if isinstance(node, ast.Constant) and type(node.value) is bool:
            return "a constant condition (`while True:`) is not in the dialect yet — loop on a bool state or a comparison"
        if isinstance(node, ast.Compare) and len(node.ops) > 1:
            return "a chained comparison (`0 < x < 10`) is not in the dialect yet — write `0 < x and x < 10`"
        if isinstance(node, ast.Name):
            return "a bare local is not a condition yet — compare it explicitly"
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr in self.STR_METHODS:
            return self._unknown_expr(node)
        return (
            f"`{self._src(node)}` is not a condition — a condition is a bool state or "
            "field, or an explicit comparison (`if name() == \"\":`, not `if name():`); "
            "Python's truthiness is not in the dialect"
        )

    def _is_const_expr(self, e) -> bool:
        """A pure literal (nested containers, unary/binary ops over
        literals, an Enum member, a value-class literal, an earlier
        constant): binding it is a declaration, not behavior."""
        if e is None:
            return False
        if isinstance(e, ast.Constant):
            return True
        if isinstance(e, (ast.List, ast.Tuple, ast.Set)):
            return all(self._is_const_expr(x) for x in e.elts)
        if isinstance(e, ast.Dict):
            return all(k is not None and self._is_const_expr(k) for k in e.keys) and all(
                self._is_const_expr(v) for v in e.values
            )
        if isinstance(e, ast.UnaryOp):
            return self._is_const_expr(e.operand)
        if isinstance(e, ast.BinOp):
            return self._is_const_expr(e.left) and self._is_const_expr(e.right)
        if isinstance(e, ast.BoolOp):
            return all(self._is_const_expr(v) for v in e.values)
        if isinstance(e, ast.Compare):
            return self._is_const_expr(e.left) and all(self._is_const_expr(c) for c in e.comparators)
        if isinstance(e, ast.JoinedStr):
            return all(
                isinstance(v, ast.Constant) or (isinstance(v, ast.FormattedValue) and self._is_const_expr(v.value))
                for v in e.values
            )
        if isinstance(e, ast.Name):
            return e.id in self.consts
        if isinstance(e, ast.Attribute):
            return (
                isinstance(e.value, ast.Name)
                and e.value.id in self.enums
                and e.attr in self.enums[e.value.id]
            )
        if isinstance(e, ast.Call) and isinstance(e.func, ast.Name) and e.func.id in self.structs:
            return all(self._is_const_expr(a) for a in e.args) and all(
                self._is_const_expr(k.value) for k in e.keywords
            )
        return False

    @staticmethod
    def _is_sys_path_call(e) -> bool:
        f = e.func if isinstance(e, ast.Call) else None
        return (
            isinstance(f, ast.Attribute)
            and f.attr in ("insert", "append")
            and isinstance(f.value, ast.Attribute)
            and f.value.attr == "path"
            and isinstance(f.value.value, ast.Name)
            and f.value.value.id == "sys"
        )

    @staticmethod
    def _is_type_checking_guard(node: ast.If) -> bool:
        t = node.test
        return (isinstance(t, ast.Name) and t.id == "TYPE_CHECKING") or (
            isinstance(t, ast.Attribute) and t.attr == "TYPE_CHECKING"
        )

    @staticmethod
    def _is_main_guard(node: ast.If) -> bool:
        t = node.test
        return (
            isinstance(t, ast.Compare)
            and isinstance(t.left, ast.Name)
            and t.left.id == "__name__"
            and len(t.comparators) == 1
            and isinstance(t.comparators[0], ast.Constant)
            and t.comparators[0].value == "__main__"
        )

    def _is_ui(self, func, name: str) -> bool:
        if (
            isinstance(func, ast.Attribute)
            and func.attr == name
            and isinstance(func.value, ast.Name)
            and func.value.id == self.ui
        ):
            return True
        return isinstance(func, ast.Name) and self.yokan_names.get(func.id) == name

    def _is_deco(self, d, name: str) -> bool:
        """A decorator in either spelling — `@yokan.store` / `@ui.store`
        (alias) or bare `@store` (from-import), with or without call
        parentheses."""
        if self._is_ui(d, name):
            return True
        return isinstance(d, ast.Call) and self._is_ui(d.func, name)

    def _take_run(self, call: ast.Call):
        if not call.args:
            raise Untranslatable(call, "run(...) takes the view function first: `run(view, title=...)`")
        v = call.args[0]
        if not (isinstance(v, ast.Name) and v.id in self.defs):
            raise Untranslatable(call, "run(...) takes a module-level def as its view")
        self.view = self.defs[v.id]
        for kw in call.keywords:
            if kw.arg == "title":
                if not (isinstance(kw.value, ast.Constant) and isinstance(kw.value.value, str)):
                    raise Untranslatable(kw.value, "title takes a string literal")
                if '"' in kw.value.value or "\\" in kw.value.value:
                    raise Untranslatable(kw.value, "title cannot contain quotes or backslashes")
                self.window["title"] = kw.value.value
                continue
            if kw.arg in ("width", "height"):
                v = kw.value
                if not (isinstance(v, ast.Constant) and type(v.value) in (int, float) and type(v.value) is not bool and v.value > 0):
                    raise Untranslatable(v, f"{kw.arg} takes a positive number literal")
                self.window[kw.arg] = float(v.value)
                continue
            if kw.arg == "on_start":
                v = kw.value
                if (
                    isinstance(v, ast.Attribute)
                    and isinstance(v.value, ast.Name)
                    and v.value.id in self.stores
                ):
                    self.on_start = ("store", v.value.id, v.attr)
                elif isinstance(v, ast.Name) and v.id in self.defs:
                    self.on_start = ("def", v.id)
                else:
                    raise Untranslatable(
                        v, "on_start takes a store's bound method or a module-level def"
                    )
                callstr = self.handler(v, takes_text=False)
                self.handlers.append(("__start", None, [callstr]))
                continue
            if kw.arg == "state":
                raise Untranslatable(
                    kw.value,
                    "dict state (`run(state={...})`) runs in development only — the compiled app declares typed state (`count: State[int] = State(0)`), whose int writes are range-checked in both runs",
                )
            if False:
                if not isinstance(kw.value, ast.Dict):
                    raise Untranslatable(kw.value, "state must be a dict literal")
                self.state_node = kw.value
                for k, val in zip(kw.value.keys, kw.value.values):
                    if not (isinstance(k, ast.Constant) and isinstance(k.value, str)):
                        raise Untranslatable(k or kw.value, "state keys must be string literals")
                    key = k.value
                    if not key.isidentifier():
                        raise Untranslatable(k, f"state key `{key}` is not an identifier")
                    self.state[key] = self._state_field(val)

    @staticmethod
    def _pix_num(val, inner):
        if inner == "Float":
            return repr(float(val))
        return str(int(val))

    def _list_literal(self, node, inner) -> str:
        if not isinstance(node, ast.List):
            raise Untranslatable(node, 'a list starts from a list literal (`State([])`, `State(["a"])`)')
        parts = []
        for e in node.elts:
            if isinstance(e, ast.Constant) or inner not in ("Int", "Float", "String", "Bool"):
                parts.append(self._literal_of(e, inner))
                continue
            # An expression in a list literal: the handler reads it
            # where the literal stands, so it lowers as itself, and its
            # type has to be the list's.
            got = self._num_ty(e, "store", None)
            if got is not None and got != inner:
                raise Untranslatable(e, f"this list holds {py_ty(inner)} — `{self._src(e)}` is {py_ty(got)}")
            parts.append(self.expr(e, "store", None))
        return "[" + ", ".join(parts) + "]"

    def _dict_literal(self, node, inner) -> str:
        if not isinstance(node, ast.Dict):
            raise Untranslatable(node, 'a dict state starts from a dict literal (`State({})`, `State({"a": 1})`)')
        parts = []
        for k, v in zip(node.keys, node.values):
            if not (isinstance(k, ast.Constant) and type(k.value) is str):
                raise Untranslatable(k or node, "a dict literal's keys are str literals")
            lit = self._literal_of(v, inner)
            parts.append(f'"{esc(k.value)}": {lit}' if not k.value.isidentifier() else f"{k.value}: {lit}")
        if not parts:
            return "{}"
        return "{ " + ", ".join(parts) + " }"

    def _state_field(self, val):
        neg = isinstance(val, ast.UnaryOp) and isinstance(val.op, ast.USub)
        v = val.operand if neg else val
        if isinstance(v, ast.Constant):
            c = v.value
            if type(c) is bool and not neg:
                return ("Bool", "true" if c else "false")
            if type(c) is int:
                self._check_i64(c, v, neg)
                return ("Int", f"-{c}" if neg else str(c))
            if type(c) is float:
                return ("Float", f"-{c!r}" if neg else repr(c))
            if type(c) is str and not neg:
                return ("String", f'"{esc(c)}"')
        raise Untranslatable(
            val,
            "a default is a literal: int, float, str or bool",
        )

    # ---- expressions ----------------------------------------------

    # stdlib call → its fallible native twin (`!T` binding); the
    # translator routes a call here when it sits inside try/except
    FALLIBLE = {
        ("http", "get_text"): "Http.tryGetText",
        ("fs", "read_text"): "Fs.tryReadText",
        ("sqlite", "query_int"): "Sqlite.tryQueryInt",
    }

    STDLIB_CALLS = {
        ("fs", "read_text"): ("Fs", "readText", 1),
        ("fs", "write_text"): ("Fs", "writeText", 2),
        ("fs", "exists"): ("Fs", "exists", 1),
        ("sqlite", "exec"): ("Sqlite", "exec", 2),
        ("sqlite", "query_text"): ("Sqlite", "queryText", 2),
        ("sqlite", "query_int"): ("Sqlite", "queryInt", 2),
        ("strings", "to_int"): ("Strings", "toInt", 2),
        ("notify", "send"): ("Notify", "send", 2),
        ("strings", "to_float"): ("Strings", "toFloat", 2),
        ("fs", "read_text_or"): ("Fs", "readTextOr", 2),
        ("http", "get_text_or"): ("Http", "getTextOr", 2),
        ("sqlite", "query_int_or"): ("Sqlite", "queryIntOr", 3),
        ("sqlite", "query_text_or"): ("Sqlite", "queryTextOr", 2),
        ("random", "seed"): ("Random", "seed", 1),
        ("random", "int"): ("Random", "int_", 2),
        ("random", "float"): ("Random", "float_", 0),
        ("http", "get_text"): ("Http", "getText", 1),
        ("math", "sqrt"): ("Math", "sqrt", 1),
        ("math", "sin"): ("Math", "sin", 1),
        ("math", "cos"): ("Math", "cos", 1),
        ("math", "pow"): ("Math", "pow_", 2),
        ("math", "fabs"): ("Math", "fabs", 1),
        ("math", "floor"): ("Math", "floor", 1),
        ("math", "ceil"): ("Math", "ceil", 1),
        ("math", "pi"): ("Math", "pi", 0),
        ("json", "get_text"): ("Json", "getText", 2),
        ("json", "get_int"): ("Json", "getInt", 2),
        ("json", "get_float"): ("Json", "getFloat", 2),
        ("json", "get_bool"): ("Json", "getBool", 2),
        ("json", "length"): ("Json", "length", 2),
        ("json", "has"): ("Json", "has", 2),
        ("time", "now_ms"): ("Time", "nowMs", 0),
        ("time", "format_ms"): ("Time", "formatMs", 2),
        ("time", "sleep_ms"): ("Time", "sleepMs", 1),
    }

    def _check_crate_structs(self, node, crate, entry):
        """Every struct or enum a crate call crosses needs a matching
        twin in the app — a @value class (same fields, same order) or
        a @value Enum (same variant names) — so the interpreted run
        has a real Python value to hold it."""
        used = [t for t in list(entry[0]) + [entry[1]]]
        crate_structs = self.crate_info[crate].get("structs", {})
        disp = lambda ty: {"Int": "int", "Float": "float", "Bool": "bool", "String": "str"}.get(ty, ty)
        seen = set()

        def check_struct(t):
            if t in seen:
                return
            seen.add(t)
            cs = crate_structs[t]
            mine = self.structs.get(t)
            want = ", ".join(f"{f}: {disp(ty)}" for f, ty, _rf, _w in cs["fields"])
            if mine is None:
                raise Untranslatable(
                    node,
                    f"`{t}` crosses this call — declare its twin in the app: "
                    f"@value class {t}: {want}",
                )
            have = [(f, ty) for f, ty, _lit in mine]
            if have != [(f, ty) for f, ty, _rf, _w in cs["fields"]]:
                raise Untranslatable(
                    node,
                    f"@value class {t} does not match the crate's `{t}` — "
                    f"the crate declares ({want})",
                )
            # A nested field needs ITS twin too — the loader rebuilds
            # inner values by the same rule as outer ones.
            for _f, ty, _rf, _w in cs["fields"]:
                if ty in crate_structs and crate_structs[ty]["ok"] is True:
                    check_struct(ty)

        for t in used:
            cs = crate_structs.get(t)
            if cs is not None and cs["ok"] is True:
                check_struct(t)
                continue
            ce = self.crate_info[crate].get("enums", {}).get(t)
            if ce is not None and ce["ok"] is True:
                mine = self.enums.get(t)
                want = ", ".join(ce["variants"])
                if mine is None:
                    raise Untranslatable(
                        node,
                        f"`{t}` crosses this call — declare its twin in the app: "
                        f"@value class {t}(Enum) with members {want}",
                    )
                if set(mine) != set(ce["variants"]):
                    raise Untranslatable(
                        node,
                        f"enum {t} does not match the crate's — the crate declares ({want})",
                    )

    def _crate_call(self, func):
        # `from yokan import crates` -> crates.hexfmt.encode(...)
        if (
            isinstance(func, ast.Attribute)
            and isinstance(func.value, ast.Attribute)
            and isinstance(func.value.value, ast.Name)
            and self.crates_name is not None
            and func.value.value.id == self.crates_name
        ):
            return (func.value.attr, func.attr)
        return None

    def _stdlib_call(self, func):
        # `from yokan import fs` → fs.read_text(...)   (the documented
        # spelling); the attribute chain yokan.fs.read_text(...) works
        # too — it is the same object in Python, so refusing it would
        # invent a rule Python does not have.
        if (
            isinstance(func, ast.Attribute)
            and isinstance(func.value, ast.Name)
            and func.value.id in self.stdlib_mods
        ):
            return (self.stdlib_mods[func.value.id], func.attr)
        if (
            isinstance(func, ast.Attribute)
            and isinstance(func.value, ast.Attribute)
            and isinstance(func.value.value, ast.Name)
            and self.ui is not None
            and func.value.value.id == self.ui
            and func.value.attr in ("fs", "sqlite", "http", "math", "json", "time", "strings", "random", "notify")
        ):
            return (func.value.attr, func.attr)
        return None

    def _check_i64(self, value: int, node, neg: bool):
        lo, hi = (0, 2**63) if neg else (-(2**63), 2**63 - 1)
        if not lo <= value <= hi:
            raise Untranslatable(node, "the int literal exceeds the 64-bit range an int holds")

    def expr(self, node, ctx: str, param: str | None = None) -> str:
        """ctx: 'view' (fields as App.k) or 'store' (bare k)."""
        if isinstance(node, ast.Attribute) and node.attr in ("name", "value"):
            meta = self._enum_meta(node, ctx, param)
            if meta is not None:
                return meta
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            sm = self._str_method(node, ctx, param)
            if sm is not None:
                return sm
            if (
                node.func.attr == "join"
                and len(node.args) == 1
                and not node.keywords
                and self._num_ty(node.func.value, ctx, param) == "String"
            ):
                src = self._list_source(node.args[0], ctx, param)
                if src is None or not src[1].startswith("List<String"):
                    raise Untranslatable(node.args[0], "join() takes a list[str] — a state read, a field or a local holding one")
                self.uses_stdlib = True
                return f"Py.strJoin({self.expr(node.func.value, ctx, param)}, {src[0]})"
        if (
            isinstance(node, ast.Subscript)
            and self._num_ty(node.value, ctx, param) == "String"
        ):
            recv = self.expr(node.value, ctx, param)
            self.uses_stdlib = True
            if isinstance(node.slice, ast.Slice):
                if node.slice.step is not None:
                    raise Untranslatable(node.slice, "a slice step is not in the dialect yet — `s[a:b]` takes a start and a stop")
                lo = self.expr(node.slice.lower, ctx, param) if node.slice.lower else "0"
                hi = self.expr(node.slice.upper, ctx, param) if node.slice.upper else f"Py.strLen({recv})"
                return f"Py.strSlice({recv}, {lo}, {hi})"
            return f"Py.strIndex({recv}, {self.expr(node.slice, ctx, param)})"
        if isinstance(node, ast.Name) and self.row is not None and node.id == self.row[1]:
            # The row index: the repeater binds it beside the row, so
            # it reads like any other local in the row's own scope.
            return self.row[3]
        if isinstance(node, ast.Constant):
            if type(node.value) is bool:
                return "true" if node.value else "false"
            if type(node.value) is int:
                self._check_i64(node.value, node, False)
                return str(node.value)
            if type(node.value) is float:
                return repr(node.value)
            if type(node.value) is str:
                return f'"{esc(node.value)}"'
            raise Untranslatable(node, f"a literal of this kind ({node.value!r}) is not in the dialect — the literals are int, float, str, bool and None")
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "len"
            and len(node.args) == 1
        ):
            c = self._cell_read(node.args[0])
            if c is not None and (
                self.cells.get(c, "").startswith("List<")
                or self.cells.get(c, "").startswith("Map<")
            ):
                return f"{c}.length" if ctx == "store" else f"App.{c}.length"
            a0 = node.args[0]
            if (
                isinstance(a0, ast.Attribute)
                and isinstance(a0.value, ast.Name)
                and a0.value.id in self.stores
            ):
                fty = self.stores[a0.value.id]["field_tys"].get(a0.attr, "")
                if fty.startswith("List<") or fty.startswith("Map<"):
                    return f"{a0.value.id}.{a0.attr}.length"
            src0 = self._list_source(a0, ctx, param)
            if src0 is not None and (src0[1].startswith("List<") or src0[1].startswith("Map<")):
                return f"{src0[0]}.length"
            if self._num_ty(a0, ctx, param) == "String":
                self.uses_stdlib = True
                return f"Py.strLen({self.expr(a0, ctx, param)})"
            if isinstance(a0, (ast.List, ast.Tuple, ast.Set)):
                return str(len(a0.elts))
            if isinstance(a0, ast.Dict):
                return str(len(a0.keys))
            if isinstance(a0, ast.Constant) and type(a0.value) is str:
                return str(len(a0.value))
            raise Untranslatable(node, "len() takes a list or dict state read (`len(items())`) or a store field (`len(Cart.items)`) — `len(s)` of a str is not in the dialect yet")
        if (
            self.row is not None
            and isinstance(node, ast.Subscript)
            and isinstance(node.slice, ast.Name)
            and node.slice.id == self.row[1]
            and self._row_source_read(node.value)
        ):
            return self.row[2]
        if isinstance(node, ast.ListComp):
            return self._comprehension(node, ctx, param)
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in self.LIST_BUILTINS
            and node.func.id not in self.defs
        ):
            return self._list_builtin(node, ctx, param)
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in ("str", "int", "float", "bool", "round")
            and len(node.args) == 1
            and not node.keywords
            and node.func.id not in self.defs
        ):
            conv = self._conversion(node, ctx, param)
            if conv is not None:
                return conv
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and node.func.value.id in self.stores
        ):
            sname, meth = node.func.value.id, node.func.attr
            statics = self.stores[sname]["statics"]
            if meth in statics:
                sparams, _sr, _sl, _sb = self.helpers[statics[meth]]
                if node.keywords or len(node.args) != len(sparams):
                    raise Untranslatable(node, f"`{sname}.{meth}` takes {len(sparams)} positional argument(s)")
                sargs = ", ".join(self.expr(a, ctx, param) for a in node.args)
                return f"Helpers.{statics[meth]}({sargs})"
            sret = self.stores[sname]["ret_tys"].get(meth)
            if sret is not None:
                if ctx != "store":
                    raise Untranslatable(node, f"a view cannot call `{sname}.{meth}()` — building the screen only reads state, and a method may write to it; read a `@property`, or call the method in a handler and keep the result in a field")
                ordered = self._call_args(node, self.stores[sname]["params"].get(meth, []), f"{sname}.{meth}", self.stores[sname]["defaults"].get(meth))
                margs = ", ".join(self.expr(a, "store", param) for a in ordered)
                return f"{sname}.{self._smeth(sname, meth)}({margs})"
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and (self.model_instances.get(node.func.value.id)
                 or (self.typed_locals.get(node.func.value.id) if self.typed_locals.get(node.func.value.id) in self.models else None))
        ):
            base = node.func.value.id
            mname = self.model_instances.get(base) or self.typed_locals.get(base)
            mret = self.models[mname]["ret_tys"].get(node.func.attr)
            if mret is not None:
                if ctx != "store" or self.pre_lines is None:
                    raise Untranslatable(node, f"a view cannot call `{base}.{node.func.attr}()` — building the screen only reads state, and a method may write to it; call it in a handler and keep the result in a field")
                ordered = self._call_args(node, self.models[mname]["params"].get(node.func.attr, []), f"{base}.{node.func.attr}", self.models[mname]["defaults"].get(node.func.attr))
                oargs = ", ".join(self.expr(a, "store", param) for a in ordered)
                tmp = f"__o{len(self.handler_locals)}"
                self.handler_locals.add(tmp)
                self.pre_lines.append(f"var {tmp} = {self.expr(node.func.value, 'store', param)}")
                return f"{tmp}.{self._mmeth(mname, node.func.attr)}({oargs})"
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in self.models
        ):
            if ctx != "store":
                raise Untranslatable(node, "construct models in handlers — views read their fields")
            if not (node.args or node.keywords):
                return f"{node.func.id}()"
            # `Acc(v=3)` is the model's own constructor: build it with
            # its defaults, then write the fields that were given —
            # which is what Python's synthesized __init__ does.
            mname = node.func.id
            fields = [(f, t) for f, t, _ in self.models[mname]["fields"]]
            ordered = self._call_args(node, fields, mname)
            if self.pre_lines is None:
                raise Untranslatable(node, f"`{mname}(...)` with arguments is built in a handler statement — a view reads a model, it does not make one")
            tmp = f"__m{len(self.handler_locals)}"
            self.handler_locals.add(tmp)
            self.typed_locals[tmp] = mname
            self.pre_lines.append(f"var {tmp} = {mname}()")
            for (f, _t), a in zip(fields, ordered):
                self.pre_lines.append(f"{tmp}.{f} = {self.expr(a, 'store', param)}")
            return tmp
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            rt0 = self._num_ty(node.func.value, ctx, param)
            if rt0 is not None and rt0 in self.structs:
                entry = self.struct_methods.get(rt0, {}).get(node.func.attr)
                if entry is not None and not node.func.attr.startswith("__"):
                    if ctx == "view" and not self.inline_handler and not self.text_hole:
                        raise Untranslatable(node, "call value-class methods from handlers — views read fields")
                    recv = self.expr(node.func.value, ctx, param)
                    args = ", ".join(self.expr(a, ctx, param) for a in node.args)
                    return f"({recv}).{entry[0]}({args})"
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and node.func.attr == "get"
        ):
            recv = None
            d9 = self._cell_read(node.func.value)
            if d9 is not None and self._ty(d9).startswith("Map<"):
                recv = d9 if ctx == "store" else f"App.{d9}"
            elif (
                isinstance(node.func.value, ast.Attribute)
                and isinstance(node.func.value.value, ast.Name)
                and node.func.value.value.id in self.stores
                and self.stores[node.func.value.value.id]["field_tys"].get(node.func.value.attr, "").startswith("Map<")
            ):
                recv = f"{node.func.value.value.id}.{node.func.value.attr}"
            elif (
                isinstance(node.func.value, ast.Attribute)
                and isinstance(node.func.value.value, ast.Name)
                and node.func.value.value.id == "self"
                and self.model_scope is not None
                and self.model_scope[1].get(node.func.value.attr, "").startswith("Map<")
            ):
                recv = node.func.value.attr
            elif (
                isinstance(node.func.value, ast.Name)
                and self.typed_locals.get(node.func.value.id, "").startswith("Map<")
            ):
                recv = node.func.value.id
            if recv is not None:
                if len(node.args) != 2 or node.keywords:
                    raise Untranslatable(node, "a bare `d[k]` read raises KeyError in Python when the key is missing; read with `.get(key, default)`, which says what a missing key means")
                key = self._dict_key(node.args[0], ctx, param)
                dflt = self.expr(node.args[1], ctx, param)
                return f"{recv}.getOr({key}, {dflt})"
        if (
            isinstance(node, ast.Subscript)
            and (c9 := self._cell_read(node.value)) is not None
            and self._ty(c9).startswith("Map<")
        ):
            raise Untranslatable(
                node,
                "a bare `d[k]` read raises KeyError in Python when the key is missing; read with `.get(key, default)`, which says what a missing key means",
            )
        if (
            isinstance(node, ast.Subscript)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.handler_locals
            and ctx != "store"
        ):
            raise Untranslatable(node, "a handler local cannot be read in a view — hold it in a State or a store field")
        if isinstance(node, ast.Subscript):
            indexed = self._index_read(node, ctx, param)
            if indexed is not None:
                return indexed
        if isinstance(node, ast.JoinedStr):
            if ctx != "store":
                raise Untranslatable(node, 'an f-string in this position is not in the dialect — render it with `text(f"...")`')
            parts = []
            for part in node.values:
                if isinstance(part, ast.Constant) and type(part.value) is str:
                    parts.append(esc(part.value))
                elif isinstance(part, ast.FormattedValue):
                    if part.conversion != -1:
                        raise Untranslatable(part, "`!r` / `!s` conversions are not in the dialect — a hole renders `str()` already")
                    if part.format_spec is not None:
                        parts.append("#{" + self._format_call(part.value, self._spec_text(part), "store", param) + "}")
                    else:
                        parts.append(self._splice(self._hole(part.value, "store", param)))
                else:
                    raise Untranslatable(part, "this f-string part is not in the dialect — a hole holds a state read, a field, a parameter or `+ - *` arithmetic over them")
            return '"' + "".join(parts) + '"'
        if isinstance(node, ast.Subscript):
            return self._field(node, ctx)
        if (
            isinstance(node, ast.Name)
            and self.helper_params is not None
            and node.id in self.helper_params
        ):
            return node.id
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.view_bindings
            and self.view_bindings[node.value.id] in self.models
            and ctx == "view"
        ):
            mn = self.view_bindings[node.value.id]
            mfields = {f: t for f, t, _ in self.models[mn]["fields"]}
            if node.attr not in mfields:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {mn}")
            return f"{node.value.id}.{node.attr}"
        if isinstance(node, ast.Name) and node.id in self.view_bindings and ctx == "view":
            if self.text_hole and not self.in_wrapped_hole and self.view_bindings[node.id] in ("Float", "Bool"):
                raise Untranslatable(node, 'a float or bool local has no text in a hole yet — write it to a State first and render `f"{r()}"`, or pin decimals with `.Nf`')
            return node.id
        if isinstance(node, ast.Name) and node.id in self.dead_locals and node.id not in self.handler_locals:
            raise Untranslatable(node, self.dead_locals[node.id])
        if isinstance(node, ast.Name) and node.id in self.handler_locals:
            if ctx != "store":
                raise Untranslatable(node, "a handler local cannot be read in a view — hold it in a State or a store field")
            if self.text_hole and not self.in_wrapped_hole:
                ty = self.typed_locals.get(node.id)
                if ty in ("Int", "String"):
                    return node.id
                if ty in ("Float", "Bool"):
                    raise Untranslatable(node, 'a float or bool local has no text in a hole yet — write it to a State first and render `f"{r()}"`, or pin decimals with `.Nf`')
                raise Untranslatable(node, "a local of unknown type has no text in a hole — hold the value in a State or a typed parameter")
            return node.id
        cell = self._cell_read(node)
        if cell is not None:
            if self.comp_locals and cell in self.comp_locals:
                return cell  # component-local: bare in every position
            return cell if ctx == "store" else f"App.{cell}"
        if isinstance(node, ast.Name) and param is not None and node.id == param:
            if self.inline_text_param == param:
                return self.inline_implicit_name or "text"
            return "t" if ctx == "store" else node.id
        if (
            self.comp_params is not None
            and isinstance(node, ast.Name)
            and node.id in self.comp_params
        ):
            if ctx == "store":
                raise Untranslatable(node, f"a handler inside a component cannot read the component's parameter `{node.id}` yet — hold the value in a State or a store field")
            return node.id
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in self.union_of
        ):
            if ctx == "view" and not self.inline_handler:
                raise Untranslatable(node, "construct value classes in handlers — views read their fields")
            return self._variant_ctor(node, ctx, param)
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in self.structs
        ):
            if ctx == "view" and not self.inline_handler:
                raise Untranslatable(node, "construct value classes in handlers — views read their fields")
            return self._struct_ctor(node.func.id, node, ctx, param)
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and self.replace_name is not None
            and node.func.id == self.replace_name
        ):
            if ctx == "view" and not self.inline_handler:
                raise Untranslatable(node, "construct value classes in handlers — views read their fields")
            if len(node.args) != 1:
                raise Untranslatable(node, "replace(value, field=...) takes the value first, then keyword fields")
            src = node.args[0]
            sname = None
            c = self._cell_read(src)
            if c is not None and self._ty(c) in self.structs:
                sname, src_code = self._ty(c), (c if ctx == "store" else f"App.{c}")
            elif isinstance(src, ast.Name) and src.id in self.handler_locals:
                raise Untranslatable(src, "replace(...) on a local cannot see its value class yet — read the value from a State (`replace(sel(), x=1)`)")
            if sname is None:
                raise Untranslatable(src, "replace(...) takes a value read from a State (`replace(sel(), x=1)`) — a local or a field is not accepted here yet")
            by_kw = {}
            for kw in node.keywords:
                if kw.arg is None:
                    raise Untranslatable(node, "`**` unpacking is not in the dialect — pass fields by name")
                by_kw[kw.arg] = kw.value
            vals = []
            for f, _t, _d in self.structs[sname]:
                if f in by_kw:
                    vals.append(self.expr(by_kw.pop(f), ctx, param))
                else:
                    vals.append(f"{src_code}.{f}")
            if by_kw:
                raise Untranslatable(node, f"unknown field `{next(iter(by_kw))}`")
            return f"{sname}({', '.join(vals)})"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id == "self"
            and self.struct_self is not None
        ):
            sfields = {f for f, _, _ in self.structs[self.struct_self]}
            if node.attr not in sfields:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {self.struct_self}")
            return f"this.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id == "self"
            and self.model_scope is not None
        ):
            fld = node.attr
            if fld not in self.model_scope[1]:
                raise Untranslatable(node, f"`{fld}` is not a field of {self.model_scope[0]}")
            if self.text_hole and not self.in_wrapped_hole and self.model_scope[1][fld] in ("Float", "Bool"):
                raise Untranslatable(node, 'a float or bool local has no text in a hole yet — write it to a State first and render `f"{r()}"`, or pin decimals with `.Nf`')
            return fld
        if (
            isinstance(node, ast.Attribute)
            and (c8 := self._cell_read(node.value)) is not None
            and self._ty(c8) in self.structs
        ):
            fields = {f: t for f, t, _ in self.structs[self._ty(c8)]}
            if node.attr not in fields:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {self._ty(c8)}")
            if self.text_hole and not self.in_wrapped_hole and fields[node.attr] in ("Float", "Bool"):
                raise Untranslatable(node, 'a float or bool field has no text in this position — read it in an f-string hole (`f"{Cart.ratio}"`)')
            return f"{c8}.{node.attr}" if ctx == "store" else f"App.{c8}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and self.typed_locals.get(node.value.id) in self.structs
        ):
            sname9 = self.typed_locals[node.value.id]
            fields9 = {f: t for f, t, _ in self.structs[sname9]}
            if node.attr not in fields9:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {sname9}")
            return f"{node.value.id}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Attribute)
            and (s8 := self._struct_of(node.value)) is not None
        ):
            fields8 = {f: t for f, t, _ in self.structs[s8]}
            if node.attr not in fields8:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {s8}")
            if self.text_hole and not self.in_wrapped_hole and fields8[node.attr] in ("Float", "Bool"):
                raise Untranslatable(node, 'a float or bool field has no text in this position — read it in an f-string hole (`f"{Cart.ratio}"`)')
            return f"{self.expr(node.value, ctx, param)}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.stores
            and node.attr in self.stores[node.value.id]["props"]
        ):
            sname = node.value.id
            if (sname, node.attr) in self.prop_stack:
                raise Untranslatable(node, f"`{sname}.{node.attr}` is defined in terms of itself")
            self.prop_stack.append((sname, node.attr))
            try:
                return self.expr(self.stores[sname]["props"][node.attr], ctx, param)
            finally:
                self.prop_stack.pop()
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.stores
        ):
            sname = node.value.id
            ftys = self.stores[sname]["field_tys"]
            if node.attr not in ftys:
                raise Untranslatable(node, f"`{node.attr}` is not a field of store {sname}")
            if self.text_hole and not self.in_wrapped_hole and ftys[node.attr] in ("Float", "Bool"):
                raise Untranslatable(node, 'a float or bool field has no text in this position — read it in an f-string hole (`f"{Cart.ratio}"`)')
            return f"{sname}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.model_instances
        ):
            mname = self.model_instances[node.value.id]
            fields = {f: t for f, t, _ in self.models[mname]["fields"]}
            if node.attr not in fields:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {mname}")
            if self.text_hole and not self.in_wrapped_hole and fields[node.attr] in ("Float", "Bool"):
                raise Untranslatable(node, 'a float or bool field has no text in this position — read it in an f-string hole (`f"{Cart.ratio}"`)')
            return f"{node.value.id}.{node.attr}" if ctx == "store" else f"App.{node.value.id}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and self.typed_locals.get(node.value.id) in self.models
        ):
            mn = self.typed_locals[node.value.id]
            mfields = {f: t for f, t, _ in self.models[mn]["fields"]}
            if node.attr not in mfields:
                raise Untranslatable(node, f"`{node.attr}` is not a field of {mn}")
            return f"{node.value.id}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.handler_locals
            and self.model_scope is None
        ):
            return f"{node.value.id}.{node.attr}"
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.enums
        ):
            if node.attr not in self.enums[node.value.id]:
                raise Untranslatable(node, f"`{node.attr}` is not a member of {node.value.id}")
            return f"{node.value.id}.{node.attr}"
        if isinstance(node, ast.Name) and node.id in self.model_instances:
            if ctx != "store":
                raise Untranslatable(node, "pass models around in handlers — views read their fields")
            return node.id
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Attribute)
            and isinstance(node.func.value, ast.Name)
            and self.helper_params is not None
            and node.func.value.id in self.helper_params
            and self.helper_params[node.func.value.id] in self.protocols
        ):
            recv = node.func.value.id
            proto = self.helper_params[recv]
            sigs = {m: (ps, r) for m, ps, r in self.protocols[proto]}
            if node.func.attr not in sigs:
                raise Untranslatable(node, f"`{node.func.attr}` is not a method of {proto}")
            args = ", ".join(self.expr(a, ctx, param) for a in node.args)
            return f"{recv}.{node.func.attr}({args})"
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in self.defs
            and node.func.id not in self.escapes
            and node.func.id not in self.comp_defs
        ):
            name = node.func.id
            if name not in self.helpers:
                self._take_helper(self.defs[name])
            if ctx == "view" and not self.inline_handler and self.helpers[name][3]:
                raise Untranslatable(node, "a helper with a Protocol-typed parameter is called from handlers, not views")
            if name not in self.helpers:
                self._take_helper(self.defs[name])
            params, _ret, _lines, bounded = self.helpers[name]
            call_args = list(node.args)
            if node.keywords:
                raise Untranslatable(node, f"`{name}` takes positional arguments — keyword arguments to a helper are not in the dialect yet")
            defaults = self.defs[name].args.defaults
            if len(call_args) < len(params) and defaults:
                # Python evaluates a default once, at the def; the
                # dialect takes literal defaults only, so writing the
                # value at the call site is the same value.
                need = len(params) - len(call_args)
                if need > len(defaults):
                    raise Untranslatable(node, f"`{name}` takes at least {len(params) - len(defaults)} positional argument(s)")
                for d in defaults[len(defaults) - need:]:
                    if not self._is_const_expr(d):
                        raise Untranslatable(d, f"`{name}`'s default is a literal — a computed default is not in the dialect yet")
                    call_args.append(d)
            if len(call_args) != len(params):
                raise Untranslatable(node, f"`{name}` takes {len(params)} positional argument(s)")
            args = ", ".join(self.expr(a, ctx, param) for a in call_args)
            if bounded:
                return f"{name}({args})"
            return f"Helpers.{name}({args})"
        if isinstance(node, ast.Call) and self._stdlib_call(node.func) is not None:
            mod, fn = self._stdlib_call(node.func)
            if ctx == "view":
                raise Untranslatable(node, "standard-library calls run in handlers, not views (views stay pure) — compute in a handler and hold the result in a State")
            spec = self.STDLIB_CALLS.get((mod, fn))
            if spec is None:
                raise Untranslatable(
                    node, f"`{mod}.{fn}` is not in the standard library's {mod} — it has {', '.join(f for (m, f) in self.STDLIB_CALLS if m == mod)}"
                )
            cls, camel, arity = spec
            if len(node.args) != arity or node.keywords:
                raise Untranslatable(node, f"`{mod}.{fn}` takes {arity} argument(s)")
            self.uses_stdlib = True
            args = ", ".join(self.expr(a, ctx, param) for a in node.args)
            call = f"{cls}.{camel}({args})"
            if self.in_async and self.pre_lines is not None:
                # `await` is a whole right-hand side in pixie, and the
                # await is what puts the work on the background pool —
                # so the call is bound first and read after.
                tmp = f"__a{len(self.handler_locals)}"
                self.handler_locals.add(tmp)
                rt = self.STDLIB_RET.get((mod, fn))
                if rt:
                    self.typed_locals[tmp] = rt
                self.pre_lines.append(f"var {tmp} = await {call}")
                return tmp
            return call
        if isinstance(node, ast.Call) and self._crate_call(node.func) is not None:
            crate, fn = self._crate_call(node.func)
            if ctx == "view":
                raise Untranslatable(node, "crate calls run in handlers, not views (views stay pure) — compute in a handler and hold the result in a State")
            info = self.crate_info.get(crate)
            if info is None:
                raise Untranslatable(
                    node,
                    f"crate `{crate}` is not declared — run `yokan add <app> {crate} <version>` "
                    f"(or `--path <dir>`), or write it into `[tool.yokan.crates]` "
                    f"in the PEP 723 block or pyproject.toml",
                )
            entry = info["fns"].get(fn)
            if entry is None:
                have = ", ".join(sorted(k for k, v in info["fns"].items() if not isinstance(v, str)))
                raise Untranslatable(
                    node, f"`{crate}` has no bindable function `{fn}` (bindable here: {have or 'none'})"
                )
            if isinstance(entry, str):
                raise Untranslatable(node, f"`{crate}.{fn}` cannot cross yet — {entry}")
            params, _ret, rpi_name, _rp, fallible = entry
            if fallible:
                raise Untranslatable(
                    node,
                    f"{crate}.{fn} returns Result — call it inside try/except "
                    f"(`except Exception as e` binds the message)",
                )
            if len(node.args) != len(params) or node.keywords:
                raise Untranslatable(
                    node, f"{crate}.{fn} takes {len(params)} positional argument(s)"
                )
            self._check_crate_structs(node, crate, entry)
            self.uses_crates.add(crate)
            args = ", ".join(
                "nil" if isinstance(a, ast.Constant) and a.value is None
                else self.expr(a, ctx, param)
                for a in node.args
            )
            return f"{info['class']}.{rpi_name}({args})"
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in self.escapes
        ):
            if ctx == "view":
                raise Untranslatable(
                    node, "call @py functions from handlers and hold the result in a State — views stay pure"
                )
            args = ", ".join(self.expr(a, ctx, param) for a in node.args)
            return f"Escapes.{self._camel(node.func.id)}({args})"
        if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Div, ast.FloorDiv, ast.Mod, ast.Pow)):
            # Python-semantics arithmetic: the native side calls the
            # Py statics (exactly CPython's results, zero divisors
            # and overflow failing the statement the way the raised
            # exception does); tier A just runs the real operator.
            if ctx == "view" and not self.inline_handler:
                raise Untranslatable(
                    node,
                    "`/`, `//`, `%`, `**` can fail (zero, overflow) — compute in a handler and render the result",
                )
            lt = self._num_ty(node.left, ctx, param)
            rt = self._num_ty(node.right, ctx, param)
            if lt not in ("Int", "Float") or rt not in ("Int", "Float"):
                raise Untranslatable(
                    node, "the operand types here are not visible — hold the values in typed state or locals first"
                )
            self.uses_stdlib = True
            a = self.expr(node.left, ctx, param)
            b = self.expr(node.right, ctx, param)
            fl = "Float" in (lt, rt)
            wa = a if lt == "Float" else f"1.0 * ({a})"
            wb = b if rt == "Float" else f"1.0 * ({b})"
            if isinstance(node.op, ast.Div):
                return f"Py.truedivFloat({wa}, {wb})" if fl else f"Py.truedivInt({a}, {b})"
            if isinstance(node.op, ast.FloorDiv):
                return f"Py.floordivFloat({wa}, {wb})" if fl else f"Py.floordivInt({a}, {b})"
            if isinstance(node.op, ast.Mod):
                return f"Py.modFloat({wa}, {wb})" if fl else f"Py.modInt({a}, {b})"
            if fl:
                return f"Py.powFloat({wa}, {wb})"
            if isinstance(node.right, ast.Constant) and type(node.right.value) is int and node.right.value >= 0:
                return f"Py.powInt({a}, {b})"
            raise Untranslatable(
                node,
                "`int ** int` takes a non-negative literal exponent (a negative exponent would make the result a float at runtime) — write the exponent as a literal, or make one side a float",
            )
        if isinstance(node, ast.BinOp) and isinstance(node.op, (ast.Add, ast.Sub, ast.Mult)):
            lt0 = self._num_ty(node.left, ctx, param)
            if lt0 is not None and lt0 in self.structs:
                dunder = {ast.Add: "__add__", ast.Sub: "__sub__", ast.Mult: "__mul__"}[type(node.op)]
                entry = self.struct_methods.get(lt0, {}).get(dunder)
                if entry is None:
                    raise Untranslatable(
                        node, f"`{lt0}` does not define {dunder} — add it to the value class to use this operator"
                    )
                a = self.expr(node.left, ctx, param)
                b = self.expr(node.right, ctx, param)
                return f"({a}).{entry[0]}({b})"
            if ctx == "view" and not self.inline_handler and not self.text_hole and not self.in_prop:
                raise Untranslatable(
                    node,
                    "arithmetic outside an f-string hole is not in the dialect in views — compute in a handler and render the result",
                )
            if isinstance(node.op, ast.Add) and (
                self._list_of(node.left, ctx, param) is not None
                or self._list_of(node.right, ctx, param) is not None
            ):
                    left = self._list_of(node.left, ctx, param)
                    right = self._list_of(node.right, ctx, param)
                    if left is not None and right is not None and left[1] == right[1]:
                        self.uses_stdlib = True
                        return f"Py.listConcat{left[2]}({left[0]}, {right[0]})"
                    if left is not None and isinstance(node.right, ast.List):
                        self.uses_stdlib = True
                        lit = self._list_literal(node.right, left[1])
                        return f"Py.listConcat{left[2]}({left[0]}, {lit})"
                    raise Untranslatable(
                        node, "joining lists takes two lists of the same type — a state read, a field, a local, or a list literal"
                    )
            op = {ast.Add: "+", ast.Sub: "-", ast.Mult: "*"}[type(node.op)]
            return f"{self.expr(node.left, ctx, param)} {op} {self.expr(node.right, ctx, param)}"
        if isinstance(node, ast.BoolOp) and isinstance(node.op, (ast.And, ast.Or)):
            # As a VALUE, Python's and/or answer an OPERAND by
            # truthiness; over bools that operand IS the && / ||
            # result, so bools translate and anything else is named.
            if all(self._num_ty(v, ctx, param) == "Bool" for v in node.values):
                glue = " && " if isinstance(node.op, ast.And) else " || "
                return "(" + glue.join(self.expr(v, ctx, param) for v in node.values) + ")"
            raise Untranslatable(
                node,
                "`and` / `or` as a value works over bools — over other types Python answers one of the operands by truthiness; write a comparison or an `if`",
            )
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.Not):
            if self._num_ty(node.operand, ctx, param) == "Bool":
                return f"!({self.expr(node.operand, ctx, param)})"
            raise Untranslatable(node, "`not` as a value works over bools — write a comparison for other types")
        if isinstance(node, ast.IfExp):
            t = self._num_ty(node.body, ctx, param) or self._num_ty(node.orelse, ctx, param)
            if ctx != "store" or self.pre_lines is None or t not in self.ZERO_OF:
                raise Untranslatable(
                    node,
                    "a conditional expression (`a if c else b`) is written in a handler over "
                    "int, float, str or bool — in a view, branch the elements with `if` / `else`",
                )
            cond = self._cond(node.test, ctx, param)
            saved = self.pre_lines
            self.pre_lines = []
            a = self.expr(node.body, ctx, param)
            a_pre = self.pre_lines
            self.pre_lines = []
            b = self.expr(node.orelse, ctx, param)
            b_pre = self.pre_lines
            self.pre_lines = saved
            tmp = f"__t{len(self.handler_locals)}"
            self.handler_locals.add(tmp)
            self.typed_locals[tmp] = t
            self.pre_lines += [
                f"var {tmp} = {self.ZERO_OF[t]}",
                f"if {cond} {{",
                *[f"  {l}" for l in a_pre],
                f"  {tmp} = {a}",
                "} else {",
                *[f"  {l}" for l in b_pre],
                f"  {tmp} = {b}",
                "}",
            ]
            return tmp
        if isinstance(node, ast.NamedExpr) and isinstance(node.target, ast.Name):
            if ctx != "store" or self.pre_lines is None:
                raise Untranslatable(node, "`:=` binds a local in a handler — in a view, read the state directly")
            val = self.expr(node.value, ctx, param)
            name = node.target.id
            self.handler_locals.add(name)
            vt = self._num_ty(node.value, ctx, param)
            if vt is not None:
                self.typed_locals[name] = vt
            self.pre_lines.append(f"var {name} = {val}")
            return name
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
            if isinstance(node.operand, ast.Constant) and type(node.operand.value) is int:
                self._check_i64(node.operand.value, node, True)
                return f"-{node.operand.value}"
            if ctx == "view" and not self.inline_handler:
                raise Untranslatable(
                    node, "a unary minus in a view is not in the dialect yet — compute in a handler and render the result"
                )
            return f"-{self.expr(node.operand, ctx, param)}"
        raise Untranslatable(node, self._unknown_expr(node))

    def _reject_bool_text(self, node):
        for sub in ast.walk(node):
            if isinstance(sub, ast.Constant) and type(sub.value) is bool:
                raise Untranslatable(sub, 'a bool literal has no text here — render a bool from a State or field (`f"{flag()}"`)')
            c = self._cell_read(sub) if isinstance(sub, ast.Call) else None
            if c is not None and self._ty(c) == "Bool":
                raise Untranslatable(sub, 'a bool has no text in this position — render it in an f-string hole (`f"{flag()}"`)')

    def _reject_opt_enum_text(self, node):
        c = self._cell_read(node)
        if c is not None and self._ty(c).endswith("?"):
            raise Untranslatable(node, "an optional has no bare text — narrow it first (`if (v := x()) is not None:`)")
        if c is not None and self._ty(c) in self.enums:
            raise Untranslatable(node, 'an Enum has no text in this position — render it in an f-string hole (`f"{mood()}"`)')

    def _reject_map_text(self, node):
        # Top-node only: `len(d())` is fine, a bare `{d()}` hole is not.
        c = self._cell_read(node)
        if c is not None and self._ty(c).startswith("Map<"):
            raise Untranslatable(node, "a dict has no single text — render entries with `.get(key, default)`")

    def _reject_float_text(self, node):
        for sub in ast.walk(node):
            if isinstance(sub, ast.Constant) and type(sub.value) in (float, bool):
                raise Untranslatable(
                    sub, 'a float has no text in this position — render it in an f-string hole (`f"{ratio()}"`), or pin decimals with `.Nf`'
                )
            c = self._cell_read(sub) if isinstance(sub, ast.Call) else None
            if c is not None and self._ty(c) in ("Float", "Bool"):
                raise Untranslatable(sub, 'a float or bool has no text in this position — render it in an f-string hole (`f"{ratio()}"`)')
            if (
                self.row is not None
                and isinstance(sub, ast.Subscript)
                and self._row_source_read(sub.value)
                and self._row_elem_ty() == "List<Float>"
            ):
                raise Untranslatable(sub, 'a float has no text in this position — render it in an f-string hole (`f"{ratio()}"`)')

    CMP_OPS = {
        ast.Eq: "==", ast.NotEq: "!=", ast.Lt: "<",
        ast.LtE: "<=", ast.Gt: ">", ast.GtE: ">=",
    }

    def _is_plain_read(self, node) -> bool:
        """An expression that costs nothing and answers the same thing
        twice: a literal, a local, a field, a state read."""
        if isinstance(node, (ast.Constant, ast.Name)):
            return True
        if self._cell_read(node) is not None:
            return True
        if isinstance(node, ast.Attribute):
            return self._is_plain_read(node.value) or isinstance(node.value, ast.Name)
        return False

    def _cond(self, node, ctx: str = "view", param=None) -> str:
        """A condition (view `if` or handler `if`/`while`): a bool
        cell read, a comparison, bool `and`/`or`/`not` over those, or
        dict membership. (Truthiness of int/str needs an explicit
        compare — write `name() != \"\"`.)"""
        if isinstance(node, ast.BoolOp) and isinstance(node.op, (ast.And, ast.Or)):
            glue = " && " if isinstance(node.op, ast.And) else " || "
            # Python short-circuits and returns an OPERAND; over bools
            # (all a condition admits) that is exactly && / ||.
            return "(" + glue.join(self._cond(v, ctx, param) for v in node.values) + ")"
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.Not):
            return f"!({self._cond(node.operand, ctx, param)})"
        if (
            isinstance(node, ast.Compare)
            and len(node.ops) == 1
            and isinstance(node.ops[0], (ast.In, ast.NotIn))
        ):
            lst = self._list_of(node.comparators[0], ctx, param)
            if lst is not None:
                self.uses_stdlib = True
                code, _el, suf = lst
                bare = f"Py.listContains{suf}({code}, {self.expr(node.left, ctx, param)})"
                return f"!{bare}" if isinstance(node.ops[0], ast.NotIn) else bare
            if self._num_ty(node.comparators[0], ctx, param) == "String":
                self.uses_stdlib = True
                hay = self.expr(node.comparators[0], ctx, param)
                needle = self.expr(node.left, ctx, param)
                bare = f"Py.strContains({hay}, {needle})"
                return f"!{bare}" if isinstance(node.ops[0], ast.NotIn) else bare
            d = self._cell_read(node.comparators[0])
            if d is None or not self._ty(d).startswith("Map<"):
                raise Untranslatable(node, '`in` tests dict membership (`"k" in prices()`) — `in` over a list is not in the dialect yet')
            recv = d if ctx == "store" else f"App.{d}"
            bare = f"{recv}.contains({self._dict_key(node.left, ctx, param)})"
            return f"!{bare}" if isinstance(node.ops[0], ast.NotIn) else bare
        c = self._cell_read(node)
        if c is not None:
            if self._ty(c) != "Bool":
                raise Untranslatable(node, self._unknown_cond(node))
            return c if ctx == "store" else f"App.{c}"
        if (
            isinstance(node, ast.Attribute)
            and self._num_ty(node, "store" if ctx == "store" else "view", param) == "Bool"
        ):
            return self.expr(node, ctx, param)
        if not isinstance(node, (ast.Compare, ast.BoolOp, ast.UnaryOp, ast.NamedExpr, ast.IfExp)) and \
                self._num_ty(node, "store" if ctx == "store" else "view", param) == "Bool":
            # A bool IS the condition — `if on:` over a bool asks no
            # question about truthiness, so it needs no comparison.
            # (`while True:` arrives here too.)
            return self.expr(node, ctx, param)
        if isinstance(node, ast.Name) and node.id in self.handler_locals and ctx == "store":
            raise Untranslatable(node, "a bare local is not a condition yet — compare it explicitly")
        if isinstance(node, ast.Compare) and len(node.ops) > 1:
            # `0 < x < 10` is `0 < x and x < 10` with x evaluated once:
            # a plain read can simply be written twice, anything else
            # is bound to a local first.
            operands = [node.left, *node.comparators]
            reads = []
            for i, o in enumerate(operands):
                code = self.expr(o, ctx, param)
                repeated = 0 < i < len(operands) - 1
                if repeated and not self._is_plain_read(o):
                    if ctx != "store" or self.pre_lines is None:
                        raise Untranslatable(o, "the middle of a chained comparison is read twice, so it is a state read, a field or a local here")
                    tmp = f"__c{len(self.handler_locals)}"
                    self.handler_locals.add(tmp)
                    self.pre_lines.append(f"var {tmp} = {code}")
                    code = tmp
                reads.append(code)
            parts = []
            for i, o in enumerate(node.ops):
                sym = self.CMP_OPS.get(type(o))
                if sym is None:
                    raise Untranslatable(node, f"`{self._src(node)}` is not a comparison the dialect knows — the comparisons are ==, !=, <, <=, >, >=")
                parts.append(f"{reads[i]} {sym} {reads[i + 1]}")
            return "(" + " && ".join(parts) + ")"
        if isinstance(node, ast.Compare) and len(node.ops) == 1:
            op = {
                ast.Eq: "==",
                ast.NotEq: "!=",
                ast.Lt: "<",
                ast.LtE: "<=",
                ast.Gt: ">",
                ast.GtE: ">=",
            }.get(type(node.ops[0]))
            if op is None:
                if isinstance(node.ops[0], (ast.Is, ast.IsNot)):
                    raise Untranslatable(
                        node,
                        "`is None` / `is not None` narrows an Optional in a walrus "
                        "(`if (v := x()) is not None:`) — for a value that is not "
                        "Optional, compare with `==`",
                    )
                raise Untranslatable(
                    node,
                    f"`{self._src(node)}` is not a comparison the dialect knows — the "
                    "comparisons are ==, !=, <, <=, >, >=",
                )
            left = self.expr(node.left, ctx, param)
            right = self.expr(node.comparators[0], ctx, param)
            return f"{left} {op} {right}"
        raise Untranslatable(node, self._unknown_cond(node))

    def _enum_value_ty(self, ename: str):
        """`Int` or `String` if an enum's values all share a type."""
        vals = list(self.enum_values.get(ename, {}).values())
        if vals and all(type(v) is int for v in vals):
            return "Int"
        if vals and all(type(v) is str for v in vals):
            return "String"
        return None

    def _enum_meta(self, node: ast.Attribute, ctx, param):
        """`Mood.HAPPY.name` / `.value`, and the same on an enum-typed
        read. A member written out is answered here (its name and its
        value are known); a value that arrives at runtime is answered
        by a static synthesized for that enum, so both runs read the
        same table."""
        base = node.value
        if (
            isinstance(base, ast.Attribute)
            and isinstance(base.value, ast.Name)
            and base.value.id in self.enums
            and base.attr in self.enums[base.value.id]
        ):
            if node.attr == "name":
                return f'"{esc(base.attr)}"'
            v = self.enum_values.get(base.value.id, {}).get(base.attr)
            if isinstance(v, str):
                return f'"{esc(v)}"'
            return str(v)
        ename = self._num_ty(base, ctx, param)
        if ename not in self.enums:
            return None
        if node.attr == "value" and self._enum_value_ty(ename) is None:
            raise Untranslatable(node, f"`{ename}`'s members carry values of different types, so `.value` has no single type here")
        self.enum_meta_used.add(ename)
        fn = "enumName" if node.attr == "name" else "enumValue"
        # The static IS the text; the value inside it is an ordinary
        # read, so the hole's guards stand down for it.
        prev, self.in_wrapped_hole = self.in_wrapped_hole, True
        try:
            recv = self.expr(base, ctx, param)
        finally:
            self.in_wrapped_hole = prev
        return f"PyText.{fn}{ename}({recv})"

    def _union_match(self, stmt: ast.Match, subj: str, uname: str, param):
        """`match` over a sum type. pixie's `case` has one arm per
        variant and no guards, so the arms are grouped by variant and
        their guards become an if/else chain inside that arm — a
        failing guard falling through to the wildcard body, which is
        what Python does with the arms below it."""
        groups: dict[str, list] = {}
        order: list[str] = []
        default = None
        for case in stmt.cases:
            pats = case.pattern.patterns if isinstance(case.pattern, ast.MatchOr) else [case.pattern]
            if len(pats) > 1 and case.guard is not None:
                raise Untranslatable(case.pattern, "a `|` arm with a guard is not in the dialect — write the variants as separate arms")
            for pat in pats:
                arm, binds = self._union_arm(pat, uname)
                if arm == "when _":
                    if len(pats) > 1 or case.guard is not None:
                        raise Untranslatable(pat, "`_` is the arm that catches what is left, so it takes no guard and no alternatives")
                    default = case.body
                    continue
                if len(pats) > 1 and binds:
                    raise Untranslatable(pat, "a `|` arm binds nothing — the alternatives would have to bind the same names")
                if arm not in groups:
                    groups[arm] = []
                    order.append(arm)
                groups[arm].append((binds, case.guard, case.body))

        def body_lines(binds, body, indent: str) -> list[str]:
            for b, t in binds:
                self.handler_locals.add(b)
                self.typed_locals[b] = t
            try:
                out = [indent + l for l in self._block(body, param)]
            finally:
                for b, _t in binds:
                    self.handler_locals.discard(b)
                    self.typed_locals.pop(b, None)
                    self.dead_locals[b] = f"`{b}` is bound inside its match arm only"
            return out

        lines = [f"case {subj} {{"]
        for arm in order:
            lines.append(f"  {arm} {{")
            entries = groups[arm]
            depth = 0
            for binds, guard, body in entries:
                if guard is None:
                    lines += body_lines(binds, body, "    " + "  " * depth)
                    break
                for b, t in binds:
                    self.handler_locals.add(b)
                    self.typed_locals[b] = t
                try:
                    cond = self._cond(guard, "store", param)
                finally:
                    for b, _t in binds:
                        self.handler_locals.discard(b)
                        self.typed_locals.pop(b, None)
                pad = "    " + "  " * depth
                lines.append(f"{pad}if {cond} {{")
                lines += body_lines(binds, body, pad + "  ")
                lines.append(f"{pad}}} else {{")
                depth += 1
            else:
                # every arm for this variant was guarded: what is left
                # is the wildcard body, exactly as Python reads it.
                if default is not None:
                    lines += body_lines([], default, "    " + "  " * depth)
            for d in range(depth - 1, -1, -1):
                lines.append("    " + "  " * d + "}")
            lines.append("  }")
        if default is not None:
            covered = set(order)
            rest = [v for v in self.unions[uname] if f"when {v}" not in {a.split("(")[0] for a in covered}]
            if rest:
                lines.append("  when _ {")
                lines += body_lines([], default, "    ")
                lines.append("  }")
        lines.append("}")
        return lines

    def _match_literals(self, stmt: ast.Match, subj: str, param):
        """`match n:` over int, float, str or bool literals. pixie's
        `case` matches enum variants and sum-type constructors, so a
        literal match lowers to the if/elif chain it is: the subject is
        read once into a local, and a failing guard falls through to
        the next arm exactly as Python's does."""
        ty = self._num_ty(stmt.subject, "store", param)
        if ty not in ("Int", "Float", "String", "Bool"):
            return None
        arms = []
        for case in stmt.cases:
            pats = case.pattern.patterns if isinstance(case.pattern, ast.MatchOr) else [case.pattern]
            tests, wildcard = [], False
            for pat in pats:
                if isinstance(pat, ast.MatchAs) and pat.pattern is None and pat.name is None:
                    wildcard = True
                elif isinstance(pat, ast.MatchValue) and isinstance(pat.value, ast.Constant):
                    tests.append(pat.value)
                else:
                    return None
            if wildcard and tests:
                return None
            arms.append((tests, case.guard, case.body))
        tmp = f"__s{len(self.handler_locals)}"
        self.handler_locals.add(tmp)
        self.typed_locals[tmp] = ty

        def build(i):
            if i >= len(arms):
                return []
            tests, guard, body = arms[i]
            cond = " || ".join(f"{tmp} == {self.expr(t, 'store', param)}" for t in tests)
            if guard is not None:
                g = self._cond(guard, "store", param)
                cond = f"({cond}) && ({g})" if cond else g
            if not cond:
                return self._block(body, param)
            out = [f"if {cond} {{"]
            out += self._block(body, param)
            rest = build(i + 1)
            if rest:
                out.append("} else {")
                out += ["  " + l for l in rest]
            out.append("}")
            return out

        return [f"var {tmp} = {subj}"] + build(0)

    def _write_index(self, target: ast.Subscript, param, code: str) -> str:

        """The index of an `xs[i] = v`, lowered the way a read is: a
        literal or a `range()` variable goes in bare, anything else is
        bound first so a negative index counts from the back."""
        ix = target.slice
        if isinstance(ix, ast.Constant) and type(ix.value) is int and ix.value >= 0:
            return str(ix.value)
        if (
            isinstance(ix, ast.UnaryOp)
            and isinstance(ix.op, ast.USub)
            and isinstance(ix.operand, ast.Constant)
            and type(ix.operand.value) is int
        ):
            return f"{code}.length - {ix.operand.value}"
        if isinstance(ix, ast.Name) and ix.id in self.nonneg_loop_vars:
            return ix.id
        if self.pre_lines is None:
            raise Untranslatable(ix, "`xs[i] = v` indexes with a literal or a `range()` loop variable here")
        i = self.expr(ix, "store", param)
        tmp = f"__ix{len(self.handler_locals)}"
        self.handler_locals.add(tmp)
        self.pre_lines += [f"var {tmp} = {i}", f"if {tmp} < 0 {{", f"  {tmp} = {tmp} + {code}.length", "}"]
        return tmp

    @staticmethod
    def _defaults_of(m) -> dict:
        """{parameter: default node} for a method — the defaults sit at
        the end of the parameter list, as Python puts them."""
        names = [a.arg for a in m.args.args]
        start = len(names) - len(m.args.defaults)
        return {names[start + i]: d for i, d in enumerate(m.args.defaults)}

    def _call_args(self, call, params, what, defaults=None):
        """A call's arguments in the signature's order. Python binds by
        name, the compiled call binds by position, so a keyword
        argument is put back where the signature has it."""
        names = [p for p, _ in params]
        defaults = defaults or {}
        if not call.keywords and len(call.args) == len(names):
            return list(call.args)
        slot = {n: None for n in names}
        for i, a in enumerate(call.args):
            if i >= len(names):
                raise Untranslatable(a, f"`{what}` takes {len(names)} argument(s)")
            slot[names[i]] = a
        for kw in call.keywords:
            if kw.arg is None:
                raise Untranslatable(kw.value, "`**` unpacking is not in the dialect — pass the arguments by name")
            if kw.arg not in slot:
                raise Untranslatable(kw.value, f"`{what}` has no parameter `{kw.arg}` — its parameters are {', '.join(names)}")
            if slot[kw.arg] is not None:
                raise Untranslatable(kw.value, f"`{kw.arg}` is given twice")
            slot[kw.arg] = kw.value
        for n in names:
            if slot[n] is None and n in defaults:
                d = defaults[n]
                if not self._is_const_expr(d):
                    raise Untranslatable(d, f"`{what}`'s default for `{n}` is a literal — a computed default is not in the dialect yet")
                slot[n] = d
        missing = [n for n in names if slot[n] is None]
        if missing:
            raise Untranslatable(call, f"`{what}` is missing {', '.join(missing)}")
        return [slot[n] for n in names]

    # (method, argument count) -> (static, pix return type). Each one
    # is a Py static: written once in Rust, with CPython's meaning.
    STR_METHOD_STATICS = {
        ("upper", 0): ("strUpper", "String"),
        ("lower", 0): ("strLower", "String"),
        ("strip", 0): ("strStrip", "String"),
        ("lstrip", 0): ("strLstrip", "String"),
        ("rstrip", 0): ("strRstrip", "String"),
        ("split", 1): ("strSplit", "List<String>"),
        ("split", 0): ("strSplitWs", "List<String>"),
        ("startswith", 1): ("strStartswith", "Bool"),
        ("endswith", 1): ("strEndswith", "Bool"),
        ("replace", 2): ("strReplace", "String"),
        ("find", 1): ("strFind", "Int"),
        ("count", 1): ("strCount", "Int"),
    }

    @staticmethod
    def _spec_text(part: ast.FormattedValue) -> str:
        """The literal text of a hole's format spec."""
        spec = part.format_spec
        if not (
            isinstance(spec, ast.JoinedStr)
            and len(spec.values) == 1
            and isinstance(spec.values[0], ast.Constant)
            and isinstance(spec.values[0].value, str)
        ):
            raise Untranslatable(part, "a format spec is written out (`:.2f`, `:>8`, `:,`) — a computed spec is not in the dialect")
        return spec.values[0].value

    def _format_call(self, node, spec: str, ctx, param) -> str:
        """`f"{x:>8,.2f}"` — CPython's format mini-language, answered by
        the same implementation in both runs."""
        ty = self._num_ty(node, ctx, param)
        fn = {"Int": "formatInt", "Float": "formatFloat", "String": "formatStr"}.get(ty)
        if fn is None:
            raise Untranslatable(
                node,
                f"a format spec applies to an int, a float or a str — `{self._src(node)}` is "
                + (f"{py_ty(ty)} here" if ty else "of no type the dialect can format here"),
            )
        self.uses_stdlib = True
        prev, self.in_wrapped_hole = self.in_wrapped_hole, True
        try:
            v = self.expr(node, ctx, param)
        finally:
            self.in_wrapped_hole = prev
        return f'Py.{fn}({v}, "{esc(spec)}")'

    def _conversion(self, node: ast.Call, ctx, param):
        """`str(x)`, `int(x)`, `float(x)`, `bool(x)`, `round(x)` — the
        five conversions, each with Python's own answer: `str` is the
        text an f-string renders, `int` truncates a float and parses a
        str (failing the way Python raises), `bool` is the comparison
        the value stands for, and `round` rounds half to even."""
        fn = node.func.id
        a = node.args[0]
        ty = self._num_ty(a, ctx, param)
        if fn == "str":
            return '"' + self._str_parts(a, ctx, param) + '"'
        if fn == "bool":
            if ty == "Bool":
                return self.expr(a, ctx, param)
            zero = {"Int": "0", "Float": "0.0", "String": '""'}.get(ty)
            if zero is None:
                return None
            return f"({self.expr(a, ctx, param)} != {zero})"
        self.uses_stdlib = True
        inner = self.expr(a, ctx, param)
        if fn == "round":
            if ty == "Int":
                return inner
            return f"Py.round({inner})" if ty == "Float" else None
        if fn == "int":
            if ty == "Int":
                return inner
            if ty == "Float":
                return f"Py.intOfFloat({inner})"
            if ty == "String":
                return f"Py.intOfStr({inner})"
            return None
        if fn == "float":
            if ty == "Float":
                return inner
            if ty == "Int":
                return f"Py.floatOfInt({inner})"
            if ty == "String":
                return f"Py.floatOfStr({inner})"
        return None

    def _str_method(self, node: ast.Call, ctx, param):
        """`name().upper()` and the rest: a Py static over the same
        receiver, so the compiled run answers what CPython answered
        during development."""
        f = node.func
        if not isinstance(f, ast.Attribute) or node.keywords:
            return None
        entry = self.STR_METHOD_STATICS.get((f.attr, len(node.args)))
        if entry is None:
            return None
        if f.attr == "join":
            return None
        if self._num_ty(f.value, ctx, param) != "String":
            return None
        fn, _ret = entry
        self.uses_stdlib = True
        args = [self.expr(f.value, ctx, param)] + [self.expr(a, ctx, param) for a in node.args]
        return f"Py.{fn}({', '.join(args)})"

    def _str_method_ty(self, node: ast.Call, ctx, param):
        f = node.func
        if not isinstance(f, ast.Attribute) or node.keywords:
            return None
        entry = self.STR_METHOD_STATICS.get((f.attr, len(node.args)))
        if entry is None or self._num_ty(f.value, ctx, param) != "String":
            return None
        return entry[1]

    LIST_SUFFIX = {"String": "Str", "Int": "Int", "Float": "Float", "Bool": "Bool"}

    def _list_of(self, node, ctx, param):
        """(code, element type, static suffix) for a list the dialect
        can hand to a Py static: a state read, a field, or a local."""
        src = self._list_source(node, ctx, param)
        if src is None or not src[1].startswith("List<"):
            return None
        el = src[1][5:-1]
        suf = self.LIST_SUFFIX.get(el)
        return None if suf is None else (src[0], el, suf)

    LIST_BUILTINS = ("min", "max", "sum", "abs", "sorted", "reversed")

    def _comprehension(self, node: ast.ListComp, ctx, param):
        """`[f(x) for x in xs if cond]` — the loop it stands for, built
        into a local the expression then reads."""
        if ctx != "store" or self.pre_lines is None:
            raise Untranslatable(node, "a comprehension builds a list in a handler — a view reads one that is already built")
        if len(node.generators) != 1:
            raise Untranslatable(node, "a comprehension takes one `for` — nested comprehensions are not in the dialect yet")
        gen = node.generators[0]
        if gen.is_async or len(gen.ifs) > 1:
            raise Untranslatable(node, "a comprehension takes at most one `if`")
        if not isinstance(gen.target, ast.Name):
            raise Untranslatable(gen.target, "a comprehension binds one plain name")
        src = self._list_of(gen.iter, ctx, param)
        if src is None:
            raise Untranslatable(gen.iter, "a comprehension walks a list the dialect can name — a state read, a field or a local")
        code, el, _suf = src
        var = gen.target.id
        prev_ty = self.typed_locals.get(var)
        self.handler_locals.add(var)
        self.typed_locals[var] = el
        try:
            item_ty = self._num_ty(node.elt, ctx, param) or el
            suffix = self.LIST_SUFFIX.get(item_ty)
            if suffix is None:
                raise Untranslatable(node.elt, f"a comprehension builds a list of int, float, str or bool — this one builds {py_ty(item_ty)}")
            item = self.expr(node.elt, ctx, param)
            cond = self._cond(gen.ifs[0], ctx, param) if gen.ifs else None
        finally:
            self.handler_locals.discard(var)
            if prev_ty is None:
                self.typed_locals.pop(var, None)
            else:
                self.typed_locals[var] = prev_ty
        out = f"__lc{len(self.handler_locals)}"
        self.handler_locals.add(out)
        self.typed_locals[out] = f"List<{item_ty}>"
        body = [f"  {out}.push({item})"]
        if cond is not None:
            body = [f"  if {cond} {{", f"    {out}.push({item})", "  }"]
        self.pre_lines += [
            f"var {out} : List<{item_ty}> = []",
            f"for {var} in {code} {{",
            *body,
            "}",
        ]
        return out

    def _list_builtin(self, node: ast.Call, ctx, param):
        """`sum(xs)`, `min(a, b)`, `sorted(xs)`, `abs(n)` — Python's
        answers, from the same statics both runs would agree on."""
        fn = node.func.id
        if node.keywords:
            raise Untranslatable(node, f"`{fn}(...)` takes positional arguments")
        if fn in ("min", "max") and len(node.args) == 2:
            t = self._num_ty(node.args[0], ctx, param) or self._num_ty(node.args[1], ctx, param)
            if t not in ("Int", "Float"):
                raise Untranslatable(node, f"`{fn}(a, b)` compares two ints or two floats")
            self.uses_stdlib = True
            a = self.expr(node.args[0], ctx, param)
            b = self.expr(node.args[1], ctx, param)
            return f"Py.{fn}{t}({a}, {b})"
        if fn == "abs" and len(node.args) == 1:
            t = self._num_ty(node.args[0], ctx, param)
            if t not in ("Int", "Float"):
                raise Untranslatable(node, "`abs(x)` takes an int or a float")
            self.uses_stdlib = True
            return f"Py.abs{t}({self.expr(node.args[0], ctx, param)})"
        if len(node.args) != 1:
            raise Untranslatable(node, f"`{fn}(...)` takes one list here")
        lst = self._list_of(node.args[0], ctx, param)
        if lst is None:
            raise Untranslatable(
                node.args[0],
                f"`{fn}(...)` takes a list the dialect can name — a state read, a field or a local",
            )
        code, el, suf = lst
        if fn in ("min", "max", "sum") and el not in ("Int", "Float"):
            raise Untranslatable(node, f"`{fn}(xs)` takes a list of int or float — this one holds {py_ty(el)}")
        if fn == "sorted" and el == "Bool":
            raise Untranslatable(node, "`sorted(xs)` takes a list of int, float or str")
        self.uses_stdlib = True
        name = {"min": "listMin", "max": "listMax", "sum": "listSum",
                "sorted": "listSorted", "reversed": "listReversed"}[fn]
        return f"Py.{name}{suf}({code})"

    def _dict_key(self, node, ctx, param) -> str:
        """A dict key: any str the app can name. A literal is written
        as itself (`"two words"` included), and a str-typed expression
        is written as the expression, which is what the interpreted
        run reads too."""
        if isinstance(node, ast.Constant) and type(node.value) is str:
            return f'"{esc(node.value)}"'
        if self._num_ty(node, ctx, param) == "String":
            return self.expr(node, ctx, param)
        raise Untranslatable(
            node,
            f"a dict key is a str — `{self._src(node)}` is not a str here",
        )

    def _list_source(self, node, ctx, param):
        """(pixie expression, pix type) for a list the dialect can
        index: a state read, a store or model field written `Store.xs`
        or `self.xs`, or a handler local holding one. The two spellings
        of a field are the same field, so they index the same way."""
        c = self._cell_read(node)
        if c is not None:
            ty = self._ty(c)
            if self.comp_locals and c in self.comp_locals:
                return c, ty
            return (c if ctx == "store" else f"App.{c}"), ty
        if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
            base = node.value.id
            if base in self.stores and node.attr in self.stores[base]["field_tys"]:
                return f"{base}.{node.attr}", self.stores[base]["field_tys"][node.attr]
            if base == "self" and self.model_scope is not None and node.attr in self.model_scope[1]:
                return node.attr, self.model_scope[1][node.attr]
        if isinstance(node, ast.Attribute):
            # A list held by a value class, read through it.
            sname = self._struct_of(node.value)
            if sname is not None:
                for f, t, _d in self.structs[sname]:
                    if f == node.attr and t.startswith(("List<", "Map<")):
                        return f"({self.expr(node.value, ctx, param)}).{f}", t
            # ...or held by a model, read through the instance.
            if isinstance(node.value, ast.Name):
                mn = self.model_instances.get(node.value.id) or (
                    self.typed_locals.get(node.value.id)
                    if self.typed_locals.get(node.value.id) in self.models
                    else None
                )
                if mn is not None:
                    for f, t, _d in self.models[mn]["fields"]:
                        if f == node.attr and t.startswith(("List<", "Map<")):
                            return f"{node.value.id}.{f}", t
        if isinstance(node, ast.Name) and self.comp_params and node.id in self.comp_params:
            return node.id, self.comp_params[node.id]
        if isinstance(node, ast.Name) and self.comp_locals and node.id in self.comp_locals:
            return node.id, self.comp_locals[node.id]
        if isinstance(node, ast.Name) and node.id in self.handler_locals:
            t = self.typed_locals.get(node.id)
            if t in ("Int", "Float", "Bool", "String"):
                return None      # a scalar local; `s[0]` is a str index
            # A local copied out of a list state carries no type of its
            # own; indexing is what it is for, so it counts as a list.
            return node.id, (t if t and t.startswith(("List<", "Map<")) else "List<?>")
        return None

    @staticmethod
    def _int_index(ix):
        """The literal int an index node names, or None."""
        if isinstance(ix, ast.Constant) and type(ix.value) is int:
            return ix.value
        if (
            isinstance(ix, ast.UnaryOp)
            and isinstance(ix.op, ast.USub)
            and isinstance(ix.operand, ast.Constant)
            and type(ix.operand.value) is int
        ):
            return -ix.operand.value
        return None

    def _literal_element(self, node: ast.Subscript):
        """`["a", "b"][0]` — a literal list indexed by a literal. A
        module constant reads this way once its value is in place, and
        the element is known here, so it is what gets written."""
        if not isinstance(node.value, (ast.List, ast.Tuple)):
            return None
        k = self._int_index(node.slice)
        if k is None:
            return None
        elts = node.value.elts
        if not -len(elts) <= k < len(elts):
            raise Untranslatable(node, f"index {k} is past the end of a {len(elts)}-element list")
        return elts[k]

    def _index_read(self, node: ast.Subscript, ctx, param):
        """`xs[i]` over a list, with Python's meaning: a negative index
        counts from the back, and an index past the end stops the
        statement in both runs (the list read traps). A `range()` loop
        variable and a literal need no wrap, so the common shapes emit
        the bare index."""
        lit = self._literal_element(node)
        if lit is not None:
            return self.expr(lit, ctx, param)
        if isinstance(node.slice, ast.Slice):
            lst = self._list_of(node.value, ctx, param)
            if lst is None:
                return None
            if node.slice.step is not None:
                raise Untranslatable(node.slice, "a slice step is not in the dialect yet — `xs[a:b]` takes a start and a stop")
            code, _el, suf = lst
            self.uses_stdlib = True
            lo = self.expr(node.slice.lower, ctx, param) if node.slice.lower else "0"
            hi = self.expr(node.slice.upper, ctx, param) if node.slice.upper else f"{code}.length"
            return f"Py.listSlice{suf}({code}, {lo}, {hi})"
        src = self._list_source(node.value, ctx, param)
        if src is None:
            return None
        code, ty = src
        if not ty.startswith("List<"):
            return None
        ix = node.slice
        if isinstance(ix, ast.Constant) and type(ix.value) is int and ix.value >= 0:
            return f"{code}[{ix.value}]"
        if (
            isinstance(ix, ast.UnaryOp)
            and isinstance(ix.op, ast.USub)
            and isinstance(ix.operand, ast.Constant)
            and type(ix.operand.value) is int
        ):
            return f"{code}[{code}.length - {ix.operand.value}]"
        if isinstance(ix, ast.Name) and ix.id in self.nonneg_loop_vars:
            return f"{code}[{ix.id}]"
        if ctx != "store" or self.pre_lines is None:
            raise Untranslatable(
                node,
                f"indexing with a variable happens in a handler — a view reads the element "
                f"through a `list_view` row builder, or through a State",
            )
        i = self.expr(ix, "store", param)
        tmp = f"__ix{len(self.handler_locals)}"
        self.handler_locals.add(tmp)
        self.pre_lines += [f"var {tmp} = {i}", f"if {tmp} < 0 {{", f"  {tmp} = {tmp} + {code}.length", "}"]
        return f"{code}[{tmp}]"

    def _cell_read(self, node):
        """`count()` or `count.value()` for a State cell -> key."""
        if not (isinstance(node, ast.Call) and not node.args and not node.keywords):
            return None
        f = node.func
        if isinstance(f, ast.Name) and f.id in self.cells:
            return f.id
        if isinstance(f, ast.Name) and self.comp_locals and f.id in self.comp_locals:
            return f.id
        if (
            isinstance(f, ast.Attribute)
            and f.attr == "value"
            and isinstance(f.value, ast.Name)
            and f.value.id in self.cells
        ):
            return f.value.id
        return None

    def _ty(self, name: str):
        if self.comp_locals and name in self.comp_locals:
            return self.comp_locals[name]
        return self.cells.get(name)

    def _field(self, node: ast.Subscript, ctx: str) -> str:
        ok = (
            isinstance(node.value, ast.Name)
            and isinstance(node.slice, ast.Constant)
            and isinstance(node.slice.value, str)
        )
        if not ok:
            src = self._src(node)
            if isinstance(node.slice, ast.Slice):
                raise Untranslatable(node, f"a slice (`{src}`) is not in the dialect yet")
            c = self._cell_read(node.value)
            if c is not None and self._ty(c) == "String":
                raise Untranslatable(node, f"indexing a str (`{src}`) is not in the dialect yet")
            if ctx == "view":
                raise Untranslatable(
                    node,
                    f"indexing a list in a view (`{src}`) is not in the dialect yet — a "
                    "row builder indexes its list (`list_view`), or hold the element in a State",
                )
            raise Untranslatable(
                node,
                f"`{src}` is not an indexed read the dialect knows yet — index a list "
                f"through a local (`xs = {self._src(node.value)}`; `xs[0]`), with a literal index",
            )
        key = node.slice.value
        if key not in self.state:
            raise Untranslatable(node, f"`{key}` is not a state key")
        return key if ctx == "store" else f"App.{key}"

    @staticmethod
    def _splice(inner: str) -> str:
        """A hole whose expression turned out to be a string literal is
        written as text: `"#{\"a\"}"` would end the string at the inner
        quote."""
        if len(inner) >= 2 and inner[0] == '"' and inner[-1] == '"' and '"' not in inner[1:-1].replace('\\"', ""):
            return inner[1:-1]
        return "#{" + inner + "}"

    def _str_parts(self, node, ctx, param) -> str:
        """A String expression as pixie interpolation: `name() + "!"`
        becomes `#{App.name}!`, so a literal inside a concatenation
        never has to survive as a quote inside a quote."""
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            return self._str_parts(node.left, ctx, param) + self._str_parts(node.right, ctx, param)
        if isinstance(node, ast.Constant) and type(node.value) is str:
            return esc(node.value)
        return self._splice(self._hole(node, ctx, param))

    def _text_value(self, node) -> str:
        """A Text's text: literal/f-string, or a bare String-typed ref
        (a str cell read, or the row variable over a List<String>)."""
        c = self._cell_read(node)
        if c is not None and self._ty(c) == "String":
            return f"App.{c}"
        if (
            self.comp_params is not None
            and isinstance(node, ast.Name)
            and self.comp_params.get(node.id) == "String"
        ):
            return node.id
        if (
            self.row is not None
            and isinstance(node, ast.Subscript)
            and isinstance(node.slice, ast.Name)
            and node.slice.id == self.row[1]
            and self._row_source_read(node.value)
        ):
            if self._row_elem_ty() != "List<String>":
                raise Untranslatable(node, 'a row renders a str list\'s element as text (`text(items()[i])`) — for other lists, format in a hole (`text(f"{nums()[i]}")`)')
            return self.row[2]
        if not isinstance(node, (ast.Constant, ast.JoinedStr)) and self._num_ty(node, "view", None) == "String":
            # A str-typed expression is text: `text(Cart.label)` says
            # the same thing as `text(f"{Cart.label}")`, and reads
            # better, so it lowers to the same interpolation.
            return '"' + self._str_parts(node, "view", None) + '"'
        return self.fstring(node)

    def fstring(self, node) -> str:
        if isinstance(node, ast.Constant) and isinstance(node.value, str):
            return f'"{esc(node.value)}"'
        if not isinstance(node, ast.JoinedStr):
            raise Untranslatable(node, 'text(...) takes a str literal or an f-string — a str state or field goes in a hole (`text(f"{Cart.label}")`) until bare str expressions land')
        out = '"'
        for part in node.values:
            if isinstance(part, ast.Constant):
                out += esc(part.value)
            elif isinstance(part, ast.FormattedValue):
                if part.conversion != -1:
                    raise Untranslatable(part, "`!r` / `!s` conversions are not in the dialect — a hole renders `str()` already")
                if part.format_spec is not None:
                    spec = self._spec_text(part)
                    if re.fullmatch(r"\.\d+f", spec):
                        # The engine formats fixed decimals itself.
                        self._reject_bool_text(part.value)
                        out += "#{" + self.expr(part.value, "view") + ":" + spec + "}"
                    else:
                        out += "#{" + self._format_call(part.value, spec, "view", None) + "}"
                else:
                    out += self._splice(self._hole(part.value, "view", None))
            else:
                raise Untranslatable(part, "this f-string part is not in the dialect — a hole holds a state read, a field, a parameter or `+ - *` arithmetic over them")
        return out + '"'

    # ---- handlers --------------------------------------------------

    def _scoped_handler(self, node, takes_text: bool, extras):
        """A handler that reads something living in the VIEW's scope —
        a repeater's row index, a component's parameter — becomes a
        store fn that takes it, with the view passing it at the call
        site. `extras` is [(name, pix type, what to pass)]."""
        if takes_text:
            raise Untranslatable(node, "a callback that takes a value and also reads the row index or a component parameter is not in the dialect yet — use a button")
        if isinstance(node, ast.Lambda):
            if node.args.args:
                raise Untranslatable(node, "on_click takes a zero-argument callable")
            body = [at(ast.Expr(node.body), node.body)]
        elif isinstance(node, ast.Name) and node.id in self.defs:
            body = self.defs[node.id].body
        else:
            raise Untranslatable(node, "a handler here is a lambda or a module-level def")
        prev_row, prev_locals, prev_typed = self.row, self.handler_locals, self.typed_locals
        prev_comp, prev_nonneg = self.comp_params, set(self.nonneg_loop_vars)
        self.row = None
        self.comp_params = None
        self.handler_locals = {n for n, _t, _v in extras}
        self.typed_locals = {**prev_typed, **{n: t for n, t, _v in extras}}
        for n, t, _v in extras:
            if t == "Int":
                self.nonneg_loop_vars.discard(n)
        try:
            stmts = []
            for st in body:
                stmts += self._stmt(st, None)
        finally:
            self.row, self.handler_locals, self.typed_locals = prev_row, prev_locals, prev_typed
            self.comp_params, self.nonneg_loop_vars = prev_comp, prev_nonneg
        name = f"h{len(self.handlers)}"
        if self.async_hit:
            self.async_handlers.add(name)
            self.async_hit = False
        self.handlers.append((name, [(n, t) for n, t, _v in extras], stmts))
        return f"App.{name}({', '.join(v for _n, _t, v in extras)})"

    def _mentions_row(self, node) -> bool:
        """Does this handler read the row or its index?"""
        if self.row is None:
            return False
        names = {self.row[1]}
        return any(isinstance(n, ast.Name) and n.id in names for n in ast.walk(node))

    def handler(self, node, takes_text: bool, implicit=("text", "String")):
        """Synthesize a store fn and return the view-side call string —
        or, inside a component, an ("inline", [lines]) block so locals
        and params stay in scope (cards' `onClick: { n = n + step }`)."""
        if self.comp_params is not None:
            return self._inline_handler(node, takes_text, implicit)
        if self.row is not None and self._mentions_row(node):
            return self._scoped_handler(node, takes_text, [(self.row[1], "Int", self.row[3])])
        iname, ity = implicit
        param = None
        body = None
        if (
            isinstance(node, ast.Attribute)
            and node.attr == "set"
            and isinstance(node.value, ast.Name)
            and node.value.id in self.cells
        ):
            if not takes_text:
                raise Untranslatable(node, "`x.set` as a handler needs a slot that passes a value (on_change=) — on_click passes none; write `lambda: x.set(...)`")
            name = f"h{len(self.handlers)}"
            self.handlers.append((name, ity, [f"{node.value.id} = t"]))
            return f"App.{name}({iname})"
        if isinstance(node, ast.Lambda):
            args = node.args.args
            if takes_text:
                if len(args) != 1:
                    raise Untranslatable(node, "this callback takes exactly one argument (the new value)")
                param = args[0].arg
            elif args:
                raise Untranslatable(node, "on_click takes a zero-argument callable")
            body = [at(ast.Expr(node.body), node.body)]
        elif (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.stores
            and node.attr in self.stores[node.value.id]["method_names"]
        ):
            # `on_click=Cart.clear` — a bound store method, called direct.
            m = self._smeth(node.value.id, node.attr)
            if takes_text:
                return f"{node.value.id}.{m}({iname})"
            return f"{node.value.id}.{m}()"
        elif isinstance(node, ast.Name) and node.id in self.defs:
            d = self.defs[node.id]
            if takes_text:
                if len(d.args.args) != 1:
                    raise Untranslatable(node, "this callback takes exactly one argument (the new value)")
                param = d.args.args[0].arg
            body = d.body
        else:
            raise Untranslatable(node, "a handler is a lambda, a module-level def, or a store's bound method")

        stmts = []
        self.handler_locals = set()
        self.dead_locals = {}
        prev_pty = self.typed_locals.get(param) if param else None
        if param:
            self.typed_locals[param] = ity
        try:
            for stmt in body:
                stmts += self._stmt(stmt, param)
        finally:
            self.handler_locals = set()
            self.dead_locals = {}
            if param:
                if prev_pty is None:
                    self.typed_locals.pop(param, None)
                else:
                    self.typed_locals[param] = prev_pty
        name = f"h{len(self.handlers)}"
        if self.async_hit:
            self.async_handlers.add(name)
            self.async_hit = False
        self.handlers.append((name, ity if param else None, stmts))
        return f"App.{name}({iname})" if param else f"App.{name}()"

    def _inline_handler(self, node, takes_text: bool, implicit=("text", "String")):
        param = None
        if (
            isinstance(node, ast.Attribute)
            and node.attr == "set"
            and isinstance(node.value, ast.Name)
        ):
            name = node.value.id
            if self.comp_locals and name in self.comp_locals:
                if not takes_text:
                    raise Untranslatable(node, "`x.set` as a handler needs a slot that passes a value (on_change=) — on_click passes none; write `lambda: x.set(...)`")
                return ("inline", [f"{name} = text"])
            node = at(
                ast.Lambda(
                    args=ast.arguments(posonlyargs=[], args=[ast.arg(arg="t")], kwonlyargs=[], kw_defaults=[], defaults=[]),
                    body=ast.Call(func=node, args=[ast.Name(id="t", ctx=ast.Load())], keywords=[]),
                ),
                node,
            )
        if isinstance(node, ast.Lambda):
            args = node.args.args
            if takes_text:
                if len(args) != 1:
                    raise Untranslatable(node, "this callback takes exactly one argument (the new value)")
                param = args[0].arg
            elif args:
                raise Untranslatable(node, "on_click takes a zero-argument callable")
            body = [at(ast.Expr(node.body), node.body)]
        elif isinstance(node, ast.Name) and node.id in self.defs:
            d = self.defs[node.id]
            if takes_text:
                if len(d.args.args) != 1:
                    raise Untranslatable(node, "this callback takes exactly one argument (the new value)")
                param = d.args.args[0].arg
            body = d.body
        else:
            raise Untranslatable(node, "a handler is a lambda, a module-level def, or a store's bound method")

        iname, ity = implicit
        prev_inline = self.inline_text_param
        prev_iname = self.inline_implicit_name
        self.inline_text_param = param
        self.inline_implicit_name = iname
        local_lines: list[str] = []
        cell_stmts: list[str] = []
        cell_reads: list[str] = []
        cell_uses_param = False
        try:
            for stmt in body:
                for piece in self._flatten(stmt):
                    kind, line, used, reads = self._route_inline(piece, param)
                    if kind == "local":
                        local_lines.append(line)
                    else:
                        cell_stmts.append(line)
                        cell_uses_param = cell_uses_param or used
                        for r in reads:
                            if r not in cell_reads:
                                cell_reads.append(r)
        finally:
            self.inline_text_param = prev_inline
            self.inline_implicit_name = prev_iname
        if cell_stmts:
            name = f"h{len(self.handlers)}"
            if self.async_hit:
                self.async_handlers.add(name)
                self.async_hit = False
            sig = ([("t", ity)] if cell_uses_param else []) + [
                (n, (self.comp_params or {}).get(n, "String")) for n in cell_reads
            ]
            self.handlers.append((name, sig or None, cell_stmts))
            call = ([iname] if cell_uses_param else []) + cell_reads
            local_lines.append(f"App.{name}({', '.join(call)})")
        if len(local_lines) == 1 and local_lines[0].startswith("App.h"):
            return local_lines[0]
        if not local_lines:
            raise Untranslatable(node, "a handler needs at least one statement")
        return ("inline", local_lines)

    def _flatten(self, stmt):
        if isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Tuple):
            return [at(ast.Expr(e), stmt) for e in stmt.value.elts]
        return [stmt]

    def _route_inline(self, stmt, param):
        """One handler statement inside a component. Local writes stay
        inline (params render as `text`); cell/bundle writes go to a
        synthesized store fn — which takes `t` when the statement uses
        the callback's parameter (`title.set(t)` works)."""
        prev_inline = self.inline_handler
        self.inline_handler = True
        try:
            return self._route_inline_guarded(stmt, param)
        finally:
            self.inline_handler = prev_inline

    def _route_inline_guarded(self, stmt, param):
        if isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Call):
            call = stmt.value
            f = call.func
            if (
                isinstance(f, ast.Attribute)
                and f.attr == "set"
                and isinstance(f.value, ast.Name)
                and self.comp_locals
                and f.value.id in self.comp_locals
            ):
                if len(call.args) != 1:
                    raise Untranslatable(call, "`.set` takes exactly one value")
                return ("local", f"{f.value.id} = {self.expr(call.args[0], 'view', param)}", False, [])
        uses_param = param is not None and any(
            isinstance(n, ast.Name) and n.id == param for n in ast.walk(stmt)
        )
        reads = [
            n for n in (self.comp_params or {})
            if any(isinstance(x, ast.Name) and x.id == n for x in ast.walk(stmt))
        ]
        saved = self.inline_text_param
        self.inline_text_param = None
        prev_locals, prev_typed, prev_comp = self.handler_locals, self.typed_locals, self.comp_params
        if reads:
            self.handler_locals = set(prev_locals) | set(reads)
            self.typed_locals = {**prev_typed, **{n: prev_comp[n] for n in reads}}
            self.comp_params = None
        try:
            lines = self._stmt(stmt, param)
        finally:
            self.inline_text_param = saved
            self.handler_locals, self.typed_locals, self.comp_params = prev_locals, prev_typed, prev_comp
        if len(lines) != 1:
            raise Untranslatable(stmt, "a handler inside a component body takes one statement — use a def handler for more")
        return ("cell", lines[0], uses_param, reads)

    def _stmt(self, stmt, param) -> list[str]:
        """One statement, with a place for the lines an expression
        inside it needs first: a model's method is called on a local,
        so reading a value out of one binds the receiver above."""
        prev, self.pre_lines = self.pre_lines, []
        try:
            lines = self._stmt_inner(stmt, param)
            return self.pre_lines + lines
        finally:
            self.pre_lines = prev

    def _stmt_inner(self, stmt, param) -> list[str]:
        if isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Tuple):
            out = []
            for e in stmt.value.elts:
                out += self._stmt(at(ast.Expr(e), stmt), param)
            return out
        if isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Call):
            call = stmt.value
            f = call.func
            if (
                isinstance(f, ast.Attribute)
                and f.attr == "set"
                and isinstance(f.value, ast.Name)
                and f.value.id in self.cells
            ):
                if len(call.args) != 1:
                    raise Untranslatable(call, "`.set` takes exactly one value")
                cell = f.value.id
                arg = call.args[0]
                if isinstance(arg, ast.Constant) and arg.value is None:
                    if not self._ty(cell).endswith("?"):
                        raise Untranslatable(arg, f"`{cell}` is not optional — annotate it `T | None`")
                    return [f"{cell} = nil"]
                # `xs.set(xs() + [e])` → xs.push(e)
                if (
                    isinstance(arg, ast.BinOp)
                    and isinstance(arg.op, ast.Add)
                    and self._cell_read(arg.left) == cell
                    and isinstance(arg.right, ast.List)
                    and len(arg.right.elts) == 1
                ):
                    return [f"{cell}.push({self.expr(arg.right.elts[0], 'store', param)})"]
                # `xs.set([...])` → xs = [...]
                if isinstance(arg, ast.List):
                    ty = self.cells[cell]
                    if not ty.startswith("List<"):
                        raise Untranslatable(arg, "assigning a list to a state that is not a list")
                    return [f"{cell} = {self._list_literal(arg, ty[5:-1])}"]
                if isinstance(arg, ast.Dict):
                    ty = self.cells[cell]
                    if not ty.startswith("Map<"):
                        raise Untranslatable(arg, "assigning a dict to a state that is not a dict")
                    return [f"{cell} = {self._dict_literal(arg, ty.split(', ', 1)[1][:-1])}"]
                return [f"{cell} = {self.expr(arg, 'store', param)}"]
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Subscript)
            and isinstance(stmt.targets[0].value, ast.Attribute)
            and isinstance(stmt.targets[0].value.value, ast.Name)
            and stmt.targets[0].value.value.id == "self"
            and self.model_scope is not None
        ):
            t = stmt.targets[0]
            fld = t.value.attr
            fty = self.model_scope[1].get(fld, "")
            if fty.startswith("Map<"):
                key = self._dict_key(t.slice, "store", param)
                return [f'{fld}[{key}] = {self.expr(stmt.value, "store", param)}']
            if fty.startswith("List<"):
                return [f"{fld}[{self._write_index(t, param, fld)}] = {self.expr(stmt.value, 'store', param)}"]
            raise Untranslatable(t, f"`{fld}` is not a list or a dict, so `{fld}[...] = ...` has nothing to write to")
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Subscript)
            and isinstance(stmt.targets[0].value, ast.Name)
            and stmt.targets[0].value.id in self.cells
        ):
            t = stmt.targets[0]
            cell = t.value.id
            ty = self.cells[cell]
            val = self.expr(stmt.value, "store", param)
            if ty.startswith("Map<"):
                return [f"{cell}[{self._dict_key(t.slice, 'store', param)}] = {val}"]
            if ty.startswith("List<"):
                return [f"{cell}[{self._write_index(t, param, cell)}] = {val}"]
            raise Untranslatable(t, f"`{cell}` is not a list or a dict, so `{cell}[...] = ...` has nothing to write to")
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and (self._stdlib_call(stmt.value.func) is not None
                 or self._crate_call(stmt.value.func) is not None)
        ):
            code = self.expr(stmt.value, "store", param)
            name = f"__fs{len(self.handler_locals)}"
            self.handler_locals.add(name)
            return [f"var {name} = {code}"]
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Name)
        ):
            name = stmt.targets[0].id
            if name not in self.cells:
                self._check_name(stmt.targets[0], name, "local")
            if isinstance(stmt.value, ast.List) and name not in self.cells:
                raise Untranslatable(
                    stmt,
                    "a local list needs its element type — annotate it "
                    "(`out: list[str] = []`), which is what the compiled app reads",
                )
            if (
                name in self.handler_locals
                and self.typed_locals.get(name, "").startswith("List<")
                and isinstance(stmt.value, ast.BinOp)
                and isinstance(stmt.value.op, ast.Add)
                and isinstance(stmt.value.left, ast.Name)
                and stmt.value.left.id == name
                and isinstance(stmt.value.right, ast.List)
                and len(stmt.value.right.elts) == 1
            ):
                # `out = out + [x]` is an append, and the compiled app
                # appends rather than rebuilding the list.
                el = self.typed_locals[name][5:-1]
                got = self._num_ty(stmt.value.right.elts[0], "store", param)
                if got is not None and got != el:
                    raise Untranslatable(stmt.value.right.elts[0], f"this list holds {py_ty(el)} — the value is {py_ty(got)}")
                return [f"{name}.push({self.expr(stmt.value.right.elts[0], 'store', param)})"]
            value = self.expr(stmt.value, "store", param)
            vt = self._num_ty(stmt.value, "store", param)
            if vt is None:
                src = self._list_of(stmt.value, "store", param)
                if src is not None:
                    vt = f"List<{src[1]}>"
            if vt is not None and (
                vt in ("Int", "Float", "Bool", "String")
                or vt in self.models
                or vt in self.structs
                or vt in self.enums
                or vt.startswith("Map<")
                or vt.startswith("List<")
                or vt.endswith("?")
            ):
                self.typed_locals[name] = vt
            if name in self.handler_locals:
                # Python locals are mutable; the binding was emitted
                # as `var`, so a plain reassignment is exactly right.
                return [f"{name} = {value}"]
            self.handler_locals.add(name)
            return [f"var {name} = {value}"]
        if (
            isinstance(stmt, ast.AnnAssign)
            and isinstance(stmt.target, ast.Name)
            and stmt.value is not None
            and (lty := self._param_ty(stmt.annotation)) is not None
            and lty.startswith("List<")
        ):
            # `out: list[str] = []` — a local list. The annotation is
            # what tells the compiled side its element type, so the
            # dialect asks for it here as it does for a state.
            name = stmt.target.id
            lit = self._list_literal(stmt.value, lty[5:-1]) if isinstance(stmt.value, ast.List) else None
            if lit is None:
                src = self._list_of(stmt.value, "store", param)
                if src is None or f"List<{src[1]}>" != lty:
                    raise Untranslatable(stmt.value, f"a `{py_ty(lty)}` local starts from a list literal or another list of the same type")
                lit = src[0]
            self.handler_locals.add(name)
            self.typed_locals[name] = lty
            return [f"var {name} : {lty} = {lit}"]
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Tuple)
        ):
            names = stmt.targets[0].elts
            vals = stmt.value.elts if isinstance(stmt.value, ast.Tuple) else None
            if vals is None or len(vals) != len(names) or not all(isinstance(n, ast.Name) for n in names):
                raise Untranslatable(
                    stmt,
                    "a tuple assignment binds plain names from as many values "
                    "(`a, b = 1, 2`) — the dialect has no tuple to unpack",
                )
            # Python evaluates the right side first, so the values are
            # bound to temporaries before any name is written — which
            # is what makes `a, b = b, a` a swap.
            lines, tmps = [], []
            for i, v in enumerate(vals):
                tmp = f"__tu{len(self.handler_locals)}_{i}"
                self.handler_locals.add(tmp)
                vt = self._num_ty(v, "store", param)
                if vt is not None:
                    self.typed_locals[tmp] = vt
                lines += self._stmt(at(ast.Assign(targets=[ast.Name(id=tmp, ctx=ast.Store())], value=v), stmt), param)
                tmps.append((tmp, vt))
            for n, (tmp, vt) in zip(names, tmps):
                lines += self._stmt(at(ast.Assign(targets=[ast.Name(id=n.id, ctx=ast.Store())], value=ast.Name(id=tmp, ctx=ast.Load())), stmt), param)
            return lines
        if isinstance(stmt, ast.Assign) and len(stmt.targets) == 1:
            t = stmt.targets[0]
            if isinstance(t, ast.Subscript):
                key = self._field(t, "store")
                return [f"{key} = {self.expr(stmt.value, 'store', param)}"]
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Attribute)
            and isinstance(stmt.targets[0].value, ast.Name)
            and stmt.targets[0].value.id in self.stores
        ):
            # `Cart.total = 5` from outside the store. Python allows
            # it and so does the compiled side; a method is the good
            # habit where an update has an invariant, not a rule the
            # language can hold up.
            t0 = stmt.targets[0]
            sname = t0.value.id
            ftys = self.stores[sname]["field_tys"]
            if t0.attr not in ftys:
                raise Untranslatable(t0, f"`{t0.attr}` is not a field of store {sname}")
            v0 = stmt.value
            if isinstance(v0, ast.Constant) and v0.value is None:
                if not ftys[t0.attr].endswith("?"):
                    raise Untranslatable(v0, f"`{t0.attr}` is not optional — annotate it `T | None`")
                return [f"{sname}.{t0.attr} = nil"]
            if isinstance(v0, ast.List) and ftys[t0.attr].startswith("List<"):
                return [f"{sname}.{t0.attr} = {self._list_literal(v0, ftys[t0.attr][5:-1])}"]
            if isinstance(v0, ast.Dict) and ftys[t0.attr].startswith("Map<"):
                return [f"{sname}.{t0.attr} = {self._dict_literal(v0, ftys[t0.attr].split(', ', 1)[1][:-1])}"]
            return [f"{sname}.{t0.attr} = {self.expr(v0, 'store', param)}"]
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Attribute)
            and isinstance(stmt.targets[0].value, ast.Name)
            and (
                self.typed_locals.get(stmt.targets[0].value.id) in self.models
                or stmt.targets[0].value.id in self.model_instances
            )
        ):
            t0 = stmt.targets[0]
            base = t0.value.id
            mn = self.typed_locals.get(base) or self.model_instances[base]
            mfields = {f: t for f, t, _ in self.models[mn]["fields"]}
            if t0.attr not in mfields:
                raise Untranslatable(t0, f"`{t0.attr}` is not a field of {mn}")
            fty0 = mfields[t0.attr]
            v0 = stmt.value
            if isinstance(v0, ast.Constant) and v0.value is None:
                if not fty0.endswith("?"):
                    raise Untranslatable(v0, f"`{t0.attr}` is not optional — annotate it `T | None` to narrow it")
                return [f"{base}.{t0.attr} = nil"]
            if isinstance(v0, (ast.List, ast.Dict)):
                return [f"{base}.{t0.attr} = {self._literal_of(v0, fty0)}"]
            return [f"{base}.{t0.attr} = {self.expr(v0, 'store', param)}"]
        if (
            isinstance(stmt, ast.Assign)
            and len(stmt.targets) == 1
            and isinstance(stmt.targets[0], ast.Attribute)
            and isinstance(stmt.targets[0].value, ast.Name)
            and stmt.targets[0].value.id == "self"
            and self.model_scope is not None
        ):
            fld = stmt.targets[0].attr
            if fld not in self.model_scope[1]:
                raise Untranslatable(stmt.targets[0], f"`{fld}` is not a field of {self.model_scope[0]}")
            v = stmt.value
            if isinstance(v, ast.Constant) and v.value is None:
                if not self.model_scope[1].get(fld, "").endswith("?"):
                    raise Untranslatable(v, f"`{fld}` is not optional — annotate it `T | None`")
                return [f"{fld} = nil"]
            # `self.xs = self.xs + [e]` → xs.push(e), the cell idiom
            if (
                isinstance(v, ast.BinOp)
                and isinstance(v.op, ast.Add)
                and isinstance(v.left, ast.Attribute)
                and isinstance(v.left.value, ast.Name)
                and v.left.value.id == "self"
                and v.left.attr == fld
                and isinstance(v.right, ast.List)
                and len(v.right.elts) == 1
            ):
                return [f"{fld}.push({self.expr(v.right.elts[0], 'store', param)})"]
            if isinstance(v, ast.List) and self.model_scope[1][fld].startswith("List<"):
                return [f"{fld} = {self._list_literal(v, self.model_scope[1][fld][5:-1])}"]
            fty = self.model_scope[1][fld]
            if isinstance(v, ast.Dict) and fty.startswith("Map<"):
                return [f"{fld} = {self._dict_literal(v, fty[fty.index(', ') + 2:-1])}"]
            return [f"{fld} = {self.expr(v, 'store', param)}"]
        if (
            isinstance(stmt, ast.AugAssign)
            and isinstance(stmt.op, (ast.Add, ast.Sub, ast.Mult))
            and isinstance(stmt.target, ast.Attribute)
            and isinstance(stmt.target.value, ast.Name)
            and stmt.target.value.id == "self"
            and self.model_scope is not None
        ):
            fld = stmt.target.attr
            if fld not in self.model_scope[1]:
                raise Untranslatable(stmt.target, f"`{fld}` is not a field of {self.model_scope[0]}")
            op = {ast.Add: "+=", ast.Sub: "-=", ast.Mult: "*="}[type(stmt.op)]
            return [f"{fld} {op} {self.expr(stmt.value, 'store', param)}"]
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and isinstance(stmt.value.func, ast.Attribute)
            and isinstance(stmt.value.func.value, ast.Name)
            and stmt.value.func.value.id == "self"
            and self.model_scope is not None
            and self.model_scope[0] in self.stores
        ):
            call = stmt.value
            sname = self.model_scope[0]
            meth = self._smeth(sname, call.func.attr)
            ordered = self._call_args(call, self.stores[sname]["params"].get(call.func.attr, []), f"{sname}.{call.func.attr}", self.stores[sname]["defaults"].get(call.func.attr))
            args = ", ".join(self.expr(a, "store", param) for a in ordered)
            return [f"{sname}.{meth}({args})"]
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and isinstance(stmt.value.func, ast.Attribute)
            and isinstance(stmt.value.func.value, ast.Name)
            and stmt.value.func.value.id in self.stores
        ):
            call = stmt.value
            sname = call.func.value.id
            meth = call.func.attr
            if meth not in self.stores[sname]["method_names"]:
                raise Untranslatable(call, f"`{meth}` is not a method of store {sname}")
            ordered = self._call_args(call, self.stores[sname]["params"].get(meth, []), f"{sname}.{meth}", self.stores[sname]["defaults"].get(meth))
            args = ", ".join(self.expr(a, "store", param) for a in ordered)
            return [f"{sname}.{self._smeth(sname, meth)}({args})"]
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and isinstance(stmt.value.func, ast.Attribute)
            and isinstance(stmt.value.func.value, ast.Name)
            and stmt.value.func.value.id in self.model_instances
        ):
            call = stmt.value
            inst = call.func.value.id
            mname = self.model_instances[inst]
            known = {m.strip().split("(")[0].replace("  pub fn ", "") for lines in self.models[mname]["methods"] for m in lines[:1]}
            known |= {ln[0].strip().split("(")[0].replace("pub fn ", "").split(" ")[0] for tl in self.models[mname]["impls"].values() for ln in tl}
            meth = call.func.attr
            ordered = self._call_args(call, self.models[mname]["params"].get(meth, []), f"{inst}.{meth}", self.models[mname]["defaults"].get(meth))
            args = ", ".join(self.expr(a, "store", param) for a in ordered)
            tmp = f"__o{len(self.handler_locals)}"
            self.handler_locals.add(tmp)
            return [f"var {tmp} = {inst}", f"{tmp}.{self._mmeth(mname, meth)}({args})"]
        if isinstance(stmt, ast.Pass):
            return []
        # A docstring (or any bare string-literal statement) is a
        # no-op in both tiers.
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Constant)
            and isinstance(stmt.value.value, str)
        ):
            return []
        if isinstance(stmt, ast.If):
            opt = self._optional_test(stmt.test)
            if opt is not None:
                src, binding, none_first = opt
                kind, key = src
                if kind == "local":
                    subject, sub_ty = key, self.typed_locals[key]
                elif kind == "cell":
                    subject, sub_ty = key, self._ty(key)
                elif kind == "selffield":
                    subject, sub_ty = key, self.model_scope[1][key]
                elif kind == "instfield":
                    iname, fname = key.split(".", 1)
                    mi = self.model_instances[iname]
                    subject = key
                    sub_ty = {f: t for f, t, _ in self.models[mi]["fields"]}[fname]
                elif kind == "localfield":
                    iname, fname = key.split(".", 1)
                    mi = self.typed_locals[iname]
                    subject = key
                    sub_ty = {f: t for f, t, _ in self.models[mi]["fields"]}[fname]
                else:
                    sname, fname = key.split(".", 1)
                    subject, sub_ty = key, self.stores[sname]["field_tys"][fname]
                some_body = stmt.orelse if none_first else stmt.body
                none_body = stmt.body if none_first else stmt.orelse
                # A local narrows under its OWN name: `if v is not
                # None:` reads `v` in the branch, and Python means the
                # value there, not the optional.
                v = binding or (key if kind == "local" else "__opt")
                self.handler_locals.add(v)
                prev_ty = self.typed_locals.get(v)
                self.typed_locals[v] = sub_ty[:-1]
                lines = [f"if let some({v}) = {subject} {{"]
                if some_body:
                    lines += self._block(some_body, param)
                self.handler_locals.discard(v)
                if prev_ty is None:
                    self.typed_locals.pop(v, None)
                else:
                    self.typed_locals[v] = prev_ty
                self.dead_locals[v] = f"`{v}` is bound inside the some-branch only"
                if none_body:
                    lines.append("} else {")
                    lines += self._block(none_body, param)
                lines.append("}")
                return lines
            # A local assigned on BOTH arms of an if/else (elif chains
            # included) outlives the branch, exactly as in Python: it
            # is hoisted above the if with a typed zero. One-armed
            # assignments still refuse the later read — there Python
            # raises where the native side would read the zero.
            hoist_lines = []
            if stmt.orelse:
                def _chain_assigned(body):
                    names = set()
                    for st in body:
                        if (
                            isinstance(st, ast.Assign)
                            and len(st.targets) == 1
                            and isinstance(st.targets[0], ast.Name)
                        ):
                            names.add(st.targets[0].id)
                    if len(body) == 1 and isinstance(body[0], ast.If) and body[0].orelse:
                        names |= _chain_assigned(body[0].body) & _chain_assigned(body[0].orelse)
                    return names

                both = _chain_assigned(stmt.body) & _chain_assigned(stmt.orelse)
                for n in sorted(both):
                    if n in self.handler_locals or n in self.cells:
                        continue
                    rhs = next(
                        (
                            st.value
                            for st in stmt.body
                            if isinstance(st, ast.Assign)
                            and isinstance(st.targets[0], ast.Name)
                            and st.targets[0].id == n
                        ),
                        None,
                    )
                    ty = self._num_ty(rhs, "store", param) if rhs is not None else None
                    if ty is None:
                        continue  # unknown type: the old block-scope rule stands
                    hoist_lines.append(f"var {n} = {self._zero_of(ty)}")
                    self.handler_locals.add(n)
                    self.typed_locals[n] = ty
            lines = hoist_lines + [f"if {self._cond(stmt.test, 'store', param)} {{"]
            lines += self._block(stmt.body, param)
            if stmt.orelse:
                lines.append("} else {")
                lines += self._block(stmt.orelse, param)
            lines.append("}")
            return lines
        if isinstance(stmt, ast.While):
            if stmt.orelse:
                raise Untranslatable(stmt, "while-else is not in the dialect yet")
            # A while test runs on every turn of the loop, so an
            # expression that would need a line before the statement
            # has nowhere to put it: no prelude here.
            prev_pre, self.pre_lines = self.pre_lines, None
            try:
                lines = [f"while {self._cond(stmt.test, 'store', param)} {{"]
            finally:
                self.pre_lines = prev_pre
            lines += self._block(stmt.body, param)
            lines.append("}")
            return lines
        if isinstance(stmt, ast.For):
            return self._handler_for(stmt, param)
        if isinstance(stmt, ast.Try):
            return self._lower_try(stmt, param)
        if isinstance(stmt, ast.Match):
            subj = self.expr(stmt.subject, "store", param)
            ety = None
            c = self._cell_read(stmt.subject)
            if c is not None:
                ety = self._ty(c)
            elif (
                isinstance(stmt.subject, ast.Attribute)
                and isinstance(stmt.subject.value, ast.Name)
                and stmt.subject.value.id == "self"
                and self.model_scope is not None
            ):
                ety = self.model_scope[1].get(stmt.subject.attr)
            elif (
                isinstance(stmt.subject, ast.Attribute)
                and isinstance(stmt.subject.value, ast.Name)
                and stmt.subject.value.id in self.stores
            ):
                ety = self.stores[stmt.subject.value.id]["field_tys"].get(stmt.subject.attr)
            if ety in self.unions:
                return self._union_match(stmt, subj, ety, param)
            if ety not in self.enums:
                lit = self._match_literals(stmt, subj, param)
                if lit is not None:
                    return lit
                raise Untranslatable(stmt.subject, "match takes an Enum, a sum type, or a value of int, float, str or bool — this subject is none of those here")
            lines = [f"case {subj} {{"]
            for case in stmt.cases:
                if case.guard is not None:
                    raise Untranslatable(case.pattern, "a match guard (`case X if cond:`) is not in the dialect yet — test the condition inside the arm")
                p = case.pattern
                if (
                    isinstance(p, ast.MatchValue)
                    and isinstance(p.value, ast.Attribute)
                    and isinstance(p.value.value, ast.Name)
                    and p.value.value.id == ety
                ):
                    lines.append(f"  when {p.value.attr} {{")
                elif isinstance(p, ast.MatchAs) and p.pattern is None and p.name is None:
                    lines.append("  when _ {")
                else:
                    raise Untranslatable(p, "a match arm is `Mood.MEMBER` or `_` — `|` patterns and literals are not in the dialect yet")
                for inner in self._block(case.body, param):
                    lines.append("  " + inner)
                lines.append("  }")
            lines.append("}")
            return lines
        if isinstance(stmt, ast.Break):
            return ["break"]
        if isinstance(stmt, ast.Continue):
            return ["continue"]
        # An early return: bare when the body returns nothing, with a
        # value when the signature says one comes back.
        if isinstance(stmt, ast.Return) and stmt.value is None:
            return ["return"]
        if isinstance(stmt, ast.Return) and self.ret_ty is not None:
            return [f"return {self.expr(stmt.value, 'store', param)}"]
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and self._is_ui(stmt.value.func, "log")
        ):
            call = stmt.value
            if len(call.args) != 1 or call.keywords:
                raise Untranslatable(call, "log() takes one message")
            self.uses_stdlib = True
            name = f"__lg{len(self.handler_locals)}"
            self.handler_locals.add(name)
            msg = '"' + self._str_parts(call.args[0], "store", param) + '"'
            return [f"var {name} = Py.log({msg})"]
        if isinstance(stmt, ast.Assert):
            if self.in_try:
                raise Untranslatable(stmt, "an `assert` inside a `try` is not in the dialect — the two runs would disagree about what the `except` catches")
            self.uses_stdlib = True
            cond = self._cond(stmt.test, "store", param)
            msg = "assertion failed"
            if stmt.msg is not None:
                msg = self._str_parts(stmt.msg, "store", param)
            name = f"__as{len(self.handler_locals)}"
            self.handler_locals.add(name)
            return [f"if !({cond}) {{", f'  var {name} = Py.abort("{msg}")', "}"]
        if isinstance(stmt, ast.Raise):
            if self.in_try:
                raise Untranslatable(stmt, "a `raise` inside a `try` is not in the dialect — the two runs would disagree about what the `except` catches")
            if stmt.cause is not None:
                raise Untranslatable(stmt, "`raise ... from ...` is not in the dialect")
            self.uses_stdlib = True
            what = "raised"
            exc = stmt.exc
            if isinstance(exc, ast.Call) and isinstance(exc.func, ast.Name):
                inner = ""
                if len(exc.args) == 1:
                    inner = ": " + self._str_parts(exc.args[0], "store", param)
                elif exc.args:
                    raise Untranslatable(exc, "a raised exception takes one message")
                what = f"{exc.func.id}{inner}"
            elif isinstance(exc, ast.Name):
                what = exc.id
            elif exc is not None:
                raise Untranslatable(exc, "`raise` takes an exception (`raise ValueError(\"…\")`)")
            else:
                raise Untranslatable(stmt, "a bare `raise` re-raises the exception being handled — the dialect has no handler to re-raise from")
            name = f"__rs{len(self.handler_locals)}"
            self.handler_locals.add(name)
            return [f'var {name} = Py.abort("{what}")']
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and self._is_ui(stmt.value.func, "every")
        ):
            raise Untranslatable(
                stmt,
                "a timer is declared, not started: write `every(1.0, tick)` at module "
                "level and let the handler set what the tick reads",
            )
        if (
            isinstance(stmt, ast.Expr)
            and isinstance(stmt.value, ast.Call)
            and self._is_ui(stmt.value.func, "task")
        ):
            return self._task_stmt(stmt.value, param)
        raise Untranslatable(stmt, self._unknown_stmt(stmt))

    def _task_stmt(self, call: ast.Call, param):
        """`task(work, on_done=...)` — the handler around it becomes an
        `async fn`, the work's standard-library calls are awaited (which
        is what moves them off the UI thread), and `on_done` runs where
        the work's value lands. The statements after a task would run
        BEFORE the work finishes in Python, so a task is the last
        statement of its handler."""
        kw = {k.arg: k.value for k in call.keywords if k.arg}
        args = list(call.args)
        work = args[0] if args else kw.get("work")
        on_done = args[1] if len(args) > 1 else kw.get("on_done")
        on_error = args[2] if len(args) > 2 else kw.get("on_error")
        if work is None:
            raise Untranslatable(call, "task(work, on_done=...) takes the work to run")
        if on_error is not None:
            raise Untranslatable(
                on_error,
                "`on_error=` is not compiled yet — a failing standard-library call is "
                "caught with `try` / `except` around the call itself",
            )
        if isinstance(work, ast.Lambda):
            if work.args.args:
                raise Untranslatable(work, "task's work takes no arguments")
            wbody, value = [], work.body
        elif isinstance(work, ast.Name) and work.id in self.defs:
            d = self.defs[work.id]
            if d.args.args:
                raise Untranslatable(work, f"`{work.id}` takes arguments — task's work takes none")
            if d.body and isinstance(d.body[-1], ast.Return) and d.body[-1].value is not None:
                wbody, value = d.body[:-1], d.body[-1].value
            else:
                wbody, value = d.body, None
        else:
            raise Untranslatable(work, "task's work is a module-level def or a lambda that takes no arguments")

        prev_async, prev_hit = self.in_async, self.async_hit
        self.in_async = True
        try:
            lines = []
            for st in wbody:
                lines += self._stmt(st, None)
            if value is not None:
                tmp = f"__t{len(self.handler_locals)}"
                assign = at(ast.Assign(targets=[ast.Name(id=tmp, ctx=ast.Store())], value=value), call)
                ast.fix_missing_locations(assign)
                lines += self._stmt(assign, None)
                if on_done is not None:
                    lines += self._done_lines(on_done, tmp, param)
            elif on_done is not None:
                raise Untranslatable(on_done, "on_done takes the work's value, and this work returns none")
        finally:
            self.in_async = prev_async
        self.async_hit = True
        return lines

    def _done_lines(self, on_done, tmp: str, param):
        """`on_done`'s body, with its parameter reading the local the
        work's value landed in."""
        if isinstance(on_done, ast.Lambda):
            if len(on_done.args.args) != 1:
                raise Untranslatable(on_done, "on_done takes exactly one argument, the work's value")
            pname = on_done.args.args[0].arg
            body = [at(ast.Expr(on_done.body), on_done.body)]
        elif isinstance(on_done, ast.Name) and on_done.id in self.defs:
            d = self.defs[on_done.id]
            if len(d.args.args) != 1:
                raise Untranslatable(on_done, f"`{on_done.id}` takes exactly one argument, the work's value")
            pname = d.args.args[0].arg
            body = d.body
        else:
            raise Untranslatable(on_done, "on_done is a one-argument lambda or a module-level def")
        body = [_CompArgs({pname: ast.Name(id=tmp, ctx=ast.Load())}).visit(copy.deepcopy(st)) for st in body]
        out = []
        for st in body:
            out += self._stmt(st, None)
        return out

    EXC_BROAD = ("RuntimeError", "Exception", "BaseException")

    def _struct_of(self, node):
        """The value-class name an expression carries, when the
        declarations alone can answer it: a typed local, a struct
        cell, or a field chain rooted at either."""
        if isinstance(node, ast.Name):
            t = self.typed_locals.get(node.id)
            return t if t in self.structs else None
        c = self._cell_read(node)
        if c is not None and self._ty(c) in self.structs:
            return self._ty(c)
        if isinstance(node, ast.Attribute):
            s = self._struct_of(node.value)
            if s is not None:
                for f, t, _ in self.structs[s]:
                    if f == node.attr:
                        return t if t in self.structs else None
            # A value class held by a store or a model, read through it.
            if isinstance(node.value, ast.Name):
                base = node.value.id
                if base in self.stores:
                    t = self.stores[base]["field_tys"].get(node.attr)
                    return t if t in self.structs else None
                mn = self.model_instances.get(base) or (
                    self.typed_locals.get(base) if self.typed_locals.get(base) in self.models else None
                )
                if mn is not None:
                    for f, t, _ in self.models[mn]["fields"]:
                        if f == node.attr:
                            return t if t in self.structs else None
            if isinstance(node.value, ast.Attribute) and node.value.attr == "self" :
                return None
        return None

    def _num_ty(self, node, ctx, param):
        """Best-effort scalar type of an expression: "Int" / "Float" /
        "Bool" / "String" / an enum name / None (unknown). Serves the
        operator rewrites, the text-hole renderers and the if/else
        local hoist."""
        if isinstance(node, ast.Constant):
            if type(node.value) is bool:
                return "Bool"
            if type(node.value) is int:
                return "Int"
            if type(node.value) is float:
                return "Float"
            if type(node.value) is str:
                return "String"
            return None
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.USub):
            return self._num_ty(node.operand, ctx, param)
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.Not):
            return "Bool" if self._num_ty(node.operand, ctx, param) == "Bool" else None
        if isinstance(node, ast.BoolOp) and isinstance(node.op, (ast.And, ast.Or)):
            if all(self._num_ty(v, ctx, param) == "Bool" for v in node.values):
                return "Bool"
            return None
        if isinstance(node, ast.Compare):
            return "Bool"
        if isinstance(node, ast.IfExp):
            return self._num_ty(node.body, ctx, param) or self._num_ty(node.orelse, ctx, param)
        if isinstance(node, ast.Attribute) and node.attr in ("name", "value"):
            base = node.value
            en = None
            if (
                isinstance(base, ast.Attribute)
                and isinstance(base.value, ast.Name)
                and base.value.id in self.enums
                and base.attr in self.enums[base.value.id]
            ):
                en = base.value.id
            else:
                t0 = self._num_ty(base, ctx, param)
                en = t0 if t0 in self.enums else None
            if en is not None:
                return "String" if node.attr == "name" else self._enum_value_ty(en)
        if isinstance(node, ast.NamedExpr):
            return self._num_ty(node.value, ctx, param)
        if isinstance(node, ast.BinOp):
            if isinstance(node.op, ast.Div):
                return "Float"
            if isinstance(node.op, (ast.Add, ast.Sub, ast.Mult, ast.FloorDiv, ast.Mod, ast.Pow)):
                lt = self._num_ty(node.left, ctx, param)
                rt = self._num_ty(node.right, ctx, param)
                if lt is not None and lt in self.structs and isinstance(node.op, (ast.Add, ast.Sub, ast.Mult)):
                    dunder = {ast.Add: "__add__", ast.Sub: "__sub__", ast.Mult: "__mul__"}[type(node.op)]
                    entry = self.struct_methods.get(lt, {}).get(dunder)
                    return entry[2] if entry else None
                if lt == "String" and rt == "String" and isinstance(node.op, ast.Add):
                    return "String"
                if "Float" in (lt, rt):
                    return "Float"
                if lt == "Int" and rt == "Int":
                    return "Int"
                return None
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute):
            smt = self._str_method_ty(node, ctx, param)
            if smt is not None:
                return smt if smt in ("Int", "Float", "Bool", "String") else None
            if (
                node.func.attr == "join"
                and len(node.args) == 1
                and self._num_ty(node.func.value, ctx, param) == "String"
            ):
                return "String"
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in ("str", "int", "float", "bool", "round")
            and len(node.args) == 1
            and node.func.id not in self.defs
        ):
            return {"str": "String", "int": "Int", "float": "Float", "bool": "Bool", "round": "Int"}[node.func.id]
        if (
            isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id in ("min", "max", "sum", "abs")
            and node.func.id not in self.defs
        ):
            if len(node.args) == 2:
                return self._num_ty(node.args[0], ctx, param) or self._num_ty(node.args[1], ctx, param)
            if len(node.args) == 1:
                if node.func.id == "abs":
                    return self._num_ty(node.args[0], ctx, param)
                lst = self._list_of(node.args[0], ctx, param)
                return lst[1] if lst and lst[1] in ("Int", "Float") else None
        if isinstance(node, ast.Call):
            if (
                isinstance(node.func, ast.Attribute)
                and isinstance(node.func.value, ast.Name)
            ):
                r = self.STDLIB_RET.get((node.func.value.id, node.func.attr))
                if r is not None:
                    return r
                base = node.func.value.id
                if base in self.stores:
                    st = self.stores[base]
                    if node.func.attr in st["statics"]:
                        return self.helpers[st["statics"][node.func.attr]][1]
                    if node.func.attr in st["ret_tys"]:
                        return st["ret_tys"][node.func.attr]
                mn = self.model_instances.get(base)
                if mn is None and self.typed_locals.get(base) in self.models:
                    mn = self.typed_locals[base]
                if mn is not None and node.func.attr in self.models[mn]["ret_tys"]:
                    return self.models[mn]["ret_tys"][node.func.attr]
            cc = self._crate_call(node.func)
            if cc is not None:
                entry = self.crate_info.get(cc[0], {}).get("fns", {}).get(cc[1])
                if entry and not isinstance(entry, str) and not entry[4]:
                    ret = entry[1]
                    if ret in self.crate_info.get(cc[0], {}).get("structs", {}):
                        return ret
                    if ret in self.crate_info.get(cc[0], {}).get("enums", {}):
                        return ret
                    if ret.startswith("Map<"):
                        return ret
                    return {"Int": "Int", "Float": "Float", "Bool": "Bool", "String": "String"}.get(ret)
                return None
            if isinstance(node.func, ast.Name):
                if node.func.id == "len":
                    return "Int"
                if node.func.id in self.structs and node.func.id not in self.union_of:
                    return node.func.id
                if node.func.id in self.models:
                    return node.func.id
                if node.func.id in self.escapes:
                    t = self.escapes[node.func.id]["ret"][0]
                    return t if t in ("Int", "Float", "Bool", "String") or t.endswith("?") else None
                d = self.defs.get(node.func.id)
                if d is not None and d.returns is not None:
                    t = self._param_ty(d.returns)
                    return t if t in ("Int", "Float", "Bool", "String") or t in self.structs or t in self.enums else None
            c = self._cell_read(node)
            if c is not None:
                t = self._ty(c)
                if t in ("Int", "Float", "Bool", "String") or t in self.enums or t in self.structs:
                    return t
                return None
        if isinstance(node, ast.Subscript) and self._num_ty(node.value, ctx, param) == "String":
            return "String"
        if isinstance(node, ast.Subscript) and not isinstance(node.slice, ast.Slice):
            lit = self._literal_element(node)
            if lit is not None:
                return self._num_ty(lit, ctx, param)
            src = self._list_source(node.value, ctx, param)
            if src is not None and src[1].startswith("List<"):
                el = src[1][5:-1]
                return el if el in ("Int", "Float", "Bool", "String") or el in self.enums or el in self.structs else None
        if isinstance(node, ast.Name) and self.row is not None and node.id == self.row[1]:
            return "Int"
        if isinstance(node, ast.Name):
            t = self.typed_locals.get(node.id)
            if t is None and self.comp_params is not None:
                t = self.comp_params.get(node.id)
            if t is None and self.comp_locals:
                t = self.comp_locals.get(node.id)
            if t is None and self.helper_params is not None:
                t = self.helper_params.get(node.id)
            if t in ("Int", "Float", "Bool", "String") or (
                t is not None and (t in self.enums or t in self.structs or t in self.models)
            ):
                return t
            if node.id in self.nonneg_loop_vars:
                return "Int"
            return None
        if (
            isinstance(node, ast.Attribute)
            and isinstance(node.value, ast.Name)
            and node.value.id in self.stores
            and node.attr in self.stores[node.value.id]["props"]
        ):
            if (node.value.id, node.attr) in self.prop_stack:
                return None
            self.prop_stack.append((node.value.id, node.attr))
            try:
                return self._num_ty(self.stores[node.value.id]["props"][node.attr], ctx, param)
            finally:
                self.prop_stack.pop()
        if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
            base = node.value.id
            mn = self.typed_locals.get(base)
            if mn is None and base in self.view_bindings:
                mn = self.view_bindings[base]
            if mn is None and base in self.model_instances:
                mn = self.model_instances[base]
            if mn is not None and mn in self.models:
                for f, t, _ in self.models[mn]["fields"]:
                    if f == node.attr:
                        return t if t in ("Int", "Float", "Bool", "String") else None
                return None
        if isinstance(node, ast.Attribute):
            s9 = self._struct_of(node.value)
            if s9 is not None:
                for f, t, _ in self.structs[s9]:
                    if f == node.attr and (
                        t in ("Int", "Float", "Bool", "String") or t in self.structs
                    ):
                        return t
                return None
        if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
            t = None
            if node.value.id == "self" and self.model_scope is not None:
                t = self.model_scope[1].get(node.attr)
            elif node.value.id in self.stores:
                t = self.stores[node.value.id]["field_tys"].get(node.attr)
            if t in ("Int", "Float", "Bool", "String") or (t is not None and (t in self.enums or t in self.structs)):
                return t
            return None
        return None

    def _hole(self, node, ctx, param):
        """A no-spec f-string hole. Bare float/bool/enum holes render
        through the Py-semantics helpers, so both tiers print exactly
        CPython's text; everything else lowers as before."""
        self._reject_map_text(node)
        t = self._num_ty(node, ctx, param)
        prev = self.text_hole
        self.text_hole = True
        try:
            if t == "Float":
                self.uses_stdlib = True
                self.in_wrapped_hole = True
                try:
                    return "Py.floatRepr(" + self.expr(node, ctx, param) + ")"
                finally:
                    self.in_wrapped_hole = False
            if t == "Bool":
                self.uses_stdlib = True
                self.in_wrapped_hole = True
                try:
                    return "Py.boolRepr(" + self.expr(node, ctx, param) + ")"
                finally:
                    self.in_wrapped_hole = False
            if t is not None and t in self.enums:
                self.enum_text_used.add(t)
                return f"PyText.enumStr{t}(" + self.expr(node, ctx, param) + ")"
            self._reject_float_text(node)
            self._reject_bool_text(node)
            self._reject_opt_enum_text(node)
            return self.expr(node, ctx, param)
        finally:
            self.text_hole = prev

    def _param_ty(self, ann):
        """A store/model method parameter's pixie type: the scalar
        four, list[...] of them, a value class, an enum, or a union."""
        if isinstance(ann, ast.Name):
            t = self.HELPER_TY.get(ann.id)
            if t:
                return t
            if ann.id in self.structs or ann.id in self.enums or ann.id in self.unions:
                return ann.id
            return None
        return self._pix_ty(ann)

    ZERO_OF = {"Int": "0", "Float": "0.0", "String": '""', "Bool": "false"}

    def _zero_of(self, ty: str) -> str:
        if ty in self.ZERO_OF:
            return self.ZERO_OF[ty]
        if ty.startswith("List<"):
            return "[]"
        if ty.startswith("Map<"):
            return "{}"
        if ty.endswith("?"):
            return "nil"
        raise Untranslatable(None, f"a local of type `{ty}` has no default to start from")

    def _lower_try(self, stmt, param) -> list[str]:
        """The full form: any statements in the body, multiple except
        clauses (typed, tuples, `as e`), else, finally. Escape-call
        failures dispatch through a synthesized Python function whose
        clauses are LITERALLY these (CPython's own matching, both
        tiers); stdlib failures resolve statically (they raise
        exactly RuntimeError); anything no clause catches keeps the
        plain contained path."""
        prev_try, self.in_try = self.in_try, True
        try:
            return self._lower_try_inner(stmt, param)
        finally:
            self.in_try = prev_try

    def _lower_try_inner(self, stmt, param) -> list[str]:
        clauses = []
        for h in stmt.handlers:
            if h.type is None:
                t = None
            elif isinstance(h.type, ast.Name):
                t = [h.type.id]
            elif isinstance(h.type, ast.Tuple) and all(isinstance(e, ast.Name) for e in h.type.elts):
                t = [e.id for e in h.type.elts]
            else:
                raise Untranslatable(h, "except takes exception names (`except ValueError:`, `except (ValueError, KeyError) as e:`)")
            clauses.append((t, h.name, h.body))
        catchall = any(
            t is None or any(n in ("Exception", "BaseException") for n in t)
            for t, _, _ in clauses
        )
        if stmt.finalbody and not catchall:
            raise Untranslatable(
                stmt,
                "`finally` needs a catch-all `except Exception` clause — an uncaught failure aborts the statement in the compiled run before `finally` would run, where Python runs it",
            )
        seq = self.try_seq
        self.try_seq += 1
        ok = f"__ok{seq}"
        self.handler_locals.add(ok)
        lines = [f"var {ok} = true"]
        lines += self._try_seg(stmt.body, clauses, ok, param)
        if stmt.orelse:
            lines.append(f"if {ok} {{")
            lines += self._block(stmt.orelse, param)
            lines.append("}")
        for st in stmt.finalbody:
            lines += self._stmt(st, param)
        return lines

    def _fallible_in(self, st):
        """(call, kind, target) when this statement's value is one
        catchable call — escape or fallible stdlib call — else None."""
        def catchable(c):
            if not isinstance(c, ast.Call):
                return False
            if isinstance(c.func, ast.Name) and c.func.id in self.escapes:
                return True
            cc = self._crate_call(c.func)
            if cc is not None:
                e = self.crate_info.get(cc[0], {}).get("fns", {}).get(cc[1])
                return bool(e) and not isinstance(e, str) and e[4]
            mf = self._stdlib_call(c.func)
            return mf is not None and mf in self.FALLIBLE
        if isinstance(st, ast.Expr) and catchable(st.value):
            return (st.value, "bare", None)
        if (
            isinstance(st, ast.Expr)
            and isinstance(st.value, ast.Call)
            and isinstance(st.value.func, ast.Attribute)
            and st.value.func.attr == "set"
            and isinstance(st.value.func.value, ast.Name)
            and st.value.func.value.id in self.cells
            and len(st.value.args) == 1
            and catchable(st.value.args[0])
        ):
            return (st.value.args[0], "cell", st.value.func.value.id)
        if (
            isinstance(st, ast.Assign)
            and len(st.targets) == 1
            and isinstance(st.targets[0], ast.Attribute)
            and isinstance(st.targets[0].value, ast.Name)
            and st.targets[0].value.id == "self"
            and self.model_scope is not None
            and catchable(st.value)
        ):
            return (st.value, "selffield", st.targets[0].attr)
        if (
            isinstance(st, ast.Assign)
            and len(st.targets) == 1
            and isinstance(st.targets[0], ast.Name)
            and catchable(st.value)
        ):
            return (st.value, "local", st.targets[0].id)
        return None

    def _try_seg(self, stmts, clauses, ok, param) -> list[str]:
        out = []
        for i, st in enumerate(stmts):
            f = self._fallible_in(st)
            if f is None:
                out += self._stmt(st, param)
                continue
            call, kind, target = f
            out += self._lower_fallible(call, kind, target, clauses, ok, param)
            rest = stmts[i + 1 :]
            if rest:
                out.append(f"if {ok} {{")
                out += ["  " + l for l in self._try_seg(rest, clauses, ok, param)]
                out.append("}")
            return out
        return out

    def _clause_body(self, bindname, body, msg_expr, ok, param) -> list[str]:
        lines = [f"{ok} = false"]
        if bindname:
            self.handler_locals.add(bindname)
            prev_ty = self.typed_locals.get(bindname)
            self.typed_locals[bindname] = "String"
            if msg_expr is not None:
                lines.append(f"var {bindname} = {msg_expr}")
        for st in body:
            lines += self._stmt(st, param)
        if bindname:
            self.handler_locals.discard(bindname)
            if prev_ty is None:
                self.typed_locals.pop(bindname, None)
            else:
                self.typed_locals[bindname] = prev_ty
            self.dead_locals[bindname] = f"`{bindname}` is bound inside its except clause only"
        return lines

    def _lower_fallible(self, call, kind, target, clauses, ok, param) -> list[str]:
        is_escape = isinstance(call.func, ast.Name) and call.func.id in self.escapes
        cc = self._crate_call(call.func)
        if cc is not None:
            crate, fn = cc
            info = self.crate_info[crate]
            params, ret, rpi_name, _rp, _fal = info["fns"][fn]
            if len(call.args) != len(params) or call.keywords:
                raise Untranslatable(call, f"{crate}.{fn} takes {len(params)} positional argument(s)")
            match_ix = None
            for i, (t, _b, _body) in enumerate(clauses):
                if t is None or any(n in self.EXC_BROAD for n in t):
                    match_ix = i
                    break
            if match_ix is None:
                raise Untranslatable(
                    call,
                    f"{crate}.{fn} returns Result — this try needs an "
                    f"`except Exception` clause to receive the failure",
                )
            self._check_crate_structs(call, crate, info["fns"][fn])
            self.uses_crates.add(crate)
            args = ", ".join(
                "nil" if isinstance(a, ast.Constant) and a.value is None
                else self.expr(a, "store", param)
                for a in call.args
            )
            t, bindname, body = clauses[match_ix]
            pre = []
            okline = None
            if kind == "local":
                try:
                    zero = self._zero_of(ret)
                except Untranslatable:
                    raise Untranslatable(
                        call,
                        f"{crate}.{fn} returns `{py_ty(ret)}`, which has no pre-binding "
                        f"default — assign the result to a store field instead of a local",
                    )
                pre = [f"var {target} = {zero}"]
                self.handler_locals.add(target)
                self.typed_locals[target] = ret
                okline = f"{target} = __v"
            elif kind in ("cell", "selffield"):
                okline = f"{target} = __v"
            binder = bindname or "__e"
            lines = pre + [f"case {info['class']}.{rpi_name}({args}) {{"]
            if okline:
                lines += ["  when ok(__v) {", f"    {okline}", "  }"]
            else:
                lines += ["  when ok { }"]
            self.handler_locals.add(binder)
            prev_ty = self.typed_locals.get(binder)
            self.typed_locals[binder] = "String"
            lines.append(f"  when err({binder}) {{")
            for l in self._clause_body(None, body, None, ok, param):
                lines.append("    " + l)
            lines.append("  }")
            self.handler_locals.discard(binder)
            if prev_ty is None:
                self.typed_locals.pop(binder, None)
            else:
                self.typed_locals[binder] = prev_ty
            lines.append("}")
            return lines
        if not is_escape:
            # A stdlib call: it raises exactly RuntimeError, so the
            # matching clause is known STATICALLY.
            match_ix = None
            for i, (t, _b, _body) in enumerate(clauses):
                if t is None or any(n in self.EXC_BROAD for n in t):
                    match_ix = i
                    break
            mod_fn = self._stdlib_call(call.func)
            spec = self.STDLIB_CALLS[mod_fn]
            if len(call.args) != spec[2] or call.keywords:
                raise Untranslatable(call, f"`{mod_fn[0]}.{mod_fn[1]}` takes {spec[2]} argument(s)")
            args = ", ".join(self.expr(a, "store", param) for a in call.args)
            self.uses_stdlib = True
            if match_ix is None:
                # No clause catches it: the plain contained path, in
                # both tiers.
                trypath = None
            else:
                trypath = self.FALLIBLE[mod_fn]
            if trypath is None:
                plain = f"{spec[0]}.{spec[1]}({args})"
                if kind == "cell":
                    return [f"{target} = {plain}"]
                if kind == "selffield":
                    return [f"{target} = {plain}"]
                if kind == "local":
                    self.handler_locals.add(target)
                    return [f"var {target} = {plain}"]
                name = f"__fs{len(self.handler_locals)}"
                self.handler_locals.add(name)
                return [f"var {name} = {plain}"]
            t, bindname, body = clauses[match_ix]
            pre = []
            okline = None
            if kind == "local":
                ret_pix = {"getText": "String", "readText": "String", "queryInt": "Int"}.get(spec[1], "String")
                pre = [f"var {target} = {self._zero_of(ret_pix)}"]
                self.handler_locals.add(target)
                self.typed_locals[target] = ret_pix
                okline = f"{target} = __v"
            elif kind in ("cell", "selffield"):
                okline = f"{target} = __v"
            binder = bindname or "__e"
            lines = pre + [f"case {trypath}({args}) {{"]
            if okline:
                lines += ["  when ok(__v) {", f"    {okline}", "  }"]
            else:
                lines += ["  when ok { }"]
            self.handler_locals.add(binder)
            prev_ty = self.typed_locals.get(binder)
            self.typed_locals[binder] = "String"
            lines.append(f"  when err({binder}) {{")
            for l in self._clause_body(None, body, None, ok, param):
                lines.append("    " + l)
            lines.append("  }")
            self.handler_locals.discard(binder)
            if prev_ty is None:
                self.typed_locals.pop(binder, None)
            else:
                self.typed_locals[binder] = prev_ty
            self.dead_locals[binder] = f"`{binder}` is bound inside its except clause only"
            lines.append("}")
            return lines
        # An escape: synthesize a dispatcher whose clauses are exactly
        # these — CPython does the matching in both tiers.
        ename = call.func.id
        esc = self.escapes[ename]
        if len(call.args) != len(esc["params"]) or call.keywords:
            raise Untranslatable(call, f"`{ename}` takes {len(esc['params'])} positional argument(s)")
        k = len(self.try_wrappers)
        self.try_wrappers.append({
            "k": k,
            "escape": ename,
            "clauses": [t for t, _b, _body in clauses],
        })
        args = ", ".join(self.expr(a, "store", param) for a in call.args)
        t_var = f"__t{k}"
        self.handler_locals.add(t_var)
        pre = []
        okline = None
        if kind == "local":
            pre = [f"var {target} = {self._zero_of(esc['ret'][0])}"]
            self.handler_locals.add(target)
            self.typed_locals[target] = esc["ret"][0]
            okline = f"{target} = {t_var}.value"
        elif kind in ("cell", "selffield"):
            okline = f"{target} = {t_var}.value"
        lines = pre + [f"var {t_var} = Escapes.try{k}({args})"]
        lines.append(f"if {t_var}.tag == 0 {{")
        if okline:
            lines.append(f"  {okline}")
        lines.append("} else {")
        def arm(i: int) -> list[str]:
            if i >= len(clauses):
                return []
            _t, bindname, body = clauses[i]
            b = self._clause_body(bindname, body, f"{t_var}.msg" if bindname else None, ok, param)
            rest = arm(i + 1)
            if rest:
                return [f"if {t_var}.tag == {i + 1} {{", *[f"  {l}" for l in b], "} else {", *[f"  {l}" for l in rest], "}"]
            return [f"if {t_var}.tag == {i + 1} {{", *[f"  {l}" for l in b], "}"]
        lines += ["  " + l for l in arm(0)]
        lines.append("}")
        return lines

    def _block(self, stmts, param) -> list[str]:
        """A nested handler block. Locals born inside it are
        BLOCK-scoped natively (pixie `let`), while Python leaks them
        to the function — so reads after the block are refused with
        the reason, instead of diverging or dying in rustc."""
        before = set(self.handler_locals)
        out = []
        for st in stmts:
            for line in self._stmt(st, param):
                out.append("  " + line)
        for name in self.handler_locals - before:
            self.handler_locals.discard(name)
            self.dead_locals[name] = (
                f"`{name}` was assigned inside a branch or loop only — had that path "
                "not run, Python would raise NameError here; assign it before the "
                "block, or in both branches"
            )
        return out

    def _pair_for(self, stmt: ast.For, param) -> list[str]:
        """`for i, x in enumerate(xs)` and `for a, b in zip(xs, ys)` —
        the index loop each one stands for. enumerate rides the
        repeater's own index; zip walks to the shorter list, as Python
        does."""
        names = stmt.target.elts
        if len(names) != 2 or not all(isinstance(n, ast.Name) for n in names):
            raise Untranslatable(stmt.target, "a pair target binds two plain names (`for i, x in enumerate(xs)`)")
        a, b = names[0].id, names[1].id
        it = stmt.iter
        if not (isinstance(it, ast.Call) and isinstance(it.func, ast.Name) and it.func.id in ("enumerate", "zip")):
            raise Untranslatable(it, "a pair target walks enumerate() or zip()")
        if it.func.id == "enumerate":
            if len(it.args) != 1:
                raise Untranslatable(it, "enumerate() takes the list — a start value is not in the dialect yet")
            src = self._list_of(it.args[0], "store", param)
            if src is None:
                raise Untranslatable(it.args[0], "enumerate() walks a list the dialect can name — a state read, a field or a local")
            code, el, _suf = src
            lines = [f"for {b}, {a} in {code} {{"]
            binds = [(a, "Int"), (b, el)]
        else:
            if len(it.args) != 2:
                raise Untranslatable(it, "zip() takes two lists")
            left = self._list_of(it.args[0], "store", param)
            right = self._list_of(it.args[1], "store", param)
            if left is None or right is None:
                raise Untranslatable(it, "zip() walks two lists the dialect can name")
            n = f"__zn{len(self.handler_locals)}"
            i = f"__zi{len(self.handler_locals)}"
            self.handler_locals.add(n)
            lines = [
                f"var {n} = {left[0]}.length",
                f"if {right[0]}.length < {n} {{",
                f"  {n} = {right[0]}.length",
                "}",
                f"for {i} in 0..{n} {{",
                f"  var {a} = {left[0]}[{i}]",
                f"  var {b} = {right[0]}[{i}]",
            ]
            binds = [(a, left[1]), (b, right[1])]
        prev = {nm: self.typed_locals.get(nm) for nm, _t in binds}
        for nm, t in binds:
            self.handler_locals.add(nm)
            self.typed_locals[nm] = t
        try:
            body = self._block(stmt.body, param)
        finally:
            for nm, _t in binds:
                self.handler_locals.discard(nm)
                if prev[nm] is None:
                    self.typed_locals.pop(nm, None)
                else:
                    self.typed_locals[nm] = prev[nm]
                self.dead_locals[nm] = f"`{nm}` is bound inside the loop only"
        return lines + body + ["}"]

    def _stepped_for(self, stmt: ast.For, it: ast.Call, param) -> list[str]:
        """`for i in range(a, b, step)` — pixie counts by one, so the
        loop counts turns and the variable is the value each turn
        stands for."""
        var = stmt.target.id
        lo = self.expr(it.args[0], "store", param)
        hi = self.expr(it.args[1], "store", param)
        step_node = it.args[2]
        neg = isinstance(step_node, ast.UnaryOp) and isinstance(step_node.op, ast.USub)
        c = step_node.operand if neg else step_node
        if not (isinstance(c, ast.Constant) and type(c.value) is int and c.value != 0):
            raise Untranslatable(step_node, "range()'s step is a non-zero int literal")
        step = -c.value if neg else c.value
        k = len(self.handler_locals)
        n, t = f"__rn{k}", f"__rt{k}"
        self.handler_locals.update({n, t})
        span = f"({hi}) - ({lo})" if step > 0 else f"({lo}) - ({hi})"
        mag = abs(step)
        lines = [
            f"var {n} = {span}",
            f"if {n} < 0 {{",
            f"  {n} = 0",
            "}",
            f"{n} = ({n} + {mag - 1}) / {mag}",
            f"for {t} in 0..{n} {{",
            f"  var {var} = ({lo}) + {t} * {step}",
        ]
        prev = self.typed_locals.get(var)
        self.handler_locals.add(var)
        self.typed_locals[var] = "Int"
        try:
            body = self._block(stmt.body, param)
        finally:
            self.handler_locals.discard(var)
            if prev is None:
                self.typed_locals.pop(var, None)
            else:
                self.typed_locals[var] = prev
            self.dead_locals[var] = f"`{var}` is bound inside the loop only"
        return lines + body + ["}"]

    def _handler_for(self, stmt, param) -> list[str]:
        if stmt.orelse:
            raise Untranslatable(stmt, "for-else is not in the dialect yet")
        if isinstance(stmt.target, ast.Tuple):
            return self._pair_for(stmt, param)
        if not isinstance(stmt.target, ast.Name):
            raise Untranslatable(stmt.target, "a for loop binds one plain name, or `k, v` over enumerate() / zip()")
        var = stmt.target.id
        if var in self.cells or var in self.handler_locals:
            raise Untranslatable(stmt.target, f"loop variable `{var}` shadows an existing name")
        it = stmt.iter
        self._loop_src_elem = None
        if (
            isinstance(it, ast.Call)
            and isinstance(it.func, ast.Name)
            and it.func.id == "range"
        ):
            if it.keywords or not 1 <= len(it.args) <= 3:
                raise Untranslatable(it, "range() takes one, two or three arguments")
            if len(it.args) == 3:
                return self._stepped_for(stmt, it, param)
            if len(it.args) == 1:
                lo, hi = "0", self.expr(it.args[0], "store", param)
                nonneg = True
            else:
                lo = self.expr(it.args[0], "store", param)
                hi = self.expr(it.args[1], "store", param)
                nonneg = isinstance(it.args[0], ast.Constant) and type(it.args[0].value) is int and it.args[0].value >= 0
            if nonneg:
                self.nonneg_loop_vars.add(var)
            head = f"for {var} in {lo}..{hi} {{"
        elif (
            isinstance(it, ast.Call)
            and isinstance(it.func, ast.Name)
            and it.func.id == "sorted"
            and len(it.args) == 1
            and not it.keywords
        ):
            # `for k in sorted(d())` — key order, which is exactly the
            # native map's own order, so the tiers agree. (Bare
            # iteration stays refused: Python walks insertion order.)
            inner = it.args[0]
            msrc = None
            c = self._cell_read(inner)
            if c is not None and self._ty(c).startswith("Map<"):
                msrc = c
            elif (
                isinstance(inner, ast.Attribute)
                and isinstance(inner.value, ast.Name)
                and inner.value.id == "self"
                and self.model_scope is not None
                and self.model_scope[1].get(inner.attr, "").startswith("Map<")
            ):
                msrc = inner.attr
            elif (
                isinstance(inner, ast.Attribute)
                and isinstance(inner.value, ast.Name)
                and inner.value.id in self.stores
                and self.stores[inner.value.id]["field_tys"].get(inner.attr, "").startswith("Map<")
            ):
                msrc = f"{inner.value.id}.{inner.attr}"
            if msrc is None:
                raise Untranslatable(it, "sorted() here iterates a dict state or field in key order — `sorted(list)` is not in the dialect yet")
            head = f"for {var} in {msrc}.keys() {{"
        else:
            c = self._cell_read(it)
            src = None
            self._loop_src_elem = None
            if c is not None and self._ty(c).startswith("List<"):
                src = c
                self._loop_src_elem = self._ty(c)[5:-1]
            elif (
                isinstance(it, ast.Name)
                and self.typed_locals.get(it.id, "").startswith("List<")
            ):
                # a list-typed local or method parameter
                src = it.id
                self._loop_src_elem = self.typed_locals[it.id][5:-1]
            elif isinstance(it, ast.Name) and it.id in self.enums:
                # `for m in Mood:` — the members are known here, so the
                # loop runs over them in declaration order, which is
                # the order Python iterates an Enum.
                src = "[" + ", ".join(f"{it.id}.{m}" for m in self.enums[it.id]) + "]"
                self._loop_src_elem = it.id
            elif (
                isinstance(it, ast.Attribute)
                and isinstance(it.value, ast.Name)
                and it.value.id == "self"
                and self.model_scope is not None
                and self.model_scope[1].get(it.attr, "").startswith("List<")
            ):
                src = it.attr
                self._loop_src_elem = self.model_scope[1][it.attr][5:-1]
            elif (
                isinstance(it, ast.Attribute)
                and isinstance(it.value, ast.Name)
                and it.value.id in self.stores
                and self.stores[it.value.id]["field_tys"].get(it.attr, "").startswith("List<")
            ):
                src = f"{it.value.id}.{it.attr}"
                self._loop_src_elem = self.stores[it.value.id]["field_tys"][it.attr][5:-1]
            if src is None and isinstance(it, ast.Call) and isinstance(it.func, ast.Name) and it.func.id in ("sorted", "reversed") and len(it.args) == 1:
                inner = self._list_of(it.args[0], "store", param)
                if inner is not None:
                    src = self._list_builtin(it, "store", param)
                    self._loop_src_elem = inner[1]
            if src is None:
                if isinstance(it, ast.Call) and isinstance(it.func, ast.Name) and it.func.id in ("reversed", "enumerate", "zip"):
                    raise Untranslatable(it, f"`{it.func.id}()` here takes a list the dialect can name — a state read, a field or a local")
                if isinstance(it, ast.Call) and isinstance(it.func, ast.Attribute) and it.func.attr in ("values", "items", "keys"):
                    raise Untranslatable(
                        it,
                        f"dict `.{it.func.attr}()` iterates in insertion order in Python and by key "
                        "in the compiled app, so it is refused rather than reordered — iterate "
                        "`sorted(d())` and read `d().get(k, default)`",
                    )
                if isinstance(it, ast.Name) and it.id in self.enums:
                    pass
                if isinstance(it, ast.Name) and it.id in self.handler_locals:
                    raise Untranslatable(it, "iterating a local list is not in the dialect yet — keep the list in a State or a store field")
                if c is not None and self._ty(c) == "String":
                    raise Untranslatable(it, "iterating a str is not in the dialect yet")
                raise Untranslatable(it, "for iterates range(...), a list state, a list field, a list parameter, or `sorted(d())` — a local list, `enumerate`, `zip` and a str are not in the dialect yet")
            head = f"for {var} in {src} {{"
        self.handler_locals.add(var)
        # A loop variable is Int by construction (range) or the list's
        # element type — Int/String render in text holes.
        prev_ty = self.typed_locals.get(var)
        if head.startswith(f"for {var} in ") and ".." in head:
            self.typed_locals[var] = "Int"
        elif head.endswith(".keys() {"):
            self.typed_locals[var] = "String"
        else:
            ety = self._loop_src_elem
            if ety is not None and (ety in self.models or ety in self.structs or ety in self.enums or ety in ("Int", "Float", "Bool", "String")):
                self.typed_locals[var] = ety
        body = self._block(stmt.body, param)
        self.handler_locals.discard(var)
        self.nonneg_loop_vars.discard(var)
        if prev_ty is None:
            self.typed_locals.pop(var, None)
        else:
            self.typed_locals[var] = prev_ty
        self.dead_locals[var] = (
            f"loop variable `{var}` is not readable after its loop — Python leaves "
            "it bound, the compiled app does not; copy it into a local declared "
            "before the loop"
        )
        return [head, *body, "}"]

    # ---- elements --------------------------------------------------

    def element(self, node, indent: int) -> list[str]:
        pad = "  " * indent
        if isinstance(node, ast.IfExp):
            lines = [f"{pad}if {self._cond(node.test)} {{"]
            lines += self.element(node.body, indent + 1)
            lines.append(f"{pad}}} else {{")
            lines += self.element(node.orelse, indent + 1)
            lines.append(f"{pad}}}")
            return lines
        if not isinstance(node, ast.Call):
            raise Untranslatable(node, "a view's children are element calls")
        kw = {k.arg: k.value for k in node.keywords if k.arg}
        style_rider = None
        for k in node.keywords:
            if k.arg is not None:
                continue
            if not (isinstance(k.value, ast.Name) and k.value.id in self.styles):
                raise Untranslatable(k.value, "`**` on an element takes a style (`**chip` where `chip = style(...)`)")
            if style_rider is not None:
                raise Untranslatable(k.value, "one style per element — compose with `|` first")
            style_rider = self.styles[k.value.id][0]
        theme_rider = None
        if "theme" in kw:
            tv = kw.pop("theme")
            if isinstance(tv, ast.Constant) and type(tv.value) is str:
                theme_rider = f'"{esc(tv.value)}"'
            elif (tc := self._cell_read(tv)) is not None and self._ty(tc) == "String":
                theme_rider = f"App.{tc}"
            else:
                raise Untranslatable(tv, 'theme= takes "light", "dark" or a str state read')
        num = lambda name: self._num(kw, name)  # noqa: E731
        strlit = lambda name: self._strlit(kw, name)  # noqa: E731

        if self._is_ui(node.func, "column") or self._is_ui(node.func, "row"):
            tag = "Column" if self._is_ui(node.func, "column") else "Row"
            lines = [f"{pad}{tag} {{"]
            if style_rider:
                lines.append(f"{pad}  style: {style_rider}")
            if theme_rider:
                lines.append(f"{pad}  theme: {theme_rider}")
            lines += self._container_props(kw, pad)
            for child in node.args:
                lines += self.element(child, indent + 1)
            lines.append(f"{pad}}}")
            return lines

        if self._is_ui(node.func, "text"):
            if not node.args:
                raise Untranslatable(node, "text() needs its string")
            props = []
            if style_rider:
                props.append(f"style: {style_rider}")
            props += self._anim_props(kw)
            props.append(f"text: {self._text_value(node.args[0])}")
            size = num("size")
            if size is not None:
                props.append(f"fontSize: {size}")
            color = strlit("color")
            if color is not None:
                props.append(f"color: {pixstr(color)}")
            align = strlit("align")
            if align is not None:
                props.append(f"align: {pixstr(align)}")
            grow = num("grow")
            if grow is not None:
                props.append(f"grow: {grow}")
            for k in kw:
                if k not in ("size", "color", "align", "grow"):
                    raise Untranslatable(kw[k], f"text() does not take `{k}=`")
            return [f"{pad}Text {{ {'; '.join(props)} }}"]

        if self._is_ui(node.func, "button"):
            if not node.args:
                raise Untranslatable(node, "button() needs its label")
            label = self._text_value(node.args[0])
            h = self.handler(kw["on_click"], takes_text=False) if "on_click" in kw else None
            sty = [f"style: {style_rider}"] if style_rider else []
            sty += self._anim_props(kw)
            for pykey, prop in (("col_span", "colSpan"), ("row_span", "rowSpan")):
                sp = self._num(kw, pykey)
                if sp is not None:
                    sty.append(f"{prop}: {sp}")
            if isinstance(h, tuple):
                lines = [f"{pad}Button {{"] + [f"{pad}  {p}" for p in sty]
                lines += [f"{pad}  text: {label}", f"{pad}  onClick: {{"]
                lines += [f"{pad}    {ln}" for ln in h[1]]
                lines += [f"{pad}  }}", f"{pad}}}"]
                return lines
            props = sty + [f"text: {label}"] + ([f"onClick: {h}"] if h else [])
            return [f"{pad}Button {{ {'; '.join(props)} }}"]

        if self._is_ui(node.func, "text_field"):
            if not node.args:
                raise Untranslatable(node, "text_field() needs its value")
            v = node.args[0]
            if isinstance(v, ast.Subscript):
                ref = self._field(v, "view")
            else:
                cell = self._cell_read(v)
                if cell is not None:
                    ref = f"App.{cell}"
                elif (
                    isinstance(v, ast.Attribute)
                    and isinstance(v.value, ast.Name)
                    and v.value.id in self.stores
                    and self.stores[v.value.id]["field_tys"].get(v.attr) == "String"
                ):
                    ref = f"{v.value.id}.{v.attr}"
                else:
                    raise Untranslatable(v, "text_field's value is a str state or store-field read (a model field is not accepted here yet)")
            lines = [f"{pad}TextField {{", f"{pad}  text: {ref}"]
            ph = strlit("placeholder")
            if ph is not None:
                lines.append(f"{pad}  placeholder: {pixstr(ph)}")
            for pykey, prop in (("on_change", "onTextChanged"), ("on_submit", "onSubmitted")):
                if pykey not in kw:
                    continue
                h = self.handler(kw[pykey], takes_text=True)
                if isinstance(h, tuple):
                    lines.append(f"{pad}  {prop}: {{")
                    lines += [f"{pad}    {ln}" for ln in h[1]]
                    lines.append(f"{pad}  }}")
                else:
                    lines.append(f"{pad}  {prop}: {h}")
            lines.append(f"{pad}}}")
            return lines

        if self._is_ui(node.func, "list_view"):
            if len(node.args) != 2:
                raise Untranslatable(node, "list_view takes the count and the row builder: `list_view(len(items()), row, ...)`")
            count_cell = None
            ca = node.args[0]
            if (
                isinstance(ca, ast.Call)
                and isinstance(ca.func, ast.Name)
                and ca.func.id == "len"
                and len(ca.args) == 1
            ):
                count_cell = self._cell_read(ca.args[0])
                a0 = ca.args[0]
                if (
                    count_cell is None
                    and isinstance(a0, ast.Attribute)
                    and isinstance(a0.value, ast.Name)
                    and a0.value.id in self.stores
                    and self.stores[a0.value.id]["field_tys"].get(a0.attr, "").startswith("List<")
                ):
                    count_cell = ("store", a0.value.id, a0.attr)
            if count_cell is None or (
                isinstance(count_cell, str)
                and not self.cells.get(count_cell, "").startswith("List<")
            ):
                raise Untranslatable(ca, "list_view's count is `len()` of a list state or store field")
            rf = node.args[1]
            if isinstance(rf, ast.Name) and rf.id in self.defs:
                d = self.defs[rf.id]
                if len(d.args.args) != 1:
                    raise Untranslatable(rf, "a row builder takes one argument, the row index")
                rparam = d.args.args[0].arg
                rbody = d.body
            elif isinstance(rf, ast.Lambda) and len(rf.args.args) == 1:
                rparam = rf.args.args[0].arg
                rbody = [ast.Return(rf.body)]
            else:
                raise Untranslatable(rf, "a row builder is a one-argument lambda or a module-level def")
            loopvar = "it" if "it" not in self.cells else "it_"
            lines = [f"{pad}ListView {{"]
            virt = kw.get("virtualized")
            virt_on = not (
                isinstance(virt, ast.Constant) and virt.value is False
            )
            if virt_on:
                lines.append(f"{pad}  virtualized: true")
            ih = self._num(kw, "item_height")
            lines.append(f"{pad}  itemHeight: {ih if ih is not None else 24}")
            h = self._num(kw, "height")
            if h is not None:
                lines.append(f"{pad}  height: {h}")
            g = self._num(kw, "grow")
            if g is not None:
                lines.append(f"{pad}  grow: {g}")
            iter_src = (
                f"{count_cell[1]}.{count_cell[2]}"
                if isinstance(count_cell, tuple)
                else f"App.{count_cell}"
            )
            ixvar = rparam if rparam not in self.cells and rparam != loopvar else f"{rparam}_"
            lines.append(f"{pad}  for {loopvar}, {ixvar} in {iter_src} {{")
            prev = self.row
            self.row = (count_cell, rparam, loopvar, ixvar)
            try:
                if len(rbody) == 1 and isinstance(rbody[0], ast.Return):
                    lines += self.element(rbody[0].value, indent + 2)
                elif len(rbody) == 1 and isinstance(rbody[0], ast.With):
                    lines += self.with_element(rbody[0], indent + 2)
                else:
                    raise Untranslatable(rf, "a row builder is a single `return text(...)` or one `with` block")
            finally:
                self.row = prev
            lines.append(f"{pad}  }}")
            lines.append(f"{pad}}}")
            return lines

        def typed_read(v, want, what):
            """A Bool/Int/Float/String prop read for a widget binding:
            a state read or a store-field read of that type."""
            c9 = self._cell_read(v)
            if c9 is not None and self._ty(c9) == want:
                return f"App.{c9}"
            if (
                isinstance(v, ast.Attribute)
                and isinstance(v.value, ast.Name)
                and v.value.id in self.stores
                and self.stores[v.value.id]["field_tys"].get(v.attr) == want
            ):
                return f"{v.value.id}.{v.attr}"
            raise Untranslatable(v, f"{what} is a {want.lower()} state or store-field read")

        def list_read(v, what):
            c9 = self._cell_read(v)
            if c9 is not None and self._ty(c9) == "List<String>":
                return f"App.{c9}"
            if (
                isinstance(v, ast.Attribute)
                and isinstance(v.value, ast.Name)
                and v.value.id in self.stores
                and self.stores[v.value.id]["field_tys"].get(v.attr) == "List<String>"
            ):
                return f"{v.value.id}.{v.attr}"
            if isinstance(v, ast.List) and all(
                isinstance(e, ast.Constant) and type(e.value) is str for e in v.elts
            ):
                return self._list_literal(v, "String")
            raise Untranslatable(v, f"{what} is a list of str literals, or a list[str] state or store-field read")

        for fname, tag in (("checkbox", "Checkbox"), ("switch", "Switch")):
            if self._is_ui(node.func, fname):
                if not node.args:
                    raise Untranslatable(node, f"{fname}() needs its label")
                props = [f"label: {self._text_value(node.args[0])}"]
                if "checked" not in kw:
                    raise Untranslatable(node, f"{fname}() needs checked=")
                cv = kw["checked"]
                if isinstance(cv, ast.Constant) and type(cv.value) is bool:
                    props.append(f"checked: {'true' if cv.value else 'false'}")
                else:
                    props.append(f"checked: {typed_read(cv, 'Bool', 'checked=')}")
                if "on_change" in kw:
                    h = self.handler(kw["on_change"], takes_text=True, implicit=("checked", "Bool"))
                    if isinstance(h, tuple):
                        lines = [f"{pad}{tag} {{"] + [f"{pad}  {p}" for p in props]
                        lines += [f"{pad}  onToggle: {{"] + [f"{pad}    {ln}" for ln in h[1]]
                        lines += [f"{pad}  }}", f"{pad}}}"]
                        return lines
                    props.append(f"onToggle: {h}")
                return [f"{pad}{tag} {{ {'; '.join(props)} }}"]

        if self._is_ui(node.func, "slider"):
            if "value" not in kw:
                raise Untranslatable(node, "slider() needs value=")
            props = [f"value: {typed_read(kw['value'], 'Float', 'value=')}"]
            for k2 in ("min", "max", "step"):
                n2 = self._num(kw, k2)
                if n2 is not None:
                    props.append(f"{k2}: {n2}")
            if "on_change" in kw:
                h = self.handler(kw["on_change"], takes_text=True, implicit=("value", "Float"))
                if isinstance(h, tuple):
                    lines = [f"{pad}Slider {{"] + [f"{pad}  {p}" for p in props]
                    lines += [f"{pad}  onChange: {{"] + [f"{pad}    {ln}" for ln in h[1]]
                    lines += [f"{pad}  }}", f"{pad}}}"]
                    return lines
                props.append(f"onChange: {h}")
            return [f"{pad}Slider {{ {'; '.join(props)} }}"]

        for fname, tag, listkey, selkey, pixlist, pixsel in (
            ("select", "Select", "options", "selected", "options", "selected"),
            ("radio_group", "RadioGroup", "options", "selected", "options", "selected"),
            ("tab_bar", "TabBar", "labels", "active", "labels", "active"),
        ):
            if self._is_ui(node.func, fname):
                if listkey not in kw:
                    raise Untranslatable(node, f"{fname}() needs {listkey}=")
                if selkey not in kw:
                    raise Untranslatable(node, f"{fname}() needs {selkey}=")
                props = [
                    f"{pixlist}: {list_read(kw[listkey], listkey + '=')}",
                    f"{pixsel}: {typed_read(kw[selkey], 'Int', selkey + '=')}",
                ]
                if "on_change" in kw:
                    h = self.handler(kw["on_change"], takes_text=True, implicit=("index", "Int"))
                    if isinstance(h, tuple):
                        lines = [f"{pad}{tag} {{"] + [f"{pad}  {p}" for p in props]
                        lines += [f"{pad}  onSelect: {{"] + [f"{pad}    {ln}" for ln in h[1]]
                        lines += [f"{pad}  }}", f"{pad}}}"]
                        return lines
                    props.append(f"onSelect: {h}")
                return [f"{pad}{tag} {{ {'; '.join(props)} }}"]

        for fname, tag in (("bar_chart", "BarChart"), ("line_chart", "LineChart")):
            if self._is_ui(node.func, fname):
                if not node.args:
                    raise Untranslatable(node, f"{fname}() needs its data")
                c = self._cell_read(node.args[0])
                a0 = node.args[0]
                if (
                    c is None
                    and isinstance(a0, ast.Attribute)
                    and isinstance(a0.value, ast.Name)
                    and a0.value.id in self.stores
                    and self.stores[a0.value.id]["field_tys"].get(a0.attr) in ("List<Float>", "List<Int>")
                ):
                    props = [f"data: {a0.value.id}.{a0.attr}"]
                elif c is None or self.cells.get(c) not in ("List<Float>", "List<Int>"):
                    raise Untranslatable(
                        node.args[0], f"{fname} data is a list[float] or list[int] state or store-field read"
                    )
                else:
                    props = [f"data: App.{c}"]
                if "labels" in kw:
                    la = kw["labels"]
                    lc = self._cell_read(la)
                    if (
                        lc is None
                        and isinstance(la, ast.Attribute)
                        and isinstance(la.value, ast.Name)
                        and la.value.id in self.stores
                        and self.stores[la.value.id]["field_tys"].get(la.attr) == "List<String>"
                    ):
                        props.append(f"labels: {la.value.id}.{la.attr}")
                    elif lc is None or self.cells.get(lc) != "List<String>":
                        raise Untranslatable(la, "labels= is a list[str] state or store-field read — a literal list is not accepted here yet")
                    else:
                        props.append(f"labels: App.{lc}")
                for prop, pix in (("width", "width"), ("height", "height")):
                    val = self._num(kw, prop)
                    if val is not None:
                        props.append(f"{pix}: {val}")
                return [f"{pad}{tag} {{ {'; '.join(props)} }}"]

        if self._is_ui(node.func, "grid"):
            kw2 = kw
            lines = [f"{pad}Grid {{"]
            for prop in ("columns", "rows"):
                val = self._num(kw2, prop)
                if val is not None:
                    lines.append(f"{pad}  {prop}: {val}")
            lines += self._container_props(kw2, pad)
            for child in node.args:
                lines += self.element(child, indent + 1)
            lines.append(f"{pad}}}")
            return lines

        if self._is_ui(node.func, "scroll_view"):
            lines = [f"{pad}ScrollView {{"]
            h = self._num(kw, "height")
            if h is not None:
                lines.append(f"{pad}  height: {h}")
            for child in node.args:
                lines += self.element(child, indent + 1)
            lines.append(f"{pad}}}")
            return lines

        if self._is_ui(node.func, "progress"):
            if not node.args:
                raise Untranslatable(node, "progress() needs its value (0.0..1.0)")
            v = node.args[0]
            if isinstance(v, ast.Constant) and type(v.value) in (int, float) and type(v.value) is not bool:
                val = f"{float(v.value)!r}"
            elif (pc := self._cell_read(v)) is not None and self._ty(pc) == "Float":
                val = f"App.{pc}"
            else:
                raise Untranslatable(v, "progress takes a float literal or a float state read")
            return [f"{pad}ProgressBar {{ value: {val} }}"]

        if self._is_ui(node.func, "spinner"):
            props = []
            sz = self._num(kw, "size")
            if sz is not None:
                props.append(f"size: {sz}")
            body = "; ".join(props)
            return [f"{pad}Spinner {{ {body} }}" if body else f"{pad}Spinner {{ }}"]

        for fname, tag in (("image", "Image"), ("svg", "Svg")):
            if self._is_ui(node.func, fname):
                src = None
                if node.args and isinstance(node.args[0], ast.Constant) and isinstance(node.args[0].value, str):
                    src = node.args[0].value
                if src is None:
                    raise Untranslatable(node, f"{fname}() takes a string literal source — a dynamic source is not in the dialect yet")
                props = [f'source: "{esc(src)}"']
                for prop in ("width", "height"):
                    val = self._num(kw, prop)
                    if val is not None:
                        props.append(f"{prop}: {val}")
                return [f"{pad}{tag} {{ {'; '.join(props)} }}"]

        for fname, tag in (("stack", "Stack"), ("h_scroll_view", "HScrollView"), ("data_table", "DataTable"), ("modal", "Modal")):
            if self._is_ui(node.func, fname):
                if tag == "Modal" and "open" in kw:
                    raise Untranslatable(node, "a modal is open by existing — wrap it in `if show():` instead of passing open=")
                lines = [f"{pad}{tag} {{"]
                for child in node.args:
                    lines += self.element(child, indent + 1)
                lines.append(f"{pad}}}")
                return lines

        if isinstance(node.func, ast.Name) and node.func.id in self.defs:
            return self._component_use(node, indent)

        raise Untranslatable(node, f"`{ast.unparse(node.func)}` is not an element and not a def in the app — the elements are text, button, text_field, checkbox, switch, slider, select, radio_group, tab_bar, column, row, grid, stack, list_view, scroll_view, h_scroll_view, data_table, modal, image, svg, bar_chart, line_chart, progress, spinner")

    def _num(self, kw, name):
        """A number-valued element property. A literal is written as
        itself; a state read, a field or arithmetic over them is
        written as the expression, because a view rebuilds after every
        event and reads it again."""
        if name not in kw:
            return None
        v = kw[name]
        neg = isinstance(v, ast.UnaryOp) and isinstance(v.op, ast.USub)
        c = v.operand if neg else v
        if isinstance(c, ast.Constant) and type(c.value) in (int, float):
            val = -c.value if neg else c.value
            return int(val) if float(val).is_integer() else val
        if self._num_ty(v, "view", None) in ("Int", "Float"):
            prev, self.in_prop = self.in_prop, True
            try:
                return self.expr(v, "view", None)
            finally:
                self.in_prop = prev
        raise Untranslatable(v, f"{name}= takes a number, a state or field read, or arithmetic over them — `{self._src(v)}` is not one of those here")

    def _strlit(self, kw, name):
        """A string-valued element property (a colour, a theme token,
        a placeholder). Same rule as `_num`: a literal, or a str-typed
        read the view repeats on every rebuild."""
        if name not in kw:
            return None
        v = kw[name]
        if isinstance(v, ast.Constant) and isinstance(v.value, str):
            return v.value
        if self._num_ty(v, "view", None) == "String":
            return RawPix(self.expr(v, "view", None))
        raise Untranslatable(v, f"{name}= takes a string (a hex colour or a theme token), or a str state or field read — `{self._src(v)}` is not one of those here")

    def _container_props(self, kw, pad) -> list[str]:
        lines = []
        for prop in ("spacing", "padding"):
            val = self._num(kw, prop)
            if val is not None:
                lines.append(f"{pad}  {prop}: {val}")
        bg = self._strlit(kw, "background")
        if bg is not None:
            lines.append(f"{pad}  background: {pixstr(bg)}")
        for prop, pix in (("grow", "grow"), ("border_radius", "borderRadius"), ("border_width", "borderWidth")):
            val = self._num(kw, prop)
            if val is not None:
                lines.append(f"{pad}  {pix}: {val}")
        bc = self._strlit(kw, "border_color")
        if bc is not None:
            lines.append(f"{pad}  borderColor: {pixstr(bc)}")
        return lines

    def with_element(self, node: ast.With, indent: int) -> list[str]:
        """`with ui.column(...):` — the declarative twin. Children come
        from the block; the emitted .pix is identical to the
        functional form's (one tree, two spellings)."""
        pad = "  " * indent
        if len(node.items) != 1 or node.items[0].optional_vars is not None:
            raise Untranslatable(node, "a `with` block opens one container without `as` (`with column(...):`)")
        call = node.items[0].context_expr
        tags = {
            "column": "Column",
            "row": "Row",
            "grid": "Grid",
            "stack": "Stack",
            "scroll_view": "ScrollView",
            "h_scroll_view": "HScrollView",
            "data_table": "DataTable",
            "modal": "Modal",
        }
        tag = None
        if isinstance(call, ast.Call):
            for fname, t in tags.items():
                if self._is_ui(call.func, fname):
                    tag = t
                    break
        if (
            tag is None
            and isinstance(call, ast.Call)
            and isinstance(call.func, ast.Name)
            and call.func.id in self.comp_defs
        ):
            if call.func.id not in self.comp_slotted:
                raise Untranslatable(
                    call, f"`{call.func.id}` takes no children — declare it @component(slots=True) and place them with slot()"
                )
            comp, params, _ = self._component_for_call(call)
            props = self._component_props(call, params)
            lines = [f"{pad}{comp} {{"]
            if props:
                lines.append(f"{pad}  {props.replace('; ', chr(10) + pad + '  ') if False else props}")
            lines += self._block_stmts(node.body, indent + 1)
            lines.append(f"{pad}}}")
            return lines
        if tag is None:
            raise Untranslatable(call, "a `with` block opens a container: column, row, grid, stack, scroll_view, h_scroll_view, data_table, modal, or a component with slots")
        if call.args:
            raise Untranslatable(call, "inside a `with` block, children go in the block, not as arguments")
        kw = {k.arg: k.value for k in call.keywords if k.arg}
        style_rider = None
        for k in call.keywords:
            if k.arg is not None:
                continue
            if not (isinstance(k.value, ast.Name) and k.value.id in self.styles):
                raise Untranslatable(k.value, "`**` on an element takes a style (`**chip` where `chip = style(...)`)")
            if style_rider is not None:
                raise Untranslatable(k.value, "one style per element — compose with `|` first")
            style_rider = self.styles[k.value.id][0]
        theme_rider = None
        if "theme" in kw:
            if tag not in ("Column", "Row"):
                raise Untranslatable(call, "theme= goes on a column or row (it scopes that subtree) — other elements do not take it yet")
            tv = kw.pop("theme")
            if isinstance(tv, ast.Constant) and type(tv.value) is str:
                theme_rider = f'"{esc(tv.value)}"'
            elif (tc := self._cell_read(tv)) is not None and self._ty(tc) == "String":
                theme_rider = f"App.{tc}"
            else:
                raise Untranslatable(tv, 'theme= takes "light", "dark" or a str state read')
        if tag == "Modal" and "open" in kw:
            raise Untranslatable(call, "a modal is open by existing — wrap it in `if show():` instead of passing open=")
        lines = [f"{pad}{tag} {{"]
        if style_rider:
            lines.append(f"{pad}  style: {style_rider}")
        if theme_rider:
            lines.append(f"{pad}  theme: {theme_rider}")
        for p in self._anim_props(kw):
            lines.append(f"{pad}  {p}")
        if tag == "Grid":
            for prop in ("columns", "rows"):
                val = self._num(kw, prop)
                if val is not None:
                    lines.append(f"{pad}  {prop}: {val}")
        if tag == "ScrollView":
            h = self._num(kw, "height")
            if h is not None:
                lines.append(f"{pad}  height: {h}")
        if tag in ("Column", "Row", "Grid"):
            lines += self._container_props(kw, pad)
        lines += self._block_stmts(node.body, indent + 1)
        lines.append(f"{pad}}}")
        return lines

    def _block_stmts(self, stmts, indent: int) -> list[str]:
        lines = []
        for stmt in stmts:
            if isinstance(stmt, ast.With):
                lines += self.with_element(stmt, indent)
            elif (
                isinstance(stmt, ast.Expr)
                and isinstance(stmt.value, ast.Call)
                and self._is_ui(stmt.value.func, "slot")
            ):
                if not self.in_slotted_comp:
                    raise Untranslatable(stmt, "slot() lives inside a @component(slots=True) body")
                lines.append("  " * indent + "Slot { }")
            elif isinstance(stmt, ast.Expr) and isinstance(stmt.value, ast.Call):
                lines += self.element(stmt.value, indent)
            elif isinstance(stmt, ast.If):
                lines += self._if_lines(stmt, indent)
            elif isinstance(stmt, ast.Match) and (
                (c0 := self._cell_read(stmt.subject)) is not None
                and self._ty(c0) in self.unions
            ):
                pad = "  " * indent
                uname = self._ty(c0)
                lines.append(f"{pad}case App.{c0} {{")
                for case in stmt.cases:
                    if case.guard is not None:
                        raise Untranslatable(case.pattern, "a match guard (`case X if cond:`) is not in the dialect yet — test the condition inside the arm")
                    arm, binds = self._union_arm(case.pattern, uname)
                    lines.append(f"{pad}  {arm} {{")
                    for b, t in binds:
                        self.view_bindings[b] = t
                    lines += self._block_stmts(case.body, indent + 2)
                    for b, _t in binds:
                        self.view_bindings.pop(b, None)
                    lines.append(f"{pad}  }}")
                lines.append(f"{pad}}}")
            elif isinstance(stmt, ast.Match):
                c = self._cell_read(stmt.subject)
                if c is None or self._ty(c) not in self.enums:
                    raise Untranslatable(stmt.subject, "match takes an Enum or sum-type state or field — matching on int or str literals is not in the dialect yet")
                pad = "  " * indent
                lines.append(f"{pad}case App.{c} {{")
                for case in stmt.cases:
                    if case.guard is not None:
                        raise Untranslatable(case.pattern, "a match guard (`case X if cond:`) is not in the dialect yet — test the condition inside the arm")
                    p = case.pattern
                    if (
                        isinstance(p, ast.MatchValue)
                        and isinstance(p.value, ast.Attribute)
                        and isinstance(p.value.value, ast.Name)
                        and p.value.value.id == self._ty(c)
                    ):
                        lines.append(f"{pad}  when {p.value.attr} {{")
                    elif isinstance(p, ast.MatchAs) and p.pattern is None and p.name is None:
                        lines.append(f"{pad}  when _ {{")
                    else:
                        raise Untranslatable(p, "a match arm is `Mood.MEMBER` or `_` — `|` patterns and literals are not in the dialect yet")
                    lines += self._block_stmts(case.body, indent + 2)
                    lines.append(f"{pad}  }}")
                lines.append(f"{pad}}}")
            elif isinstance(stmt, ast.Pass):
                continue
            else:
                raise Untranslatable(
                    stmt, "a `with` block holds element calls, nested `with` blocks, `if`/`elif`/`else` and `match` — loops and locals are not in the dialect in views (use list_view and store fields)"
                )
        return lines

    def _if_lines(self, node: ast.If, indent: int) -> list[str]:
        pad = "  " * indent
        opt = self._optional_test(node.test)
        if opt is not None:
            src, binding, none_first = opt
            kind, key = src
            if kind == "cell":
                subject, sub_ty = f"App.{key}", self._ty(key)
            elif kind == "storefield":
                sname, fname = key.split(".", 1)
                subject, sub_ty = key, self.stores[sname]["field_tys"][fname]
            elif kind == "instfield":
                iname, fname = key.split(".", 1)
                mi = self.model_instances[iname]
                subject = f"App.{key}"
                sub_ty = {f: t for f, t, _ in self.models[mi]["fields"]}[fname]
            else:
                raise Untranslatable(node.test, "a view narrows a state read, a store field or a model field (`if (v := sel()) is not None:`)")
            some_body = node.orelse if none_first else node.body
            none_body = node.body if none_first else node.orelse
            v = binding or "__opt"
            lines = [f"{pad}if let some({v}) = {subject} {{"]
            self.view_bindings[v] = sub_ty[:-1]
            try:
                lines += self._block_stmts(some_body, indent + 1) if some_body else []
            finally:
                self.view_bindings.pop(v, None)
            if none_body:
                lines.append(f"{pad}}} else {{")
                lines += self._block_stmts(none_body, indent + 1)
            lines.append(f"{pad}}}")
            return lines
        lines = [f"{pad}if {self._cond(node.test)} {{"]
        lines += self._block_stmts(node.body, indent + 1)
        if node.orelse:
            lines.append(f"{pad}}} else {{")
            lines += self._block_stmts(node.orelse, indent + 1)
        lines.append(f"{pad}}}")
        return lines

    def _by_reference(self, d: ast.FunctionDef) -> dict:
        """The parameters a component takes by reference rather than by
        value: a callback (no annotation, or `Callable[...]`) and a
        `State[...]` cell. A pixie view parameter carries a value, so
        these cannot cross as parameters."""
        out = {}
        for a in d.args.args:
            ann = a.annotation
            if ann is None:
                out[a.arg] = "callback"
            elif isinstance(ann, ast.Name) and ann.id == "Callable":
                out[a.arg] = "callback"
            elif isinstance(ann, ast.Subscript) and isinstance(ann.value, ast.Name) and ann.value.id == "Callable":
                out[a.arg] = "callback"
            elif isinstance(ann, ast.Subscript) and (
                (isinstance(ann.value, ast.Name) and self.yokan_names.get(ann.value.id) == "State")
                or (isinstance(ann.value, ast.Attribute) and ann.value.attr == "State")
            ):
                out[a.arg] = "state"
        return out

    def _component_for_call(self, call: ast.Call):
        """The view a component call uses. A component whose parameters
        are all values is one view, shared by every call site. One that
        takes a callback or a State becomes a view PER CALL SITE, with
        the argument written into the body — the handler and the cell
        live in the caller, and a view parameter cannot carry them.
        Two call sites that pass the same thing share one view."""
        pyname = call.func.id
        d = self.defs[pyname]
        by_ref = self._by_reference(d)
        if not by_ref:
            return self._component(pyname)
        names = [a.arg for a in d.args.args]
        bound = {}
        for i, a in enumerate(call.args):
            if i >= len(names):
                raise Untranslatable(a, f"{pyname}() takes {len(names)} argument(s)")
            bound[names[i]] = a
        for k in call.keywords:
            if k.arg is None or k.arg not in names:
                raise Untranslatable(k.value, f"`{k.arg}=` is not a parameter of {pyname}")
            bound[k.arg] = k.value
        missing = [n for n in by_ref if n not in bound]
        if missing:
            raise Untranslatable(call, f"{pyname}() needs {', '.join(missing)}")
        key = (pyname,) + tuple(sorted(f"{n}={ast.unparse(bound[n])}" for n in by_ref))
        if key in self.comp_specials:
            return self.comp_specials[key]
        spec = copy.deepcopy(d)
        spec.args.args = [a for a in spec.args.args if a.arg not in by_ref]
        subs = {n: bound[n] for n in by_ref}
        _CompArgs(subs).visit(spec)
        n = sum(1 for k2 in self.comp_specials if k2[0] == pyname)
        alias = pyname if n == 0 else f"{pyname}_{n + 1}"
        entry = self._component(alias, spec)
        self.comp_specials[key] = entry
        return entry

    def _component(self, pyname: str, d: ast.FunctionDef | None = None):
        """Translate a helper def into a pixie component (once)."""
        if pyname in self.components:
            return self.components[pyname]
        d = d if d is not None else self.defs[pyname]
        comp = pyname[0].upper() + pyname[1:]
        if comp in self.RESERVED or any(c[0] == comp for c in self.components.values()):
            raise Untranslatable(d, f"component name `{pyname}` collides with another name after capitalization — rename it")
        params = []
        for a in d.args.args:
            if a.annotation is None:
                raise Untranslatable(a, f"annotate the component parameter `{a.arg}` (int, float, str, bool or list[...] of those)")
            ty = self._param_ty(a.annotation)
            if ty in self.structs or ty in self.enums:
                raise Untranslatable(
                    a,
                    f"a component parameter is int, float, str, bool or `list[...]` of those — a value class or an enum "
                    "cannot be read inside a component body yet; pass the fields",
                )
            if ty is None:
                raise Untranslatable(
                    a,
                    f"a component parameter is int, float, str, bool or `list[...]` of those — "
                    f"`{self._src(a.annotation)}` is not in the dialect yet (a callback or a State parameter is planned)",
                )
            params.append((a.arg, ty))
        # leading per-instance state: `n: ui.State[int] = ui.local(0)`
        body_stmts = list(d.body)
        locals_ = []
        while body_stmts and isinstance(body_stmts[0], ast.AnnAssign):
            st = body_stmts[0]
            v = st.value
            is_local = isinstance(v, ast.Call) and self._is_ui(v.func, "local")
            if not is_local:
                break
            if not isinstance(st.target, ast.Name):
                raise Untranslatable(st, "local(...) binds a plain name (`n: State[int] = local(0)`)")
            ann = st.annotation
            ok = (
                isinstance(ann, ast.Subscript)
                and (
                    (isinstance(ann.value, ast.Attribute) and ann.value.attr == "State")
                    or (isinstance(ann.value, ast.Name) and self.yokan_names.get(ann.value.id) == "State")
                )
            )
            if not ok:
                raise Untranslatable(ann, f"local(...) holds int, str, bool, float or a `list[...]` of those — `{self._src(ann)}` is not in the dialect yet (annotate as `n: State[int] = local(0)`)")
            lty = self._param_ty(ann.slice)
            if lty is None or (lty not in ("Int", "Float", "String", "Bool") and not lty.startswith("List<")):
                raise Untranslatable(ann, f"local(...) holds int, str, bool, float or a `list[...]` of those — `{self._src(ann.slice)}` is not in the dialect yet")
            if len(v.args) != 1:
                raise Untranslatable(v, "local(...) takes exactly one argument, the initial value")
            if lty.startswith("List<"):
                lit = self._list_literal(v.args[0], lty[5:-1])
            else:
                got, lit = self._state_field(v.args[0])
                if got != lty:
                    raise Untranslatable(v, f"the initial value is {self._py_ty(got)}, the annotation says {self._py_ty(lty)}")
            locals_.append((st.target.id, lty, lit))
            body_stmts.pop(0)
        if locals_ and pyname not in self.comp_defs:
            raise Untranslatable(d, "local(...) lives in a @component body")

        # reserve the slot first so recursion is caught, then translate
        self.components[pyname] = (comp, params, [])
        prev = self.comp_params
        prev_locals = self.comp_locals
        prev_slotted = self.in_slotted_comp
        self.comp_params = dict(params)
        self.comp_locals = {ln: lt for ln, lt, _ in locals_}
        self.in_slotted_comp = pyname in self.comp_slotted
        try:
            if len(body_stmts) == 1 and isinstance(body_stmts[0], ast.Return):
                body = self.element(body_stmts[0].value, 1)
            elif len(body_stmts) == 1 and isinstance(body_stmts[0], ast.With):
                body = self.with_element(body_stmts[0], 1)
            else:
                raise Untranslatable(d, "a component body is `local(...)` declarations followed by one `with` block (or a single return) — an `if` or other statement at the top of the body is not in the dialect yet")
        finally:
            self.comp_params = prev
            self.comp_locals = prev_locals
            self.in_slotted_comp = prev_slotted
        sig = comp if not params else f"{comp}({', '.join(f'{n}: {t}' for n, t in params)})"
        lines = [f"view {sig} {{"]
        lines += [f"  state {ln} : {lt} = {ll}" for ln, lt, ll in locals_]
        lines += body + ["}"]
        self.components[pyname] = (comp, params, lines)
        return self.components[pyname]

    def _component_props(self, node: ast.Call, params) -> str:
        d = self.defs[node.func.id]
        names = [a.arg for a in d.args.args]
        want = {n for n, _ in params}
        args = {}
        for i, a in enumerate(node.args):
            if i >= len(names):
                raise Untranslatable(a, f"{node.func.id}() takes {len(names)} argument(s)")
            args[names[i]] = a
        for k in node.keywords:
            if k.arg is None or k.arg not in names:
                raise Untranslatable(k.value, f"`{k.arg}=` is not a parameter of {node.func.id}")
            args[k.arg] = k.value
        if set(args) != set(names):
            raise Untranslatable(node, f"{node.func.id}() needs all {len(names)} argument(s)")
        return "; ".join(f"{name}: {self._comp_arg(args[name])}" for name, _ in params if name in want)

    def _comp_arg(self, a) -> str:
        if isinstance(a, ast.JoinedStr):
            return self.fstring(a)
        return self.expr(a, "view")

    def _component_use(self, node: ast.Call, indent: int) -> list[str]:
        pad = "  " * indent
        if node.func.id in self.comp_slotted:
            raise Untranslatable(
                node, f"`{node.func.id}` takes children — use `with {node.func.id}(...):`"
            )
        comp, params, _ = self._component_for_call(node)
        props = self._component_props(node, params)
        if not props:
            return [f"{pad}{comp} {{ }}"]
        return [f"{pad}{comp} {{ {props} }}"]

    # ---- whole program ---------------------------------------------

    def _emit_timers(self):
        """One store method per declared timer, marked with the period
        the engine fires it at."""
        for i, (ms, fn, call) in enumerate(self.timers):
            if not (
                (isinstance(fn, ast.Name) and fn.id in self.defs)
                or (
                    isinstance(fn, ast.Attribute)
                    and isinstance(fn.value, ast.Name)
                    and fn.value.id in self.stores
                )
            ):
                raise Untranslatable(fn, "every() runs a module-level def or a store's bound method")
            callstr = self.handler(fn, takes_text=False)
            name = f"__tick{i}"
            self.timer_handlers[name] = ms
            self.handlers.append((name, None, [callstr]))

    def translate(self) -> str:
        trees = [self.tree, *self.modules.values()]
        for tree in trees:
            OptionalSpelling().visit(tree)
        consts, decls = module_consts(trees)
        if consts:
            inline = ConstInliner(consts, decls)
            for tree in trees:
                inline.visit(tree)
        self.scan()
        self._emit_timers()
        if ("width" in self.window) != ("height" in self.window):
            raise Untranslatable(self.view, "width= and height= come as a pair")
        body = self.view.body
        if len(body) == 1 and isinstance(body[0], ast.Return):
            view_lines = self.element(body[0].value, 1)
        elif len(body) == 1 and isinstance(body[0], ast.With):
            view_lines = self.with_element(body[0], 1)
        else:
            raise Untranslatable(
                self.view,
                "view() is one `with column(...):` block (or a single `return column(...)`) — one root, no other statements",
            )

        out = []
        crate_struct_names = set()
        crate_enum_names = set()
        for cname in self.uses_crates:
            for sn, cs in self.crate_info.get(cname, {}).get("structs", {}).items():
                if cs["ok"] is True:
                    crate_struct_names.add(sn)
            for en, ce in self.crate_info.get(cname, {}).get("enums", {}).items():
                if ce["ok"] is True:
                    crate_enum_names.add(en)
        for ename, members in sorted(self.enums.items()):
            if ename in crate_enum_names:
                # The used crate's .rpi declares this enum with its
                # @rust correspondence; the app's twin serves the
                # interpreted run only.
                continue
            out.append(f"enum {ename} {{")
            for m in members:
                out.append(f"  {m}")
            out.append("}")
            out.append("")
        for ename in sorted(self.enum_meta_used):
            self.enum_text_used.add(ename)   # they share the PyText class
        if self.enum_text_used:
            # str(Enum.MEMBER) twins: text holes call these statics so
            # both tiers print "Class.MEMBER".
            out.append("class PyText {")
            for i, ename in enumerate(sorted(self.enum_text_used)):
                if i:
                    out.append("")
                out.append(f"  pub static fn enumStr{ename}(v: {ename}) String {{")
                out.append('    var out = ""')
                out.append("    case v {")
                for mname in self.enums[ename]:
                    out.append(f'      when {mname} {{ out = "{ename}.{mname}" }}')
                out.append("    }")
                out.append("    out")
                out.append("  }")
                if ename in self.enum_meta_used:
                    out.append("")
                    out.append(f"  pub static fn enumName{ename}(v: {ename}) String {{")
                    out.append('    var out = ""')
                    out.append("    case v {")
                    for mname in self.enums[ename]:
                        out.append(f'      when {mname} {{ out = "{mname}" }}')
                    out.append("    }")
                    out.append("    out")
                    out.append("  }")
                    vty = self._enum_value_ty(ename)
                    if vty is not None:
                        zero = '""' if vty == "String" else "0"
                        out.append("")
                        out.append(f"  pub static fn enumValue{ename}(v: {ename}) {vty} {{")
                        out.append(f"    var out = {zero}")
                        out.append("    case v {")
                        for mname in self.enums[ename]:
                            val = self.enum_values.get(ename, {}).get(mname)
                            lit = f'"{esc(val)}"' if isinstance(val, str) else str(val)
                            out.append(f"      when {mname} {{ out = {lit} }}")
                        out.append("    }")
                        out.append("    out")
                        out.append("  }")
            out.append("}")
            out.append("")
        for uname, variants in sorted(self.unions.items()):
            out.append(f"enum {uname} {{")
            for vname in variants:
                fields = self.structs[vname]
                if fields:
                    sig = ", ".join(f"{f}: {t}" for f, t, _ in fields)
                    out.append(f"  {vname}({sig})")
                else:
                    out.append(f"  {vname}")
            out.append("}")
            out.append("")
        for sname, sfields in sorted(self.structs.items()):
            if sname in self.union_of:
                continue
            if sname in crate_struct_names or sname in self.escape_structs:
                # A used crate's .rpi already declares this struct,
                # WITH its @rust correspondence — the app's @value
                # twin serves the interpreted run only. A second,
                # correspondence-less declaration would shadow it.
                # An @py escape's generated crate says the same thing.
                if self.struct_methods.get(sname):
                    raise Untranslatable(
                        self.tree,
                        f"@value class {sname} is a crate struct's twin — methods on it are not carried yet",
                    )
                continue
            if not sfields:
                raise Untranslatable(self.tree, f"value class `{sname}` has no fields and belongs to no sum type — give it a field, or list it in a `type` alias")
            out.append(f"struct {sname} {{")
            for f, ty, lit in sfields:
                out.append(f"  var {f} : {ty}" + (f" = {lit}" if lit is not None else ""))
            for _py, (_em, _ps, _ret, mlines) in sorted(self.struct_methods.get(sname, {}).items()):
                out.append("")
                out += mlines
            out.append("}")
            out.append("")
        for _pyname, (_pix, lines) in sorted(self.styles.items()):
            out += lines
            out.append("")
        for tname, tmethods in sorted(self.protocols.items()):
            out.append(f"trait {tname} {{")
            for m, ps, ret in tmethods:
                sig = ", ".join(f"{p}: {t}" for p, t in ps)
                out.append(f"  fn {m}({sig}) {ret}" if ps else f"  fn {m} {ret}")
            out.append("}")
            out.append("")
        for mname, info in sorted(self.models.items()):
            out.append(f"class {mname} {{")
            for f, ty, lit in info["fields"]:
                kw = "pub weak prop" if f in info.get("weak", ()) else "pub prop"
                out.append(f"  {kw} {f} : {ty}, default: {lit}")
            for lines in info["methods"]:
                out.append("")
                out += lines
            out.append("}")
            out.append("")
            for tname, impl_methods in sorted(info["impls"].items()):
                out.append(f"impl {tname} for {mname} {{")
                for i2, lines in enumerate(impl_methods):
                    if i2:
                        out.append("")
                    out += lines
                out.append("}")
                out.append("")
        for sname, info in sorted(self.stores.items()):
            out.append(f"store {sname} {{")
            for f, ty, lit in info["fields"]:
                out.append(f"  state {f} : {ty} = {lit}")
            for lines in info["methods"]:
                out.append("")
                out += lines
            out.append("}")
            out.append("")
        if self.state or self.handlers or self.model_instances:
            out.append("store App {")
            for k, (ty, lit) in self.state.items():
                out.append(f"  state {k} : {ty} = {lit}")
            for iname, mname in self.model_instances.items():
                out.append(f"  state {iname} : {mname} = {mname}()")
            for name, param, stmts in self.handlers:
                out.append("")
                kw = "async fn" if name in self.async_handlers else "fn"
                mark = f" @every({self.timer_handlers[name]})" if name in self.timer_handlers else ""
                if isinstance(param, list):
                    sig = f"  {kw} {name}({', '.join(f'{n}: {t}' for n, t in param)}){mark} {{"
                else:
                    sig = f"  {kw} {name}(t: {param}){mark} {{" if param else f"  {kw} {name}{mark} {{"
                out.append(sig)
                for st in stmts:
                    out.append(f"    {st}")
                out.append("  }")
            out.append("}")
        static_lines = []
        for _name, (_p, _r, lines, bounded) in sorted(self.helpers.items()):
            if not lines:
                continue
            if bounded:
                out.append("")
                out += lines
            else:
                static_lines.append(lines)
        if static_lines:
            out.append("")
            out.append("class Helpers {")
            for i, lines in enumerate(static_lines):
                if i:
                    out.append("")
                out += lines
            out.append("}")
        for _, (_, _, lines) in sorted(self.components.items()):
            out.append("")
            out += lines
        out.append("")
        out.append("view Main {")
        out += view_lines
        out.append("}")
        return "\n".join(out) + "\n"


# -------------------------------------------------------------------
# The gate.

def _local_modules(entry: str) -> dict:
    """Parse sibling .py modules reachable from the entry via
    `import m` / `from m import x` (flattened for now — one emitted
    .pix; .pix-module-per-file mapping is recorded future work)."""
    base = os.path.dirname(os.path.abspath(entry))
    seen: dict = {}

    def walk(tree):
        for node in ast.walk(tree):
            names = []
            if isinstance(node, ast.Import):
                names = [a.name for a in node.names]
            elif isinstance(node, ast.ImportFrom) and node.module and node.level == 0:
                names = [node.module]
            for m in names:
                if m in seen or m == "yokan":
                    continue
                path = os.path.join(base, m + ".py")
                if os.path.isfile(path):
                    t = parse_source(path)
                    seen[m] = t
                    walk(t)

    walk(parse_source(entry))
    return seen


CRATE_CROSSING = ("Int", "Float", "Bool", "String")


def _split_top(s: str) -> list[str]:
    """Split on commas outside angle brackets — `m: Map<String, Int>`
    is one parameter, not two."""
    parts, depth, cur = [], 0, []
    for ch in s:
        if ch == "<":
            depth += 1
        elif ch == ">":
            depth -= 1
        if ch == "," and depth == 0:
            parts.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    if cur:
        parts.append("".join(cur))
    return parts


def _crate_crossable(t: str, structs=None, enums=None):
    """None if the pix type crosses the user-crate boundary, else the
    reason it does not (named, so the refusal teaches)."""
    structs = structs or {}
    enums = enums or {}
    if t in CRATE_CROSSING:
        return None
    if t in structs:
        ok = structs[t]["ok"]
        return None if ok is True else f"struct `{t}` — {ok}"
    if t in enums:
        ok = enums[t]["ok"]
        return None if ok is True else f"enum `{t}` — {ok}"
    m = re.fullmatch(r"List<(\w+)>", t)
    if m and m.group(1) in CRATE_CROSSING:
        return None
    m = re.fullmatch(r"(\w+)\?", t)
    if m and m.group(1) in CRATE_CROSSING:
        return None
    if t == "Bytes":
        return "the dialect has no bytes values yet, so nothing could hold it"
    m = re.fullmatch(r"Map<(\w+), (\w+)>", t)
    if m and m.group(1) == "String" and m.group(2) in CRATE_CROSSING:
        return None
    if t.startswith("Map<"):
        return "only `dict[str, …]` with int/float/bool/str values crosses a user crate"
    return (
        f"`{py_ty(t)}` does not cross a user crate yet — int, float, bool, str, "
        "flat value classes, and Optionals and lists of those do"
    )


def _pep723_table(path: str):
    """The app's PEP 723 tool table, or None when there is no block."""
    import tomllib

    src = open(path).read()
    m = re.search(r"(?ms)^# /// script\n(.*?)^# ///$", src)
    if not m:
        return None
    block = "\n".join(
        line[2:] if line.startswith("# ") else line[1:] for line in m.group(1).splitlines()
    )
    try:
        return tomllib.loads(block)
    except tomllib.TOMLDecodeError:
        return None


def app_crate_decls(path: str) -> dict:
    """The app's `[tool.yokan.crates]` — from the PEP 723 block
    (script-style apps) or, failing that, the nearest pyproject.toml
    upward (project-style apps); the same precedence uv gives
    `dependencies`. Entries: `name = "1.2"` (crates.io version) or
    `name = { path = "…" }`, either with `features = […]`. Returns
    name -> {"path": abs | "version": str, "features": [..]}."""
    import tomllib

    crates = None
    base_dir = os.path.dirname(os.path.abspath(path))
    data = _pep723_table(path)
    if data is not None:
        crates = data.get("tool", {}).get("yokan", {}).get("crates")
    if crates is None:
        d = base_dir
        while True:
            pp = os.path.join(d, "pyproject.toml")
            if os.path.isfile(pp):
                try:
                    with open(pp, "rb") as f:
                        crates = tomllib.load(f).get("tool", {}).get("yokan", {}).get("crates")
                except tomllib.TOMLDecodeError:
                    crates = None
                if crates is not None:
                    base_dir = d
                break
            up = os.path.dirname(d)
            if up == d:
                break
            d = up
    if not crates:
        return {}
    out = {}
    for name, spec in crates.items():
        if not re.fullmatch(r"[a-z][a-z0-9_-]*", name):
            raise ValueError(f"[tool.yokan.crates] `{name}`: crate names are lowercase")
        if isinstance(spec, str):
            out[name] = {"version": spec, "features": []}
        elif isinstance(spec, dict) and "path" in spec:
            d = os.path.normpath(os.path.join(base_dir, spec["path"]))
            if not os.path.isfile(os.path.join(d, "Cargo.toml")):
                raise ValueError(f"[tool.yokan.crates] `{name}`: no crate at {d}")
            out[name] = {"path": d, "features": list(spec.get("features", []))}
        elif isinstance(spec, dict) and "version" in spec:
            out[name] = {"version": str(spec["version"]), "features": list(spec.get("features", []))}
        else:
            raise ValueError(
                f"[tool.yokan.crates] `{name}`: write a version string "
                f'({name} = "1.2") or a path table ({name} = {{ path = "…" }})'
            )
    return out


def _crate_dep_spec(name: str, spec: dict) -> str:
    """The Cargo/pixie dependency table for a declared crate — the
    same line serves the pyo3 shim, the scratch manifest and the
    generated project's [crates]."""
    parts = []
    if "path" in spec:
        parts.append(f'path = "{spec["path"]}"')
    if "version" in spec:
        parts.append(f'version = "{spec["version"]}"')
    if spec.get("features"):
        fs = ", ".join(f'"{f}"' for f in spec["features"])
        parts.append(f"features = [{fs}]")
    return f"{name} = {{ {', '.join(parts)} }}"


def _crate_class(name: str) -> str:
    return "".join(p.title() for p in re.split(r"[-_]", name))


def _tree_mtime(d: str) -> float:
    worst = 0.0
    for root, _dirs, files in os.walk(d):
        if "target" in root.split(os.sep):
            continue
        for f in files:
            try:
                worst = max(worst, os.path.getmtime(os.path.join(root, f)))
            except OSError:
                pass
    return worst


def prepare_crates(app_path: str, decls: dict) -> dict:
    """Derive each declared crate's binding (rustdoc JSON → rpi-gen,
    cached in <app>/.yokan/rpi/) and parse the signatures the
    translator checks against. Needs the repository and cargo — the
    whole feature is native-build shaped."""
    repo_dir = repo()
    if not os.path.isdir(os.path.join(repo_dir, "crates", "pixie-rpi-gen")):
        raise ValueError(
            "[tool.yokan.crates] needs the repository checkout (rustdoc-JSON "
            "binding derivation) — clone it and run inside, or set PIXIE_REPO"
        )
    app_dir = os.path.dirname(os.path.abspath(app_path))
    rpi_dir = os.path.join(app_dir, ".yokan", "rpi")
    os.makedirs(rpi_dir, exist_ok=True)
    info = {}
    for name, spec in decls.items():
        cls = _crate_class(name)
        out_rpi = os.path.join(rpi_dir, f"{name}.rpi")
        feats = ",".join(sorted(spec.get("features", [])))
        if "path" in spec:
            crate_dir = spec["path"]
            vm = re.search(
                r'(?m)^version\s*=\s*"([^"]+)"', open(os.path.join(crate_dir, "Cargo.toml")).read()
            )
            key_ver = vm.group(1) if vm else "path"
            stale = (
                not os.path.exists(out_rpi)
                or os.path.getmtime(out_rpi) < _tree_mtime(crate_dir)
            )
        else:
            crate_dir = None
            key_ver = spec["version"]
            key = f"# pixie-cache-key: version={key_ver}; features={feats}; format=61"
            stale = True
            if os.path.exists(out_rpi):
                with open(out_rpi) as f:
                    stale = f.readline().rstrip("\n") != key
        if stale:
            env = dict(os.environ)
            env["RUSTC_BOOTSTRAP"] = "1"
            env["RUSTDOCFLAGS"] = "-Z unstable-options --output-format json"
            if crate_dir is not None:
                # Path crate: document it in place; the JSON lands in
                # the (possibly shared) cargo target.
                r = subprocess.run(
                    ["cargo", "doc", "--no-deps", "--quiet"],
                    cwd=crate_dir, env=env, capture_output=True, text=True,
                )
                doc_target = env.get("CARGO_TARGET_DIR") or os.path.join(crate_dir, "target")
            else:
                # Version crate: a scratch manifest depending on it
                # (the substrate's own recipe), documented into the
                # scratch's OWN target so the path is deterministic.
                scratch = os.path.join(app_dir, ".yokan", "rpi-scratch", name)
                os.makedirs(os.path.join(scratch, "src"), exist_ok=True)
                open(os.path.join(scratch, "src", "lib.rs"), "w").write("")
                open(os.path.join(scratch, "Cargo.toml"), "w").write(
                    f"# Generated by yokan — rustdoc-JSON scratch for `{name}`.\n"
                    f'[package]\nname = "yokan-rpi-scratch"\nversion = "0.0.0"\nedition = "2024"\n\n'
                    f"[dependencies]\n{_crate_dep_spec(name, spec)}\n\n[workspace]\n"
                )
                env.pop("CARGO_TARGET_DIR", None)
                r = subprocess.run(
                    ["cargo", "doc", "--no-deps", "--quiet", "-p", name],
                    cwd=scratch, env=env, capture_output=True, text=True,
                )
                doc_target = os.path.join(scratch, "target")
            if r.returncode != 0:
                raise ValueError(f"`{name}`: cargo doc failed —\n{r.stderr.strip()}")
            json_path = os.path.join(doc_target, "doc", name.replace("-", "_") + ".json")
            r = subprocess.run(
                ["cargo", "run", "-q", "-p", "pixie-rpi-gen",
                 "--manifest-path", os.path.join(repo_dir, "Cargo.toml"), "--",
                 json_path, "--bind", f"{name.replace('-', '_')}={cls}",
                 "--out", out_rpi],
                capture_output=True, text=True,
            )
            if r.returncode != 0 or not os.path.exists(out_rpi):
                raise ValueError(f"`{name}`: binding derivation failed —\n{r.stderr.strip()}")
            body = open(out_rpi).read()
            open(out_rpi, "w").write(
                f"# pixie-cache-key: version={key_ver}; features={feats}; format=61\n" + body
            )
        rpi_text = open(out_rpi).read()
        structs = {}
        for sm in re.finditer(
            r'struct (\w+) @rust\("([^"]+)"\) \{([^}]*)\}', rpi_text
        ):
            sname, srust, body = sm.groups()
            # fields: (pix name, pix ty, rust field name, rust width ty or None)
            sfields, ok = [], True
            for fl in body.splitlines():
                fl = fl.strip()
                if not fl:
                    continue
                fm = re.fullmatch(r"var (\w+) : (\w+)", fl)
                if fm and fm.group(2) in CRATE_CROSSING:
                    sfields.append((fm.group(1), fm.group(2), fm.group(1), None))
                    continue
                fm = re.fullmatch(r'var (\w+) : (\w+) @rust\("([\w]+): (\w+)"\)', fl)
                if (
                    fm
                    and fm.group(2) in CRATE_CROSSING
                    and fm.group(4) in _WIDTHS
                ):
                    sfields.append((fm.group(1), fm.group(2), fm.group(3), fm.group(4)))
                    continue
                # A field naming another declared struct — admitted
                # provisionally; the fixpoint below confirms the
                # named struct itself crosses.
                fm = re.fullmatch(r"var (\w+) : (\w+)", fl)
                if fm:
                    sfields.append((fm.group(1), fm.group(2), fm.group(1), None))
                    continue
                ok = "a field outside the crossable shapes"
            structs[sname] = {"rust": srust, "fields": sfields, "ok": ok}
        # Nested fields resolve to a fixpoint (mirroring rpi-gen's
        # declaration rule): a struct is ok only while every
        # struct-typed field points at an ok struct.
        changed = True
        while changed:
            changed = False
            for sname, sv in structs.items():
                if sv["ok"] is not True:
                    continue
                for f, fty, _rf, _w in sv["fields"]:
                    if fty in CRATE_CROSSING:
                        continue
                    tv = structs.get(fty)
                    if tv is None:
                        sv["ok"] = f"field `{f}`: `{fty}` does not cross"
                    elif tv["ok"] is not True:
                        sv["ok"] = f"field `{f}`: {tv['ok']}"
                    else:
                        continue
                    changed = True
                    break
        enums = {}
        for em in re.finditer(
            r'enum (\w+) @rust\("([^"]+)"\) \{([^}]*)\}', rpi_text
        ):
            ename, erust, body = em.groups()
            variants, ok = [], True
            for vl in body.splitlines():
                vl = vl.strip()
                if not vl:
                    continue
                if re.fullmatch(r"\w+", vl):
                    variants.append(vl)
                else:
                    ok = "payload-carrying variants stay compiled-only (rpi-gen does not derive them yet)"
            enums[ename] = {"rust": erust, "variants": variants, "ok": ok}
        fns = {}
        for m in re.finditer(
            r"static fn (\w+)\(([^)]*)\)\s+(.+?)\s+@rust\(\"([^\"]+)\"\)",
            rpi_text,
        ):
            rpi_name, params_s, ret, rust_path = m.groups()
            # rpi-gen marks std-HashMap fns with a `stdmap:` path
            # prefix for the compiled adapter; the crate's real path
            # is what the shim calls and the last segment names.
            rust_path = rust_path.removeprefix("stdmap:")
            # The Python-facing name is the crate's own: the last
            # segment of the @rust path (rpi-gen camelizes its side).
            py_name = rust_path.rsplit("::", 1)[-1]
            params = []
            reason = None
            for p in [q.strip() for q in _split_top(params_s) if q.strip()]:
                t = p.split(":", 1)[1].strip()
                reason = reason or _crate_crossable(t, structs, enums)
                params.append(t)
            fallible = ret.startswith("!")
            ret_inner = ret[1:] if fallible else ret
            reason = reason or _crate_crossable(ret_inner, structs, enums)
            fns[py_name] = (
                reason if reason else (params, ret_inner, rpi_name, rust_path, fallible)
            )
        info[name] = {"class": cls, "spec": spec, "rpi": out_rpi, "fns": fns, "structs": structs, "enums": enums}
    return info


_RUST_PARAM = {"Int": "i64", "Float": "f64", "Bool": "bool", "String": "&str"}
_RUST_OWN = {"Int": "i64", "Float": "f64", "Bool": "bool", "String": "String"}
# Exact-width struct fields (§8.78): the pixie side widens on read
# and casts on write; the shim mirrors with `as`.
_WIDTHS = {"u8", "u16", "u32", "u64", "usize", "i8", "i16", "i32", "isize", "i64", "f32", "f64"}


def _shim_sig(t: str, owned: bool) -> str:
    m = re.fullmatch(r"Map<(\w+), (\w+)>", t)
    if m:
        k, v = _RUST_OWN[m.group(1)], _RUST_OWN[m.group(2)]
        # Params arrive as the crate's own HashMap; returns go back
        # sorted (BTreeMap → dict keeps insertion order), matching
        # the kernel Map the compiled run answers.
        return (
            f"std::collections::BTreeMap<{k}, {v}>"
            if owned
            else f"std::collections::HashMap<{k}, {v}>"
        )
    m = re.fullmatch(r"List<(\w+)>", t)
    if m:
        return f"Vec<{_RUST_OWN[m.group(1)]}>"
    m = re.fullmatch(r"(\w+)\?", t)
    if m:
        # The compiled side passes optionals as `.as_ref().map(…)`,
        # so an optional string is `Option<&str>` at the fn — the
        # shim mirrors that exactly.
        inner = (_RUST_OWN if owned else _RUST_PARAM)[m.group(1)]
        return f"Option<{inner}>"
    return (_RUST_OWN if owned else _RUST_PARAM)[t]


def build_shims(app_path: str, tr, names=None) -> list[str]:
    """The interpreted run's door: one auto-generated pyo3 crate per
    declared Rust crate, mirroring the SAME argument adapters the
    compiled run uses (`&str` for String, owned Vec for lists), built
    into <app>/.yokan/ext/<name>.so and loaded by `yokan.crates`."""
    app_dir = os.path.dirname(os.path.abspath(app_path))
    built = []
    for name in sorted(names if names is not None else tr.uses_crates):
        info = tr.crate_info[name]
        modname = "yokan_ext_" + name.replace("-", "_")
        src_dir = os.path.join(app_dir, ".yokan", "ext-src", name)
        ext_dir = os.path.join(app_dir, ".yokan", "ext")
        os.makedirs(os.path.join(src_dir, "src"), exist_ok=True)
        os.makedirs(ext_dir, exist_ok=True)
        so_path = os.path.join(ext_dir, name + ".so")
        fresh = os.path.exists(so_path) and os.path.getmtime(so_path) >= os.path.getmtime(info["rpi"])
        if fresh and "path" in info["spec"]:
            fresh = os.path.getmtime(so_path) >= _tree_mtime(info["spec"]["path"])
        if fresh:
            built.append(so_path)
            continue
        lines = ["use pyo3::prelude::*;", ""]
        adds = []
        crate_structs = info.get("structs", {})
        crate_enums = info.get("enums", {})
        used_structs, used_enums = set(), set()
        for entry in info["fns"].values():
            if isinstance(entry, str):
                continue
            for t in list(entry[0]) + [entry[1]]:
                if t in crate_structs and crate_structs[t]["ok"] is True:
                    used_structs.add(t)
                if t in crate_enums and crate_enums[t]["ok"] is True:
                    used_enums.add(t)
        # Nested twins ride along: In-structs and unpackers recurse,
        # so every struct a used struct's fields reach is emitted too.
        grow = True
        while grow:
            grow = False
            for t in list(used_structs):
                for _f, fty, _rf, _w in crate_structs[t]["fields"]:
                    if fty in crate_structs and fty not in used_structs:
                        used_structs.add(fty)
                        grow = True
        for sname in sorted(used_structs):
            cs = crate_structs[sname]
            fields = ", ".join(
                f"{f}: In{ty}" if ty in crate_structs else f"{f}: {_RUST_OWN[ty]}"
                for f, ty, _rf, _w in cs["fields"]
            )
            lines += [
                "// Attribute extraction accepts the app's @value twin",
                "// (or anything with these fields) as the argument.",
                "#[derive(FromPyObject)]",
                f"struct In{sname} {{ {fields} }}",
                "",
            ]
        for ename in sorted(used_enums):
            ce = crate_enums[ename]
            arms_in = "".join(
                f'        "{v}" => Ok(::{ce["rust"]}::{v}),\n' for v in ce["variants"]
            )
            arms_out = "".join(
                f'        ::{ce["rust"]}::{v} => "{v}",\n' for v in ce["variants"]
            )
            lines += [
                "// Enums cross by variant NAME — the twin rule makes the",
                "// Python member names identical to the Rust ones.",
                f"fn in_{ename}(v: &Bound<'_, PyAny>) -> PyResult<::{ce['rust']}> {{",
                '    let n: String = v.getattr("name")?.extract()?;',
                "    match n.as_str() {",
                arms_in.rstrip("\n"),
                "        _ => Err(pyo3::exceptions::PyValueError::new_err(format!(",
                f'            "{ename} has no variant {{n}}"',
                "        ))),",
                "    }",
                "}",
                "",
                f"fn out_{ename}(v: ::{ce['rust']}) -> &'static str {{",
                "    match v {",
                arms_out.rstrip("\n"),
                "    }",
                "}",
                "",
            ]

        def struct_build(t, src):
            cs = crate_structs[t]
            parts = []
            for f, ty, rf, w in cs["fields"]:
                if ty in crate_structs:
                    parts.append(f"{rf}: {struct_build(ty, f'{src}.{f}')}")
                    continue
                cast = f" as {w}" if w else ""
                parts.append(f"{rf}: {src}.{f}{cast}")
            return "::" + cs["rust"] + " { " + ", ".join(parts) + " }"

        def struct_unpack(t, src):
            cs = crate_structs[t]
            parts = []
            for _f, ty, rf, w in cs["fields"]:
                if ty in crate_structs:
                    parts.append(struct_unpack(ty, f"{src}.{rf}"))
                    continue
                cast = f" as {_RUST_OWN[ty]}" if w else ""
                parts.append(f"{src}.{rf}{cast}")
            return "(" + ", ".join(parts) + ")"

        def struct_tuple_ty(t):
            cs = crate_structs[t]
            parts = [
                struct_tuple_ty(ty) if ty in crate_structs else _RUST_OWN[ty]
                for _f, ty, _rf, _w in cs["fields"]
            ]
            return "(" + ", ".join(parts) + ")"

        ret_structs, ret_enums = {}, {}
        for fname, entry in sorted(info["fns"].items()):
            if isinstance(entry, str):
                continue
            params, ret, _rpi_name, rust_path, fallible = entry
            needs_result = fallible or any(t in used_enums for t in params)
            sig_parts, arg_parts = [], []
            for i, t in enumerate(params):
                if t in used_structs:
                    sig_parts.append(f"a{i}: In{t}")
                    arg_parts.append(struct_build(t, f"a{i}"))
                elif t in used_enums:
                    sig_parts.append(f"a{i}: &Bound<'_, PyAny>")
                    arg_parts.append(f"in_{t}(a{i})?")
                else:
                    sig_parts.append(f"a{i}: {_shim_sig(t, owned=False)}")
                    arg_parts.append(f"a{i}")
            sig = ", ".join(sig_parts)
            args = ", ".join(arg_parts)
            call = f"::{rust_path}({args})"

            if ret in used_structs:
                out_ty = struct_tuple_ty(ret)
                conv = lambda v: struct_unpack(ret, v)
                ret_structs[fname] = ret
            elif ret in used_enums:
                out_ty = "String"
                conv = lambda v: f"out_{ret}({v}).to_string()"
                ret_enums[fname] = ret
            elif ret.startswith("Map<"):
                out_ty = _shim_sig(ret, owned=True)
                conv = lambda v: f"{v}.into_iter().collect()"
            else:
                out_ty = "()" if ret == "Unit" else _shim_sig(ret, owned=True)
                conv = None

            if fallible:
                body = [
                    f"    let __r = {call}",
                    '        .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(format!("{e}")))?;',
                    f"    Ok({conv('__r') if conv else '__r'})",
                ]
                lines += [
                    "#[pyfunction]",
                    f'#[pyo3(name = "{fname}")]',
                    f"fn py_{fname}({sig}) -> pyo3::PyResult<{out_ty}> {{",
                    *body,
                    "}",
                    "",
                ]
            elif needs_result:
                lines += [
                    "#[pyfunction]",
                    f'#[pyo3(name = "{fname}")]',
                    f"fn py_{fname}({sig}) -> pyo3::PyResult<{out_ty}> {{",
                    f"    let __r = {call};",
                    f"    Ok({conv('__r') if conv else '__r'})",
                    "}",
                    "",
                ]
            elif conv is not None:
                lines += [
                    "#[pyfunction]",
                    f'#[pyo3(name = "{fname}")]',
                    f"fn py_{fname}({sig}) -> {out_ty} {{",
                    f"    let __r = {call};",
                    f"    {conv('__r')}",
                    "}",
                    "",
                ]
            else:
                lines += [
                    "#[pyfunction]",
                    f'#[pyo3(name = "{fname}")]',
                    f"fn py_{fname}({sig}) -> {out_ty} {{",
                    f"    {call}",
                    "}",
                    "",
                ]
            adds.append(f"    m.add_function(wrap_pyfunction!(py_{fname}, m)?)?;")
        lines += [
            "#[pymodule]",
            f"fn {modname}(m: &Bound<'_, PyModule>) -> PyResult<()> {{",
            *adds,
            "    Ok(())",
            "}",
        ]
        open(os.path.join(src_dir, "src", "lib.rs"), "w").write("\n".join(lines) + "\n")
        open(os.path.join(src_dir, "build.rs"), "w").write(
            "fn main() {\n"
            "    // macOS: CPython symbols stay undefined in extension\n"
            "    // modules and resolve from the host process at import.\n"
            "    let target = std::env::var(\"TARGET\").unwrap_or_default();\n"
            "    if target.contains(\"apple\") {\n"
            "        println!(\"cargo:rustc-cdylib-link-arg=-undefined\");\n"
            "        println!(\"cargo:rustc-cdylib-link-arg=dynamic_lookup\");\n"
            "    }\n"
            "}\n"
        )
        open(os.path.join(src_dir, "Cargo.toml"), "w").write(
            f'[package]\nname = "{modname}"\nversion = "0.1.0"\nedition = "2024"\n\n'
            f'[lib]\ncrate-type = ["cdylib"]\nname = "{modname}"\n\n'
            f"[dependencies]\n{_crate_dep_spec(name, info['spec'])}\n"
            f'pyo3 = {{ version = "0.26", features = ["extension-module", "abi3-py312"] }}\n\n'
            f"[workspace]\n"
        )
        tc = os.path.join(repo(), "rust-toolchain.toml")
        if os.path.isfile(tc):
            shutil.copyfile(tc, os.path.join(src_dir, "rust-toolchain.toml"))
        env = dict(os.environ)
        env.pop("RUSTUP_TOOLCHAIN", None)
        r = subprocess.run(
            ["cargo", "build", "--release", "--quiet"],
            cwd=src_dir, env=env, capture_output=True, text=True,
        )
        if r.returncode != 0:
            raise ValueError(f"`{name}`: the interpreted-run door failed to build —\n{r.stderr.strip()}")
        target = env.get("CARGO_TARGET_DIR") or os.path.join(src_dir, "target")
        dylib = os.path.join(target, "release", f"lib{modname}.dylib")
        if not os.path.exists(dylib):
            dylib = os.path.join(target, "release", f"lib{modname}.so")
        shutil.copyfile(dylib, so_path)
        if sys.platform == "darwin":
            subprocess.run(["codesign", "--force", "-s", "-", so_path], capture_output=True)
        import json as _json

        meta_path = so_path + ".meta.json"
        if ret_structs or ret_enums:
            open(meta_path, "w").write(
                _json.dumps({"ret_structs": ret_structs, "ret_enums": ret_enums})
            )
        elif os.path.exists(meta_path):
            os.remove(meta_path)
        built.append(so_path)
    return built


def translate_file(path: str) -> tuple[str, "Translator"]:
    modules = _local_modules(path)   # parses (and stamps) the entry too
    tr = Translator(parse_source(path), modules)
    decls = app_crate_decls(path)
    if decls:
        tr.crate_info = prepare_crates(path, decls)
    return tr.translate(), tr


def emit_project(gate_dir: str, stem: str, pix: str, tr: "Translator") -> str:
    """@ui.py escapes: emit a pixie PROJECT — main.pix + a generated
    pyo3-embedding crate bound through [crates], with the .rpi cache
    pre-written so the build needs no rustdoc pass."""
    proj = os.path.join(gate_dir, stem)
    os.makedirs(os.path.join(proj, "src"), exist_ok=True)
    os.makedirs(os.path.join(proj, ".pixie", "rpi"), exist_ok=True)
    open(os.path.join(proj, "src", "main.pix"), "w").write(pix)
    crates = []
    if tr.escapes:
        os.makedirs(os.path.join(proj, "escapes", "src"), exist_ok=True)
        crates.append('escapes = { path = "escapes" }')
    # The gate owns the generated project's binding cache: scrub any
    # .rpi it did not write THIS run, or renamed bindings pile up as
    # duplicate declarations (three generations of the stdlib binding
    # once coexisted — 132 duplicate-method errors).
    rpi_dir = os.path.join(proj, ".pixie", "rpi")
    if os.path.isdir(rpi_dir):
        for f in os.listdir(rpi_dir):
            if f.endswith(".rpi"):
                os.remove(os.path.join(rpi_dir, f))
    if tr.uses_crates:
        app_rpi = os.path.join(os.path.dirname(gate_dir), ".yokan", "rpi")
        for cname in sorted(tr.uses_crates):
            cinfo = tr.crate_info[cname]
            crates.append(_crate_dep_spec(cname, cinfo["spec"]))
            shutil.copyfile(
                cinfo["rpi"], os.path.join(proj, ".pixie", "rpi", f"{cname}.rpi")
            )
    if tr.uses_stdlib:
        stdlib_dir = os.path.join(repo(), "crates", "yokan-stdlib")
        crates.append(f'yokan-stdlib = {{ path = "{stdlib_dir}" }}')
        open(os.path.join(proj, ".pixie", "rpi", "yokan-stdlib.rpi"), "w").write(
            "# pixie-cache-key: version=0.1.0; features=; format=61\n"
            "# Generated by yokan_gate — the standard library binding (one crate, both runs).\n\n"
            "class Fs {\n"
            '  static fn readText(path: String) String @rust("yokan_stdlib::fs_read_text")\n'
            '  static fn tryReadText(path: String) !String @rust("yokan_stdlib::fs_read_text_result")\n'
            '  static fn writeText(path: String, text: String) Int @rust("yokan_stdlib::fs_write_text")\n'
            '  static fn exists(path: String) Bool @rust("yokan_stdlib::fs_exists")\n'
            '  static fn readTextOr(path: String, default: String) String @rust("yokan_stdlib::fs_read_text_or")\n'
            "}\n"
            "\n"
            "class Sqlite {\n"
            '  static fn exec(path: String, sql: String) Int @rust("yokan_stdlib::sqlite_exec")\n'
            '  static fn queryText(path: String, sql: String) List<String> @rust("yokan_stdlib::sqlite_query_text")\n'
            '  static fn queryInt(path: String, sql: String) Int @rust("yokan_stdlib::sqlite_query_int")\n'
            '  static fn tryQueryInt(path: String, sql: String) !Int @rust("yokan_stdlib::sqlite_query_int_result")\n'
            '  static fn queryIntOr(path: String, sql: String, default: Int) Int @rust("yokan_stdlib::sqlite_query_int_or")\n'
            '  static fn queryTextOr(path: String, sql: String) List<String> @rust("yokan_stdlib::sqlite_query_text_or")\n'
            "}\n"
            "\n"
            "class Notify {\n"
            '  static fn send(title: String, body: String) @rust("yokan_stdlib::notify_send")\n'
            "}\n"
            "\n"
            "class Http {\n"
            '  static fn getText(url: String) String @rust("yokan_stdlib::http_get_text")\n'
            '  static fn tryGetText(url: String) !String @rust("yokan_stdlib::http_get_text_result")\n'
            '  static fn getTextOr(url: String, default: String) String @rust("yokan_stdlib::http_get_text_or")\n'
            "}\n"
            "\n"
            "class Math {\n"
            '  static fn sqrt(v: Float) Float @rust("yokan_stdlib::math_sqrt")\n'
            '  static fn sin(v: Float) Float @rust("yokan_stdlib::math_sin")\n'
            '  static fn cos(v: Float) Float @rust("yokan_stdlib::math_cos")\n'
            '  static fn pow_(a: Float, b: Float) Float @rust("yokan_stdlib::math_pow")\n'
            '  static fn fabs(v: Float) Float @rust("yokan_stdlib::math_fabs")\n'
            '  static fn floor(v: Float) Int @rust("yokan_stdlib::math_floor")\n'
            '  static fn ceil(v: Float) Int @rust("yokan_stdlib::math_ceil")\n'
            '  static fn pi() Float @rust("yokan_stdlib::math_pi")\n'
            "}\n"
            "\n"
            "class Json {\n"
            '  static fn getText(src: String, path: String) String @rust("yokan_stdlib::json_get_text")\n'
            '  static fn getInt(src: String, path: String) Int @rust("yokan_stdlib::json_get_int")\n'
            '  static fn getFloat(src: String, path: String) Float @rust("yokan_stdlib::json_get_float")\n'
            '  static fn getBool(src: String, path: String) Bool @rust("yokan_stdlib::json_get_bool")\n'
            '  static fn length(src: String, path: String) Int @rust("yokan_stdlib::json_length")\n'
            '  static fn has(src: String, path: String) Bool @rust("yokan_stdlib::json_has")\n'
            "}\n"
            "\n"
            "class Strings {\n"
            '  static fn toInt(s: String, default: Int) Int @rust("yokan_stdlib::strings_to_int")\n'
            '  static fn toFloat(s: String, default: Float) Float @rust("yokan_stdlib::strings_to_float")\n'
            "}\n"
            "\n"
            "class Random {\n"
            '  static fn seed(n: Int) Int @rust("yokan_stdlib::random_seed_ret")\n'
            '  static fn int_(lo: Int, hi: Int) Int @rust("yokan_stdlib::random_int")\n'
            '  static fn float_() Float @rust("yokan_stdlib::random_float")\n'
            "}\n"
            "\n"
            "class Time {\n"
            '  static fn nowMs() Int @rust("yokan_stdlib::time_now_ms")\n'
            '  static fn formatMs(ms: Int, fmt: String) String @rust("yokan_stdlib::time_format_ms")\n'
            '  static fn sleepMs(ms: Int) Int @rust("yokan_stdlib::time_sleep_ms")\n'
            "}\n"
            "\n"
            "class Py {\n"
            '  static fn floatRepr(v: Float) String @rust("yokan_stdlib::py_float_repr")\n'
            '  static fn boolRepr(v: Bool) String @rust("yokan_stdlib::py_bool_repr")\n'
            '  static fn truedivInt(a: Int, b: Int) Float @rust("yokan_stdlib::py_truediv_int")\n'
            '  static fn truedivFloat(a: Float, b: Float) Float @rust("yokan_stdlib::py_truediv_float")\n'
            '  static fn floordivInt(a: Int, b: Int) Int @rust("yokan_stdlib::py_floordiv_int")\n'
            '  static fn floordivFloat(a: Float, b: Float) Float @rust("yokan_stdlib::py_floordiv_float")\n'
            '  static fn modInt(a: Int, b: Int) Int @rust("yokan_stdlib::py_mod_int")\n'
            '  static fn modFloat(a: Float, b: Float) Float @rust("yokan_stdlib::py_mod_float")\n'
            '  static fn powInt(a: Int, b: Int) Int @rust("yokan_stdlib::py_pow_int")\n'
            '  static fn powFloat(a: Float, b: Float) Float @rust("yokan_stdlib::py_pow_float")\n'
            '  static fn strLen(s: String) Int @rust("yokan_stdlib::py_str_len")\n'
            '  static fn strIndex(s: String, i: Int) String @rust("yokan_stdlib::py_str_index")\n'
            '  static fn strSlice(s: String, a: Int, b: Int) String @rust("yokan_stdlib::py_str_slice")\n'
            '  static fn strUpper(s: String) String @rust("yokan_stdlib::py_str_upper")\n'
            '  static fn strLower(s: String) String @rust("yokan_stdlib::py_str_lower")\n'
            '  static fn strStrip(s: String) String @rust("yokan_stdlib::py_str_strip")\n'
            '  static fn strLstrip(s: String) String @rust("yokan_stdlib::py_str_lstrip")\n'
            '  static fn strRstrip(s: String) String @rust("yokan_stdlib::py_str_rstrip")\n'
            '  static fn strSplit(s: String, sep: String) List<String> @rust("yokan_stdlib::py_str_split")\n'
            '  static fn strSplitWs(s: String) List<String> @rust("yokan_stdlib::py_str_split_ws")\n'
            '  static fn strJoin(sep: String, parts: List<String>) String @rust("yokan_stdlib::py_str_join")\n'
            '  static fn strStartswith(s: String, p: String) Bool @rust("yokan_stdlib::py_str_startswith")\n'
            '  static fn strEndswith(s: String, p: String) Bool @rust("yokan_stdlib::py_str_endswith")\n'
            '  static fn strContains(s: String, p: String) Bool @rust("yokan_stdlib::py_str_contains")\n'
            '  static fn strReplace(s: String, from: String, to: String) String @rust("yokan_stdlib::py_str_replace")\n'
            '  static fn strFind(s: String, p: String) Int @rust("yokan_stdlib::py_str_find")\n'
            '  static fn strCount(s: String, p: String) Int @rust("yokan_stdlib::py_str_count")\n'
            '  static fn intOfStr(s: String) Int @rust("yokan_stdlib::py_int_of_str")\n'
            '  static fn floatOfStr(s: String) Float @rust("yokan_stdlib::py_float_of_str")\n'
            '  static fn floatOfInt(v: Int) Float @rust("yokan_stdlib::py_float_of_int")\n'
            '  static fn intOfFloat(v: Float) Int @rust("yokan_stdlib::py_int_of_float")\n'
            '  static fn round(v: Float) Int @rust("yokan_stdlib::py_round")\n'
            '  static fn formatInt(v: Int, spec: String) String @rust("yokan_stdlib::py_format_int")\n'
            '  static fn formatFloat(v: Float, spec: String) String @rust("yokan_stdlib::py_format_float")\n'
            '  static fn formatStr(v: String, spec: String) String @rust("yokan_stdlib::py_format_str")\n'
            '  static fn log(msg: String) Int @rust("yokan_stdlib::log_line")\n'
            '  static fn abort(msg: String) Int @rust("yokan_stdlib::py_abort")\n'
            '  static fn listContainsStr(xs: List<String>, v: String) Bool @rust("yokan_stdlib::py_list_contains_str")\n'
            '  static fn listSliceStr(xs: List<String>, a: Int, b: Int) List<String> @rust("yokan_stdlib::py_list_slice_str")\n'
            '  static fn listConcatStr(a: List<String>, b: List<String>) List<String> @rust("yokan_stdlib::py_list_concat_str")\n'
            '  static fn listReversedStr(xs: List<String>) List<String> @rust("yokan_stdlib::py_list_reversed_str")\n'
            '  static fn listContainsInt(xs: List<Int>, v: Int) Bool @rust("yokan_stdlib::py_list_contains_int")\n'
            '  static fn listSliceInt(xs: List<Int>, a: Int, b: Int) List<Int> @rust("yokan_stdlib::py_list_slice_int")\n'
            '  static fn listConcatInt(a: List<Int>, b: List<Int>) List<Int> @rust("yokan_stdlib::py_list_concat_int")\n'
            '  static fn listReversedInt(xs: List<Int>) List<Int> @rust("yokan_stdlib::py_list_reversed_int")\n'
            '  static fn listContainsFloat(xs: List<Float>, v: Float) Bool @rust("yokan_stdlib::py_list_contains_float")\n'
            '  static fn listSliceFloat(xs: List<Float>, a: Int, b: Int) List<Float> @rust("yokan_stdlib::py_list_slice_float")\n'
            '  static fn listConcatFloat(a: List<Float>, b: List<Float>) List<Float> @rust("yokan_stdlib::py_list_concat_float")\n'
            '  static fn listReversedFloat(xs: List<Float>) List<Float> @rust("yokan_stdlib::py_list_reversed_float")\n'
            '  static fn listContainsBool(xs: List<Bool>, v: Bool) Bool @rust("yokan_stdlib::py_list_contains_bool")\n'
            '  static fn listSliceBool(xs: List<Bool>, a: Int, b: Int) List<Bool> @rust("yokan_stdlib::py_list_slice_bool")\n'
            '  static fn listConcatBool(a: List<Bool>, b: List<Bool>) List<Bool> @rust("yokan_stdlib::py_list_concat_bool")\n'
            '  static fn listReversedBool(xs: List<Bool>) List<Bool> @rust("yokan_stdlib::py_list_reversed_bool")\n'
            '  static fn listSortedStr(xs: List<String>) List<String> @rust("yokan_stdlib::py_list_sorted_str")\n'
            '  static fn listSortedInt(xs: List<Int>) List<Int> @rust("yokan_stdlib::py_list_sorted_int")\n'
            '  static fn listSortedFloat(xs: List<Float>) List<Float> @rust("yokan_stdlib::py_list_sorted_float")\n'
            '  static fn listMinInt(xs: List<Int>) Int @rust("yokan_stdlib::py_list_min_int")\n'
            '  static fn listMaxInt(xs: List<Int>) Int @rust("yokan_stdlib::py_list_max_int")\n'
            '  static fn listSumInt(xs: List<Int>) Int @rust("yokan_stdlib::py_list_sum_int")\n'
            '  static fn minInt(a: Int, b: Int) Int @rust("yokan_stdlib::py_min2_int")\n'
            '  static fn maxInt(a: Int, b: Int) Int @rust("yokan_stdlib::py_max2_int")\n'
            '  static fn absInt(v: Int) Int @rust("yokan_stdlib::py_abs_int")\n'
            '  static fn listMinFloat(xs: List<Float>) Float @rust("yokan_stdlib::py_list_min_float")\n'
            '  static fn listMaxFloat(xs: List<Float>) Float @rust("yokan_stdlib::py_list_max_float")\n'
            '  static fn listSumFloat(xs: List<Float>) Float @rust("yokan_stdlib::py_list_sum_float")\n'
            '  static fn minFloat(a: Float, b: Float) Float @rust("yokan_stdlib::py_min2_float")\n'
            '  static fn maxFloat(a: Float, b: Float) Float @rust("yokan_stdlib::py_max2_float")\n'
            '  static fn absFloat(v: Float) Float @rust("yokan_stdlib::py_abs_float")\n'
            "}\n"
        )
    window = ""
    if tr.window:
        window = "\n[window]\n"
        if "title" in tr.window:
            window += f'title = "{tr.window["title"]}"\n'
        if "width" in tr.window:
            window += f'width = {tr.window["width"]}\nheight = {tr.window["height"]}\n'
    open(os.path.join(proj, "pixie.toml"), "w").write(
        f'[package]\nname = "{stem}"\nversion = "0.1.0"\n{window}\n[crates]\n' + "\n".join(crates) + "\n"
    )
    if not tr.escapes:
        return proj
    rpi = [
        "# pixie-cache-key: version=0.1.0; features=; format=61",
        "# Generated by yokan_gate — python escapes.",
        "",
        "class Escapes {",
    ]
    for name, e in sorted(tr.escapes.items()):
        args = ", ".join(f"{n}: {t[0]}" for n, t in e["params"])
        # A dict crosses as std's HashMap on the Rust side; `stdmap:`
        # is how a binding says so (the same marker rpi-gen writes).
        mark = "stdmap:" if any(
            t[0].startswith("Map<") for _n, t in e["params"]
        ) or e["ret"][0].startswith("Map<") else ""
        rpi.append(f'  static fn {tr._camel(name)}({args}) {e["ret"][0]} @rust("{mark}escapes::{name}")')
        rpi.append(
            f'  static fn try{tr._camel(name)[0].upper()}{tr._camel(name)[1:]}({args}) !{e["ret"][0]} @rust("{mark}escapes::try_{name}")'
        )
    for wr in tr.try_wrappers:
        k = wr["k"]
        esc = tr.escapes[wr["escape"]]
        args = ", ".join(f"{n}: {t[0]}" for n, t in esc["params"])
        rpi.append(f'  static fn try{k}({args}) TryRes{k} @rust("escapes::try{k}")')
    rpi.append("}")
    for sname in sorted(tr.escape_structs):
        rpi.append("")
        rpi.append(f'struct {sname} @rust("escapes::{sname}") {{')
        for f, t, _d in tr.structs[sname]:
            rpi.append(f"  var {f} : {t}")
        rpi.append("}")
    for wr in tr.try_wrappers:
        k = wr["k"]
        esc = tr.escapes[wr["escape"]]
        rpi.append("")
        rpi.append(f'struct TryRes{k} @rust("escapes::TryRes{k}") {{')
        rpi.append("  var tag : Int")
        rpi.append(f"  var value : {esc['ret'][0]}")
        rpi.append("  var msg : String")
        rpi.append("}")
    open(os.path.join(proj, ".pixie", "rpi", "escapes.rpi"), "w").write("\n".join(rpi) + "\n")
    open(os.path.join(proj, "escapes", "Cargo.toml"), "w").write(
        '[package]\nname = "escapes"\nversion = "0.1.0"\nedition = "2024"\n\n'
        '[dependencies]\npyo3 = { version = "0.26", features = ["auto-initialize"] }\n\n[workspace]\n'
    )
    dataclasses_py = []
    for sname in sorted(tr.escape_structs):
        dataclasses_py.append("@dataclass(frozen=True)")
        dataclasses_py.append(f"class {sname}:")
        for f, t, _d in tr.structs[sname]:
            dataclasses_py.append(f"    {f}: {py_ty(t)}")
        dataclasses_py.append("")
    src_py = "\n\n".join(e["src"] for _, e in sorted(tr.escapes.items()))
    if dataclasses_py:
        src_py = "from dataclasses import dataclass\n\n" + "\n".join(dataclasses_py) + "\n\n" + src_py
    for wr in tr.try_wrappers:
        k = wr["k"]
        esc = tr.escapes[wr["escape"]]
        pnames = ", ".join(n for n, _ in esc["params"])
        w = [f"def __try{k}({pnames}):", "    try:"]
        w.append(f'        return {{"tag": 0, "value": {wr["escape"]}({pnames}), "msg": ""}}')
        for i, tnames in enumerate(wr["clauses"]):
            if tnames is None:
                w.append("    except:")
                w.append(f'        return {{"tag": {i + 1}, "value": None, "msg": ""}}')
            else:
                spec = tnames[0] if len(tnames) == 1 else "(" + ", ".join(tnames) + ")"
                w.append(f"    except {spec} as __e:")
                w.append(f'        return {{"tag": {i + 1}, "value": None, "msg": str(__e)}}')
        src_py = src_py + "\n\n" + "\n".join(w)
    if '"##' in src_py:
        sys.exit("escape source contains `\"##` — cannot embed")
    lib = [
        "//! Generated by yokan_gate: the app's @ui.py escapes, run on",
        "//! an embedded CPython. Loud on failure by design.",
        "use pyo3::prelude::*;",
        "use std::sync::OnceLock;",
        "",
        f'static SRC: &str = r##"{src_py}"##;',
        "static MODULE: OnceLock<Py<PyAny>> = OnceLock::new();",
        "",
        "/// Bundled runtime: if a `python/` tree sits next to the",
        "/// executable (the --bundle layout), point CPython at it before",
        "/// the interpreter initializes; otherwise the system rules hold.",
        "fn ensure_env() {",
        "    static ONCE: OnceLock<()> = OnceLock::new();",
        "    ONCE.get_or_init(|| {",
        "        if let Ok(exe) = std::env::current_exe() {",
        "            if let Some(dir) = exe.parent() {",
        '                let home = dir.join("python");',
        '                if home.join("lib").is_dir() {',
        "                    unsafe {",
        '                        std::env::set_var("PYTHONHOME", &home);',
        '                        std::env::set_var("PYTHONNOUSERSITE", "1");',
        "                    }",
        "                }",
        "            }",
        "        }",
        "    });",
        "}",
        "",
        "fn module(py: Python<'_>) -> &'static Py<PyAny> {",
        "    ensure_env();",
        "    MODULE.get_or_init(|| {",
        '        let c = std::ffi::CString::new(SRC).expect("src");',
        '        let f = std::ffi::CString::new("escapes.py").expect("name");',
        '        let m = std::ffi::CString::new("pixie_escapes").expect("mod");',
        "        pyo3::types::PyModule::from_code(py, &c, &f, &m)",
        "            .unwrap_or_else(|e| {",
        "                e.print(py);",
        '                panic!("python escape module failed to load")',
        "            })",
        "            .into_any()",
        "            .unbind()",
        "    })",
        "}",
        "",
    ]
    for name, e in sorted(tr.escapes.items()):
        args_sig = ", ".join(f"{n}: {t[1]}" for n, t in e["params"])
        if e["params"]:
            names = ", ".join(n for n, _ in e["params"])
            call = f"f.call1(({names},))"
        else:
            call = "f.call0()"
        lib += [
            f"pub fn {name}({args_sig}) -> {e['ret'][1]} {{",
            "    Python::attach(|py| {",
            "        module(py)",
            "            .bind(py)",
            f'            .getattr("{name}")',
            f"            .and_then(|f| {call})",
            "            .and_then(|v| v.extract())",
            "            .unwrap_or_else(|e| {",
            "                e.print(py);",
            f'                panic!("python escape `{name}` failed")',
            "            })",
            "    })",
            "}",
            "",
            f"pub fn try_{name}({args_sig}) -> std::io::Result<{e['ret'][1]}> {{",
            "    Python::attach(|py| {",
            "        module(py)",
            "            .bind(py)",
            f'            .getattr("{name}")',
            f"            .and_then(|f| {call})",
            "            .and_then(|v| v.extract())",
            "            .map_err(|e| {",
            "                let msg = e",
            "                    .value(py)",
            "                    .str()",
            "                    .map(|s| s.to_string())",
            '                    .unwrap_or_else(|_| "python exception".to_string());',
            "                std::io::Error::other(msg)",
            "            })",
            "    })",
            "}",
            "",
        ]
    for sname in sorted(tr.escape_structs):
        fields = tr.structs[sname]
        rustty = {"Int": "i64", "Float": "f64", "String": "String", "Bool": "bool"}
        lib += [
            "// A value class crossing @py: the struct the compiled side",
            "// passes, converted field by field. The escape module holds",
            "// a dataclass of the same shape for the Python side.",
            "#[derive(Clone, Default)]",
            f"pub struct {sname} {{",
        ]
        lib += [f"    pub {f}: {rustty[t]}," for f, t, _d in fields]
        lib += [
            "}",
            "",
            f"impl<'py> pyo3::FromPyObject<'py> for {sname} {{",
            "    fn extract_bound(ob: &pyo3::Bound<'py, pyo3::PyAny>) -> PyResult<Self> {",
            f"        Ok({sname} {{",
        ]
        lib += [f'            {f}: ob.getattr("{f}")?.extract()?,' for f, _t, _d in fields]
        lib += [
            "        })",
            "    }",
            "}",
            "",
            f"impl<'py> pyo3::IntoPyObject<'py> for {sname} {{",
            "    type Target = pyo3::PyAny;",
            "    type Output = pyo3::Bound<'py, pyo3::PyAny>;",
            "    type Error = pyo3::PyErr;",
            "    fn into_pyobject(self, py: pyo3::Python<'py>) -> Result<Self::Output, Self::Error> {",
            f'        let cls = module(py).bind(py).getattr("{sname}")?;',
            "        cls.call1((" + ", ".join(f"self.{f}" for f, _t, _d in fields) + ",))",
            "    }",
            "}",
            "",
        ]
    for wr in tr.try_wrappers:
        k = wr["k"]
        esc = tr.escapes[wr["escape"]]
        args_sig = ", ".join(f"{n}: {t[1]}" for n, t in esc["params"])
        if esc["params"]:
            names = ", ".join(n for n, _ in esc["params"])
            call = f"f.call1(({names},))"
        else:
            call = "f.call0()"
        rust_t = esc["ret"][1]
        lib += [
            "#[derive(Clone)]",
            f"pub struct TryRes{k} {{",
            "    pub tag: i64,",
            f"    pub value: {rust_t},",
            "    pub msg: String,",
            "}",
            "",
            f"pub fn try{k}({args_sig}) -> TryRes{k} {{",
            "    Python::attach(|py| {",
            "        let d = module(py)",
            "            .bind(py)",
            f'            .getattr("__try{k}")',
            f"            .and_then(|f| {call})",
            "            .unwrap_or_else(|e| {",
            "                e.print(py);",
            f'                panic!("python escape dispatch `__try{k}` failed")',
            "            });",
            '        let tag: i64 = d.get_item("tag").and_then(|v| v.extract()).unwrap_or_else(|e| {{ e.print(py); panic!("__try dispatch shape") }});',
            '        let msg: String = d.get_item("msg").and_then(|v| v.extract()).unwrap_or_default();',
            f"        let value: {rust_t} = if tag == 0 {{",
            '            d.get_item("value").and_then(|v| v.extract()).unwrap_or_else(|e| {{ e.print(py); panic!("__try dispatch value") }})',
            "        } else {",
            "            Default::default()",
            "        };",
            f"        TryRes{k} {{ tag, value, msg }}",
            "    })",
            "}",
            "",
        ]
    open(os.path.join(proj, "escapes", "src", "lib.rs"), "w").write("\n".join(lib))
    return proj


def tier_b_project(
    proj: str, script: str, release: bool, pyo3_python: str | None = None,
    run: bool = True,
) -> tuple[str, str]:
    cmd = [
        "cargo", "run", "-q", "--manifest-path", os.path.join(repo(), "Cargo.toml"),
        "-p", "pixie-cli", "--", "build",
    ]
    if release:
        cmd.append("--release")
    env = dict(os.environ)
    if pyo3_python:
        env["PYO3_PYTHON"] = pyo3_python
    p = subprocess.run(cmd, cwd=proj, capture_output=True, text=True, env=env)
    if p.returncode != 0:
        sys.exit(f"pixie build (project) failed:\n{p.stdout}\n{p.stderr}")
    binary = None
    for line in p.stdout.splitlines():
        if line.startswith("built: "):
            binary = line[len("built: "):].strip()
    if not binary:
        sys.exit(f"could not find `built:` line in pixie output:\n{p.stdout}")
    if not run:
        return "", binary
    env = dict(os.environ, PIXIE_SCRIPT=script)
    r = subprocess.run([binary], env=env, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"compiled tier failed:\n{r.stdout}\n{r.stderr}")
    return r.stdout, binary


def app_deps(path: str) -> list[str]:
    """The app's declared `dependencies` — the bundle's source of
    truth for third-party packages the escapes import. A PEP 723
    inline block wins (script-style app); otherwise the NEAREST
    pyproject.toml upward from the app supplies `[project]
    dependencies` (project-style app) — uv's own precedence.
    yokan itself is filtered out: an app project rightly depends
    on it, but the shipped runtime must not try to install it."""
    import tomllib

    def clean(deps) -> list[str]:
        out = []
        for d in deps:
            m = re.match(r"[A-Za-z0-9._-]+", str(d))
            name = (m.group(0) if m else "").lower().replace("_", "-")
            if name != "yokan":
                out.append(str(d))
        return out

    src = open(path).read()
    m = re.search(r"(?ms)^# /// script\n(.*?)^# ///$", src)
    if m:
        block = "\n".join(line[2:] if line.startswith("# ") else line[1:]
                          for line in m.group(1).splitlines())
        try:
            return clean(tomllib.loads(block).get("dependencies", []))
        except tomllib.TOMLDecodeError:
            return []
    d = os.path.dirname(os.path.abspath(path))
    while True:
        pp = os.path.join(d, "pyproject.toml")
        if os.path.isfile(pp):
            try:
                with open(pp, "rb") as f:
                    return clean(tomllib.load(f).get("project", {}).get("dependencies", []))
            except tomllib.TOMLDecodeError:
                return []
        up = os.path.dirname(d)
        if up == d:
            return []
        d = up


def find_pbs() -> tuple[str, str, str]:
    """A python-build-standalone install, courtesy of uv's managed
    pythons. Returns (python_bin, install_root, "3.14")."""
    # --system skips any active/project venv (the gate itself may be
    # running under `uv run --with numpy`); the /uv/python/ guard below
    # still insists on a MANAGED install, which is what PBS is.
    env = {k: v for k, v in os.environ.items() if k != "VIRTUAL_ENV"}
    r = subprocess.run(["uv", "python", "find", "--system"], capture_output=True, text=True, env=env)
    bin_ = r.stdout.strip()
    if r.returncode != 0 or "/uv/python/" not in bin_:
        sys.exit("--bundle needs a uv-managed python (`uv python install 3.14`)")
    root = os.path.dirname(os.path.dirname(bin_))
    libs = [f for f in os.listdir(os.path.join(root, "lib")) if re.fullmatch(r"libpython3\.\d+\.dylib", f)]
    if not libs:
        sys.exit(f"no libpython in {root}/lib")
    ver = libs[0][len("libpython"):-len(".dylib")]
    return bin_, root, ver


PRUNE = {"test", "idlelib", "tkinter", "turtledemo", "ensurepip", "pydoc_data", "__pycache__"}


def bundle(proj: str, binary: str, stem: str, root: str, ver: str, deps: list[str]) -> str:
    """The app-folder layout: dist/<stem> + dist/python/{lib/libpython,
    lib/pythonX.Y stdlib}, link reference rewritten to
    @executable_path, re-signed."""
    import shutil

    dist = os.path.join(proj, "dist")
    shutil.rmtree(dist, ignore_errors=True)
    os.makedirs(os.path.join(dist, "python", "lib"), exist_ok=True)
    app = os.path.join(dist, stem)
    shutil.copy2(binary, app)
    lib = f"libpython{ver}.dylib"
    shutil.copy2(os.path.join(root, "lib", lib), os.path.join(dist, "python", "lib", lib))
    shutil.copytree(
        os.path.join(root, "lib", f"python{ver}"),
        os.path.join(dist, "python", "lib", f"python{ver}"),
        ignore=shutil.ignore_patterns(*PRUNE),
    )
    if deps:
        r = subprocess.run(
            ["uv", "pip", "install", "--quiet",
             "--python", os.path.join(root, "bin", f"python{ver}"),
             "--target", os.path.join(dist, "python", "lib", f"python{ver}", "site-packages"),
             *deps],
            capture_output=True, text=True,
        )
        if r.returncode != 0:
            sys.exit(f"bundling deps {deps} failed:\n{r.stderr}")
        # uv seeds the target with pip and a bin/ of scripts; the app
        # imports packages, it never installs or runs entry points.
        sp = os.path.join(dist, "python", "lib", f"python{ver}", "site-packages")
        for junk in os.listdir(sp):
            if junk == "bin" or junk == "pip" or junk.startswith("pip-"):
                shutil.rmtree(os.path.join(sp, junk), ignore_errors=True)
    ot = subprocess.run(["otool", "-L", app], capture_output=True, text=True).stdout
    old = None
    for line in ot.splitlines():
        line = line.strip().split(" ")[0]
        if "libpython" in line:
            old = line
    if not old:
        sys.exit(f"binary has no libpython reference:\n{ot}")
    new = f"@executable_path/python/lib/{lib}"
    subprocess.run(["install_name_tool", "-change", old, new, app], check=True)
    subprocess.run(["codesign", "--force", "-s", "-", app], check=True, capture_output=True)
    return app


def make_onefile(gate_dir_proj: str, stem: str, dist: str) -> str:
    """One distributable FILE: a small launcher with the whole app
    folder (binary + python runtime + site-packages) embedded as a
    tar.gz. First run unpacks into the user cache, keyed by content
    hash; every run execs the cached app, so the @executable_path
    link to libpython resolves exactly as in the folder layout.
    (The static-libpython pack is the far milestone — see DESIGN.)"""
    import hashlib
    import shutil
    import tarfile

    onedir = os.path.join(gate_dir_proj, "onefile")
    src = os.path.join(onedir, "src")
    os.makedirs(src, exist_ok=True)
    payload = os.path.join(src, "payload.tar.gz")
    with tarfile.open(payload, "w:gz") as t:
        t.add(dist, arcname=".")
    stamp = hashlib.sha256(open(payload, "rb").read()).hexdigest()[:16]

    open(os.path.join(onedir, "Cargo.toml"), "w").write(f"""[package]
name = "{stem}_onefile"
version = "0.1.0"
edition = "2024"

[dependencies]
tar = "0.4"
flate2 = "1"

[profile.release]
strip = true

[workspace]
""")
    open(os.path.join(src, "main.rs"), "w").write(f"""//! Generated by yokan: single-file launcher for `{stem}`.
use std::os::unix::process::CommandExt;
use std::{{env, fs, path::PathBuf, process::Command}};

static PAYLOAD: &[u8] = include_bytes!("payload.tar.gz");
const STAMP: &str = "{stamp}";
const STEM: &str = "{stem}";

fn main() {{
    let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
    let cache = if cfg!(target_os = "macos") {{ home.join("Library/Caches/yokan") }} else {{ home.join(".cache/yokan") }};
    let root = cache.join(format!("{{STEM}}-{{STAMP}}"));
    let app = root.join(STEM);
    if !app.is_file() {{
        let tmp = cache.join(format!("{{STEM}}-{{STAMP}}.tmp{{}}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).expect("create cache dir");
        tar::Archive::new(flate2::read::GzDecoder::new(PAYLOAD))
            .unpack(&tmp)
            .expect("unpack runtime");
        match fs::rename(&tmp, &root) {{
            Ok(()) => {{}}
            Err(_) if app.is_file() => {{ let _ = fs::remove_dir_all(&tmp); }}
            Err(e) => panic!("install runtime into {{}}: {{e}}", root.display()),
        }}
    }}
    let err = Command::new(&app).args(env::args_os().skip(1)).exec();
    panic!("exec {{}}: {{err}}", app.display());
}}
""")
    target = os.path.expanduser("~/.cache/pixie/target")
    r = subprocess.run(
        ["cargo", "build", "-q", "--release", "--manifest-path", os.path.join(onedir, "Cargo.toml")],
        capture_output=True, text=True, env=dict(os.environ, CARGO_TARGET_DIR=target),
    )
    if r.returncode != 0:
        sys.exit(f"onefile launcher build failed:\n{r.stderr}")
    built = os.path.join(target, "release", f"{stem}_onefile")
    out = os.path.join(onedir, stem)
    if os.path.exists(out):
        os.remove(out)
    shutil.copy2(built, out)
    return out


def describe_artifact(binary: str) -> str:
    size = os.path.getsize(binary)
    runtime = os.path.join(os.path.dirname(binary), "python")
    note = f"{size / 1e6:.1f} MB"
    if os.path.isdir(runtime):
        total = sum(
            os.path.getsize(os.path.join(d, f))
            for d, _, fs in os.walk(runtime)
            for f in fs
        )
        note += f" + bundled python runtime {total / 1e6:.0f} MB"
    return note


def run_binary(binary: str, script: str) -> str:
    env = {k: v for k, v in os.environ.items() if not k.startswith("PYTHON")}
    env["PIXIE_SCRIPT"] = script
    r = subprocess.run([binary], env=env, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"bundled binary failed:\n{r.stdout}\n{r.stderr}")
    return r.stdout


def tier_a(path: str, tr: "Translator", script: str) -> str:
    sys.path.insert(0, HERE)
    sys.path.insert(0, os.path.dirname(os.path.abspath(path)))
    import yokan  # noqa: PLC0415

    spec = importlib.util.spec_from_file_location("gate_app", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)  # not __main__, so run() is inert
    state = ast.literal_eval(tr.state_node) if tr.state_node is not None else None
    on_start = None
    if tr.on_start is not None:
        if tr.on_start[0] == "store":
            on_start = getattr(getattr(module, tr.on_start[1]), tr.on_start[2])
        else:
            on_start = getattr(module, tr.on_start[1])
    return yokan._headless(getattr(module, tr.view.name), state, script, on_start=on_start)


def tier_b(pix_path: str, script: str, release: bool, run: bool = True) -> tuple[str, str]:
    cmd = ["cargo", "run", "-q", "-p", "pixie-cli", "--", "build", pix_path]
    if release:
        cmd.append("--release")
    p = subprocess.run(cmd, cwd=repo(), capture_output=True, text=True)
    if p.returncode != 0:
        sys.exit(f"pixie build failed:\n{p.stdout}\n{p.stderr}")
    binary = None
    for line in p.stdout.splitlines():
        if line.startswith("built: "):
            binary = line[len("built: "):].strip()
    if not binary:
        sys.exit(f"could not find `built:` line in pixie output:\n{p.stdout}")
    if not run:
        return "", binary
    env = dict(os.environ, PIXIE_SCRIPT=script)
    r = subprocess.run([binary], env=env, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"compiled tier failed:\n{r.stdout}\n{r.stderr}")
    return r.stdout, binary


def _png_to_icns(png: str, out_icns: str) -> bool:
    """macOS icon pipeline: sips resizes into an .iconset, iconutil
    packs it. Returns False (no icon) if any tool balks."""
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        iconset = os.path.join(td, "app.iconset")
        os.makedirs(iconset)
        for base in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                px = base * scale
                suffix = "" if scale == 1 else "@2x"
                dest = os.path.join(iconset, f"icon_{base}x{base}{suffix}.png")
                r = subprocess.run(
                    ["sips", "-z", str(px), str(px), png, "--out", dest],
                    capture_output=True,
                )
                if r.returncode != 0:
                    return False
        r = subprocess.run(
            ["iconutil", "-c", "icns", iconset, "-o", out_icns], capture_output=True
        )
        return r.returncode == 0


def make_app_bundle(app_py: str, tr, binary: str, payload_dir: str | None = None) -> str:
    """`--app`: assemble a macOS application bundle in <app>/dist/.
    Plain builds put the lone executable in Contents/MacOS; bundle
    builds (`--bundle --app`) move the WHOLE runtime folder there,
    so the binary's relative runtime lookup keeps working unchanged.
    An icon rides along when <stem>.icns or <stem>.png sits next to
    the app. Ad-hoc signed, like every other artifact here."""
    app_dir = os.path.dirname(os.path.abspath(app_py))
    stem = os.path.splitext(os.path.basename(app_py))[0]
    title = (getattr(tr, "window", None) or {}).get("title") or stem
    safe = re.sub(r"[^A-Za-z0-9 ._-]", "", title).strip() or stem
    root = os.path.join(app_dir, "dist", f"{safe}.app")
    macos = os.path.join(root, "Contents", "MacOS")
    res = os.path.join(root, "Contents", "Resources")
    shutil.rmtree(root, ignore_errors=True)
    os.makedirs(macos)
    os.makedirs(res)

    if payload_dir:
        shutil.copytree(payload_dir, macos, dirs_exist_ok=True)
        exe_rel = os.path.relpath(binary, payload_dir)
    else:
        exe_rel = stem
        shutil.copy2(binary, os.path.join(macos, exe_rel))

    icon_line = ""
    for cand, ready in ((os.path.join(app_dir, stem + ".icns"), True),
                        (os.path.join(app_dir, stem + ".png"), False)):
        if os.path.isfile(cand):
            dest = os.path.join(res, "icon.icns")
            ok = shutil.copyfile(cand, dest) or True if ready else _png_to_icns(cand, dest)
            if ok:
                icon_line = "  <key>CFBundleIconFile</key>\n  <string>icon</string>\n"
            break

    open(os.path.join(root, "Contents", "Info.plist"), "w").write(
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"'
        ' "http://www.apple.com/DTDs/PropertyList-1.0.dtd">\n'
        '<plist version="1.0">\n<dict>\n'
        f"  <key>CFBundleName</key>\n  <string>{title}</string>\n"
        f"  <key>CFBundleDisplayName</key>\n  <string>{title}</string>\n"
        f"  <key>CFBundleExecutable</key>\n  <string>{exe_rel}</string>\n"
        f"  <key>CFBundleIdentifier</key>\n  <string>com.yokan.{stem}</string>\n"
        "  <key>CFBundlePackageType</key>\n  <string>APPL</string>\n"
        "  <key>CFBundleShortVersionString</key>\n  <string>0.1.0</string>\n"
        "  <key>CFBundleVersion</key>\n  <string>0.1.0</string>\n"
        "  <key>LSMinimumSystemVersion</key>\n  <string>11.0</string>\n"
        "  <key>NSHighResolutionCapable</key>\n  <true/>\n"
        f"{icon_line}"
        "</dict>\n</plist>\n"
    )
    if sys.platform == "darwin":
        subprocess.run(["codesign", "--force", "-s", "-", root], capture_output=True)
    return root


def do_add(args) -> None:
    """`yokan add <app.py> <crate> [VERSION] [--path DIR]
    [--features a,b]` — declare the crate where the app's crates
    live (the PEP 723 block, else the nearest pyproject.toml, else a
    fresh PEP 723 block), then derive the binding and build the
    interpreted-run door immediately. Rolls the file back if any
    step fails."""
    if not args.extra:
        sys.exit("add wants: yokan add <app.py> <crate> [VERSION] [--path DIR] [--features a,b]")
    name = args.extra[0]
    version = args.extra[1] if len(args.extra) > 1 else None
    if not re.fullmatch(r"[a-z][a-z0-9_-]*", name):
        sys.exit(f"`{name}`: crate names are lowercase")
    if args.path and version:
        sys.exit("give a VERSION or --path, not both")
    if not args.path and not version:
        sys.exit(f"how should `{name}` resolve? give a VERSION (crates.io) or --path <dir>")
    feats = [f for f in args.features.split(",") if f]
    if app_crate_decls(args.app).get(name):
        sys.exit(f"`{name}` is already declared")

    app_path = os.path.abspath(args.app)
    app_dir = os.path.dirname(app_path)

    def spec_line(base_dir: str) -> str:
        parts = []
        if args.path:
            rel = os.path.relpath(os.path.abspath(args.path), base_dir)
            parts.append(f'path = "{rel}"')
        else:
            parts.append(f'version = "{version}"')
        if feats:
            parts.append("features = [" + ", ".join(f'"{f}"' for f in feats) + "]")
        if args.path or feats:
            return f"{name} = {{ {', '.join(parts)} }}"
        return f'{name} = "{version}"'

    # Decide the declaration's home and edit it textually.
    target, original = None, None
    src = open(app_path).read()
    m = re.search(r"(?ms)^# /// script\n(.*?)^(# ///)$", src)
    if m:
        target, original = app_path, src
        line = f"# {spec_line(app_dir)}\n"
        if re.search(r"(?m)^# \[tool\.yokan\.crates\]$", m.group(1)):
            new_block = re.sub(
                r"(?m)^# \[tool\.yokan\.crates\]$",
                lambda mm: mm.group(0) + "\n" + line.rstrip("\n"),
                m.group(1), count=1,
            )
            edited = src[:m.start(1)] + new_block + src[m.end(1):]
        else:
            insert = "#\n# [tool.yokan.crates]\n" + line
            edited = src[:m.start(2)] + insert + src[m.start(2):]
        open(app_path, "w").write(edited)
    else:
        pp, d = None, app_dir
        while True:
            cand = os.path.join(d, "pyproject.toml")
            if os.path.isfile(cand):
                pp = cand
                break
            up = os.path.dirname(d)
            if up == d:
                break
            d = up
        if pp:
            target, original = pp, open(pp).read()
            line = spec_line(os.path.dirname(pp)) + "\n"
            if re.search(r"(?m)^\[tool\.yokan\.crates\]$", original):
                edited = re.sub(
                    r"(?m)^\[tool\.yokan\.crates\]$",
                    lambda mm: mm.group(0) + "\n" + line.rstrip("\n"),
                    original, count=1,
                )
            else:
                edited = original.rstrip("\n") + "\n\n[tool.yokan.crates]\n" + line
            open(pp, "w").write(edited)
        else:
            target, original = app_path, src
            shebang = src.startswith("#!")
            head_end = src.index("\n") + 1 if shebang else 0
            block = (
                "# /// script\n"
                '# requires-python = ">=3.14"\n'
                "#\n# [tool.yokan.crates]\n"
                f"# {spec_line(app_dir)}\n"
                "# ///\n"
            )
            open(app_path, "w").write(src[:head_end] + block + src[head_end:])

    try:
        decls = app_crate_decls(args.app)
        info = prepare_crates(args.app, {name: decls[name]})
        import types

        holder = types.SimpleNamespace(crate_info=info, uses_crates=set())
        (so,) = build_shims(args.app, holder, names={name})
        fns = sorted(k for k, v in info[name]["fns"].items() if not isinstance(v, str))
        skipped = sorted(k for k, v in info[name]["fns"].items() if isinstance(v, str))
        print(f"added `{name}` to {os.path.relpath(target)}")
        print(f"  door:     {so}")
        print(f"  bindable: {', '.join(fns) if fns else '(nothing crossed — check the crate API)'}")
        if skipped:
            print(f"  skipped:  {', '.join(skipped)} (types outside the crossing set)")
    except (ValueError, Exception) as e:
        open(target, "w").write(original)
        sys.exit(f"add failed (declaration rolled back) — {e}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["check", "translate", "gate", "build", "sync", "add"])
    ap.add_argument("app")
    ap.add_argument("extra", nargs="*",
                    help="for `add`: <crate> [VERSION]")
    ap.add_argument("--path", default=None,
                    help="for `add`: bind a local crate directory instead of a crates.io version")
    ap.add_argument("--features", default="",
                    help="for `add`: comma-separated cargo features")
    ap.add_argument("--script", default="")
    ap.add_argument("--release", action="store_true")
    ap.add_argument("--fresh", action="append", default=[],
                    help="delete this file before EACH tier runs — external-state isolation for persistent apps (repeatable)")
    ap.add_argument("--onefile", action="store_true",
                    help="one distributable file: launcher + embedded runtime, unpacked to the user cache on first run")
    ap.add_argument("--app", dest="app_bundle", action="store_true",
                    help="build only: wrap the artifact as a macOS .app bundle in <app>/dist/")
    ap.add_argument("--bundle", action="store_true",
                    help="ship a python-build-standalone runtime next to the binary (@ui.py apps)")
    args = ap.parse_args()

    for step in args.script.split(","):
        if step.split(":")[0] in ("mem", "a11y"):
            sys.exit(f"`{step}` prints outside the dump; not gate-comparable for now")

    if args.mode == "add":
        do_add(args)
        return

    try:
        pix, tr = translate_file(args.app)
    except Untranslatable as e:
        sys.exit(e.render() if e.file else f"{args.app}: {e.render()}")
    except ValueError as e:
        sys.exit(f"{args.app}: not in the dialect — {e}")

    if args.mode == "check":
        # The checker IS the translator: everything the compiled run
        # would refuse is refused here, before a compiler is started.
        # Silent means the app is in the dialect.
        return

    if args.mode == "translate":
        print(pix, end="")
        return

    if args.mode == "sync":
        # Build the interpreted-run doors for every declared crate, so
        # a plain `uv run app.py` can call them without gating first.
        if not tr.crate_info:
            print("no [tool.yokan.crates] block — nothing to sync")
            return
        for so in build_shims(args.app, tr, names=set(tr.crate_info)):
            print(f"ready: {so}")
        return

    stem = "py" + os.path.splitext(os.path.basename(args.app))[0]
    gate_dir = os.path.join(os.path.dirname(os.path.abspath(args.app)), ".gate")
    os.makedirs(gate_dir, exist_ok=True)
    pix_path = os.path.join(gate_dir, f"{stem}.pix")
    open(pix_path, "w").write(pix)

    if args.onefile:
        args.bundle = True
    if args.bundle and not tr.escapes:
        sys.exit("--bundle / --onefile only apply to apps with @py escapes (none here — the plain binary is already self-contained)")

    if args.mode == "build":
        # Ship only: translate, compile, package. No tier comparison —
        # and no numpy needed HERE even for numpy apps (the translator
        # is ast-only; deps are installed into the bundle, not us).
        if args.script:
            sys.exit("build takes no --script — replaying scripts is the gate's job")
        if tr.escapes or tr.uses_stdlib or tr.uses_crates or tr.window:
            proj = emit_project(gate_dir, stem, pix, tr)
            pbs = find_pbs() if args.bundle else None
            _, binary = tier_b_project(
                proj, "", args.release, pyo3_python=pbs[0] if pbs else None, run=False
            )
            if pbs:
                binary = bundle(proj, binary, stem, pbs[1], pbs[2], app_deps(args.app))
                if args.onefile:
                    binary = make_onefile(proj, stem, os.path.dirname(binary))
        else:
            _, binary = tier_b(pix_path, "", args.release, run=False)
        if args.app_bundle:
            if args.onefile:
                sys.exit("--app makes a folder-shaped bundle; --onefile makes a single file — pick one")
            payload = os.path.dirname(binary) if args.bundle else None
            approot = make_app_bundle(args.app, tr, binary, payload_dir=payload)
            print(f"built: {approot}")
            print("  double-clickable; the executable inside replays PIXIE_SCRIPT like any build")
            return
        print(f"built: {binary} ({describe_artifact(binary)})")
        print("  not gate-checked — `gate` with a script proves the tiers agree")
        return

    for f in args.fresh:
        if os.path.exists(f):
            os.remove(f)
    if tr.uses_crates:
        build_shims(args.app, tr)
        os.environ["YOKAN_EXT_DIR"] = os.path.join(
            os.path.dirname(os.path.abspath(args.app)), ".yokan", "ext"
        )
    a = tier_a(args.app, tr, args.script).rstrip("\n")
    for f in args.fresh:
        if os.path.exists(f):
            os.remove(f)
    if tr.escapes or tr.uses_stdlib or tr.uses_crates or tr.window:
        proj = emit_project(gate_dir, stem, pix, tr)
        pbs = find_pbs() if args.bundle else None
        b_raw, binary = tier_b_project(
            proj, args.script, args.release, pyo3_python=pbs[0] if pbs else None,
            run=not pbs,
        )
        if pbs:
            app = bundle(proj, binary, stem, pbs[1], pbs[2], app_deps(args.app))
            if args.onefile:
                app = make_onefile(proj, stem, os.path.dirname(app))
            b_raw = run_binary(app, args.script)
            binary = app
    else:
        b_raw, binary = tier_b(pix_path, args.script, args.release)
    b = b_raw.rstrip("\n")

    if a == b:
        print(f"GATE OK — {len(a.splitlines())} dump lines identical across tiers")
        print(f"  script:   {args.script or '(none — startup dump only)'}")
        print(f"  emitted:  {pix_path}")
        print(f"  binary:   {binary} ({describe_artifact(binary)})")
    else:
        print("GATE FAILED — tiers diverge:")
        for la, lb in zip(a.splitlines(), b.splitlines()):
            if la != lb:
                print(f"  cpython: {la}")
                print(f"  binary:  {lb}")
        sys.exit(1)


if __name__ == "__main__":
    main()
