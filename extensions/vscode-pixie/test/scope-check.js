#!/usr/bin/env node
// Runs the real TextMate engine (the one VS Code ships) over pixie
// sources and reports how the grammar scoped them. A fragment that
// comes back as plain `source.pixie` is text the grammar never claimed
// — the regression signal after a grammar edit.
//
//   npm install vscode-textmate vscode-oniguruma
//   node test/scope-check.js [--histogram] <file.pix|file.rpi> ...
//
// Exits non-zero if anything is left unscoped.

const fs = require('fs');
const path = require('path');
const vsctm = require('vscode-textmate');
const oniguruma = require('vscode-oniguruma');

const args = process.argv.slice(2);
const histogram = args.includes('--histogram');
const files = args.filter((a) => a !== '--histogram');
if (files.length === 0) {
  console.error('usage: node test/scope-check.js [--histogram] <files...>');
  process.exit(2);
}

const grammarPath = path.join(__dirname, '..', 'syntaxes', 'pixie.tmLanguage.json');
const wasmPath = path.join(path.dirname(require.resolve('vscode-oniguruma')), 'onig.wasm');
const onigLib = oniguruma.loadWASM(fs.readFileSync(wasmPath).buffer).then(() => ({
  createOnigScanner: (patterns) => new oniguruma.OnigScanner(patterns),
  createOnigString: (s) => new oniguruma.OnigString(s),
}));

const registry = new vsctm.Registry({
  onigLib,
  loadGrammar: () =>
    Promise.resolve(vsctm.parseRawGrammar(fs.readFileSync(grammarPath, 'utf8'), grammarPath)),
});

registry
  .loadGrammar('source.pixie')
  .then((grammar) => {
    const counts = new Map();
    let unscoped = 0;
    for (const file of files) {
      let ruleStack = vsctm.INITIAL;
      const lines = fs.readFileSync(file, 'utf8').split(/\r?\n/);
      lines.forEach((line, i) => {
        const result = grammar.tokenizeLine(line, ruleStack);
        ruleStack = result.ruleStack;
        for (const token of result.tokens) {
          const fragment = line.slice(token.startIndex, token.endIndex);
          if (!fragment.trim()) continue;
          const scope = token.scopes[token.scopes.length - 1];
          counts.set(scope, (counts.get(scope) || 0) + 1);
          if (scope === 'source.pixie') {
            unscoped++;
            console.log(`unscoped  ${file}:${i + 1}  ${JSON.stringify(fragment)}`);
          }
        }
      });
    }
    if (histogram) {
      console.log('\nscope histogram:');
      [...counts.entries()]
        .sort((a, b) => b[1] - a[1])
        .forEach(([scope, n]) => console.log(`  ${String(n).padStart(6)}  ${scope}`));
    }
    console.log(`\n${files.length} file(s), ${unscoped} unscoped fragment(s)`);
    process.exit(unscoped === 0 ? 0 : 1);
  })
  .catch((err) => {
    console.error('grammar failed to load:', err);
    process.exit(2);
  });
