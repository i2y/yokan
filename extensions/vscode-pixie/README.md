# pixie Language for VS Code

Language support for **pixie** — the GUI-first language whose `.pix`
sources compile to Rust and render through gpui.

## Features

### From the language server

Errors appear as you type, with the compiler's own diagnostics — the
same ones `pixie build` prints, including its suggestions:

    no member `nope` on class `Session` (did you mean `name`?)

- **Hover** a name, a property or a method for its declaration
- **Go to definition** (F12) on any of them
- **Completion**, both bare names and members after a `.`

The server is the `pixie-lsp` binary. The extension looks for it in
this order:

1. the `pixie.lsp.path` setting, if you set one
2. `target/release/pixie-lsp` then `target/debug/pixie-lsp` in the
   open workspace — so a checkout works after
   `cargo build --release -p pixie-lsp`, with no configuration
3. `pixie-lsp` on `PATH`

If none of those find it, highlighting still works and the extension
says so once. **pixie: Restart Language Server** in the command
palette picks up a server you built after opening the editor.

### From the grammar

- Highlighting for `.pix` sources and `.rpi` binding files
- Knows the pixie surface, not a C-like approximation:
  - `store` / `state` / `view` / `style` / `Slot` and the 18-widget
    catalog (`Column`, `Row`, `Grid`, `Text`, `Button`, `ListView`,
    `Modal`, `BarChart`, `Spinner`, …) get their own scope
  - element properties, style keys and map keys (`text:`, `onClick:`,
    `hover.background:`) read as attributes; parameter labels don't
  - **postfix return types** (`fn sum Int {`), error unions (`!Float`),
    nullables (`String?`), ranges (`0..=n`)
  - `#{expr}` interpolation is highlighted as embedded code, including
    the `#{v:.2f}` format spec
  - `case` / `when ok(v)` / `when nil`, enum and error variants
  - `@rust("std::fs::read_to_string")` attributes in `.rpi` files
- `#` line comments, bracket matching, auto-closing pairs, 2-space indent

## Install

### Development (this repo)

```sh
ln -sfn "$(pwd)/extensions/vscode-pixie" ~/.vscode/extensions/pixie-language-dev
```

Then open a new window (`Cmd/Ctrl+Shift+P` → "Reload Window"). Grammar
edits take effect after another reload.

Cursor / VSCodium / Windsurf read their own extension directory —
`~/.cursor/extensions`, `~/.vscode-oss/extensions`,
`~/.windsurf/extensions` — the same symlink works there.

### Packaged

```sh
npm install -g @vscode/vsce
cd extensions/vscode-pixie
vsce package
code --install-extension pixie-language-0.1.0.vsix
```

## Checking the grammar after an edit

`test/highlight-sample.pix` and `test/highlight-sample.rpi` are fixtures
that exercise every construct the grammar claims to cover — open them
and look.

For an objective pass, `test/scope-check.js` runs the same TextMate
engine VS Code ships and reports every fragment the grammar left
unscoped (exit code 1 if there is any):

```sh
cd extensions/vscode-pixie
npm install vscode-textmate vscode-oniguruma      # not vendored
node test/scope-check.js --histogram test/highlight-sample.pix test/highlight-sample.rpi
node test/scope-check.js $(find ../../examples -name '*.pix' -o -name '*.rpi')
```

Every `.pix` / `.rpi` in the repo tokenizes with zero unscoped
fragments today; the histogram is the quick way to see whether a new
rule actually fires.

## Scopes the grammar emits

| pixie construct | scope |
| --- | --- |
| widget catalog + `Slot` | `support.class.widget.pixie` |
| user component used as an element | `entity.name.type.element.pixie` |
| element / style / map key | `entity.other.attribute-name.pixie` |
| `hover.` / `active.` prefix | `entity.other.attribute-name.pseudo-state.pixie` |
| `state` / `prop` / `signal` / `slot` member name | `variable.other.member.declaration.pixie` |
| parameter label | `variable.parameter.pixie` |
| `Int` `Float` `Bool` `String` `Bytes` `Void` `List` `Map` | `support.type.builtin.pixie` |
| other PascalCase names | `entity.name.type.pixie` |
| enum / error variants, `when ok(v)` | `variable.other.enummember.pixie` |
| `!T` error union | `keyword.operator.error-union.pixie` |
| `#{…}` interpolation | `meta.embedded.line.pixie` |
| `#{v:.2f}` spec | `constant.other.format-spec.pixie` |
| `@rust(…)` | `entity.name.function.decorator.pixie` |

## License

MIT OR Apache-2.0, same as the rest of the repo.
