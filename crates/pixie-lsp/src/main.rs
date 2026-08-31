//! `cute-lsp` binary: stdio LSP server for `.pix` files.
//!
//! Editor wiring (e.g. VS Code, Helix, Neovim) launches this binary,
//! pipes JSON-RPC messages over stdin/stdout, and listens for
//! `textDocument/publishDiagnostics`. All compile work happens in the
//! library half (`pixie_lsp::Backend`); this entry exists only to spin
//! up a tokio runtime and the tower-lsp `Server`.

use pixie_lsp::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
