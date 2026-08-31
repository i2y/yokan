// The pixie language client.
//
// `pixie-lsp` is a stdio JSON-RPC server that already answers
// diagnostics, hover, go-to-definition and completion. This file is
// the half that was missing: it finds the binary, starts it, and
// hands VS Code the connection.
//
// Finding the binary is the whole design problem here, because a
// contributor running from a checkout and a user who installed the
// extension have it in different places. The order below tries the
// explicit setting first, then a dev checkout, then PATH — and when
// all three miss it says so once, with the setting name, rather than
// failing silently and looking like the server is broken.

const path = require("path");
const fs = require("fs");
const { execFile } = require("child_process");
const vscode = require("vscode");
const { LanguageClient, TransportKind } = require("vscode-languageclient/node");

let client;

/** The workspace folder that owns `doc`, or the first one. */
function anyFolder() {
  const fs_ = vscode.workspace.workspaceFolders;
  return fs_ && fs_.length ? fs_[0].uri.fsPath : undefined;
}

/**
 * Where to find `pixie-lsp`, most specific first:
 *
 *   1. `pixie.lsp.path` — an explicit answer always wins.
 *   2. A cargo build in the open workspace, release before debug.
 *      This is the contributor case: clone, `cargo build -p
 *      pixie-lsp`, open the folder, and it works with no settings.
 *   3. `pixie-lsp` on PATH — the installed case.
 */
function findServer() {
  const configured = vscode.workspace
    .getConfiguration("pixie")
    .get("lsp.path");
  if (configured) {
    return { command: configured, why: "the `pixie.lsp.path` setting" };
  }
  const root = anyFolder();
  if (root) {
    for (const profile of ["release", "debug"]) {
      const p = path.join(root, "target", profile, "pixie-lsp");
      if (fs.existsSync(p)) {
        return { command: p, why: `a ${profile} build in this workspace` };
      }
    }
  }
  return { command: "pixie-lsp", why: "PATH" };
}

/** Does `command` actually run? Answers before we blame the server. */
function probe(command) {
  return new Promise((resolve) => {
    execFile(command, ["--version"], { timeout: 5000 }, (err) => {
      // A server that starts and rejects `--version` is still a
      // server; only "cannot execute" means we have the wrong path.
      resolve(!err || err.code !== "ENOENT");
    });
  });
}

async function start(context, output) {
  const { command, why } = findServer();
  if (!(await probe(command))) {
    output.appendLine(
      `pixie: no language server at \`${command}\` (tried ${why}).`
    );
    vscode.window
      .showWarningMessage(
        "pixie: language server not found. Syntax highlighting still works.",
        "How to fix"
      )
      .then((choice) => {
        if (choice === "How to fix") {
          vscode.window.showInformationMessage(
            "Build it with `cargo build --release -p pixie-lsp`, or set " +
              "`pixie.lsp.path` to the binary."
          );
        }
      });
    return;
  }
  output.appendLine(`pixie: language server at \`${command}\` (${why}).`);

  const run = { command, transport: TransportKind.stdio };
  client = new LanguageClient(
    "pixie",
    "pixie Language Server",
    { run, debug: run },
    {
      documentSelector: [{ scheme: "file", language: "pixie" }],
      // The server type-checks whole modules, so a change to a
      // sibling `.pix` can change this file's diagnostics.
      synchronize: {
        fileEvents: vscode.workspace.createFileSystemWatcher("**/*.{pix,rpi}"),
      },
      outputChannel: output,
    }
  );
  await client.start();
  context.subscriptions.push(client);
}

function activate(context) {
  const output = vscode.window.createOutputChannel("pixie");
  context.subscriptions.push(output);
  context.subscriptions.push(
    vscode.commands.registerCommand("pixie.restartServer", async () => {
      if (client) {
        await client.stop();
        client = undefined;
      }
      await start(context, output);
      vscode.window.showInformationMessage("pixie: language server restarted.");
    })
  );
  start(context, output).catch((e) => {
    output.appendLine(`pixie: could not start the language server: ${e}`);
  });
}

async function deactivate() {
  if (client) {
    await client.stop();
  }
}

module.exports = { activate, deactivate };
