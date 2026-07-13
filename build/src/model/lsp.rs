//! The phm language server: evaluate on every edit, answer from the
//! analysis.
//!
//! The server speaks the language protocol over stdio (`phm lsp`). Editor
//! buffers overlay the filesystem through [`load_overlaid`], so everything
//! tracks unsaved text; models are discovered by their `phm.toml` roots
//! within the workspace folders, and every device of every model is
//! evaluated — a component's diagnostics reach its file through whichever
//! devices import it.
//!
//! Each capability lives in its own module:
//! - [`snapshot`] — evaluation, published diagnostics, text geometry,
//! - [`navigate`] — hover, go-to-definition, and the cursor resolution
//!   they share,
//! - [`complete`] — completions,
//! - [`hints`] — inlay hints,
//! - [`rename`] — rename and find-all-references.
//!
//! [`load_overlaid`]: crate::model::load_overlaid

mod complete;
mod hints;
mod navigate;
mod rename;
mod snapshot;

use std::{
    collections::{HashMap, HashSet},
    error::Error,
    path::{Path, PathBuf},
    str::FromStr,
};

use lsp_server::{Connection, Message, Request, Response};
use lsp_types::{
    CompletionOptions, CompletionParams, GotoDefinitionParams, HoverParams,
    HoverProviderCapability, InitializeParams, InlayHintParams, OneOf, ReferenceParams,
    RenameOptions, RenameParams, ServerCapabilities, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncKind, Uri,
    notification::{
        DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument,
        Notification as _,
    },
    request::{
        Completion, GotoDefinition, HoverRequest, InlayHintRequest, PrepareRenameRequest,
        References, Rename, Request as _,
    },
};
use syntax::ast::Span;

use crate::model::elaborate::Analysis;

use self::snapshot::LineIndex;

type Failure = Box<dyn Error + Send + Sync>;

/// The session transcript, for diagnosing editor interplay:
/// `/tmp/phm-lsp-trace.log`, truncated per session.
struct Trace {
    log: Option<std::sync::Mutex<std::fs::File>>,
    started: std::time::Instant,
}

impl Trace {
    fn begin() -> Self {
        Self {
            log: std::fs::File::create("/tmp/phm-lsp-trace.log")
                .ok()
                .map(std::sync::Mutex::new),
            started: std::time::Instant::now(),
        }
    }

    fn note(&self, line: impl std::fmt::Display) {
        if let Some(log) = &self.log
            && let Ok(mut log) = log.lock()
        {
            use std::io::Write as _;
            let _ = writeln!(log, "+{:8.3} {line}", self.started.elapsed().as_secs_f64());
        }
    }
}

/// Serve the language protocol over stdio until the editor ends the
/// session.
pub fn serve() -> Result<(), Failure> {
    let (connection, io_threads) = Connection::stdio();

    let capabilities = serde_json::to_value(ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec![".".to_string(), "#".to_string()]),
            ..Default::default()
        }),
        inlay_hint_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        references_provider: Some(OneOf::Left(true)),
        ..Default::default()
    })?;

    let initialization: InitializeParams =
        serde_json::from_value(connection.initialize(capabilities)?)?;

    let mut folders = initialization
        .workspace_folders
        .unwrap_or_default()
        .into_iter()
        .filter_map(|folder| path_of(&folder.uri))
        .collect::<Vec<_>>();

    #[allow(deprecated)]
    if folders.is_empty()
        && let Some(uri) = initialization.root_uri
        && let Some(path) = path_of(&uri)
    {
        folders.push(path);
    }

    let refreshes = initialization
        .capabilities
        .workspace
        .and_then(|workspace| workspace.inlay_hint)
        .and_then(|hints| hints.refresh_support)
        .unwrap_or(false);

    let mut server = Server {
        folders,
        overlays: HashMap::new(),
        published: HashSet::new(),
        snapshots: Vec::new(),
        refreshes,
        revision: 0,
        vectors: None,
        trace: Trace::begin(),
    };

    server.trace.note(format!(
        "session: folders {:?}, refresh support {}",
        server.folders, server.refreshes,
    ));

    // the session's first picture — the handshake (`connection.initialize`)
    // has already consumed the `initialized` notification, so this cannot
    // wait for it; without it, requests racing ahead of the first edit
    // would be answered from nothing
    server.publish(&connection)?;

    for message in &connection.receiver {
        match message {
            Message::Request(request) => {
                if connection.handle_shutdown(&request)? {
                    break;
                }

                if let Err(error) = server.requested(&connection, request) {
                    eprintln!("phm lsp: {error}");
                }
            }
            Message::Notification(notification) => {
                if let Err(error) = server.notified(&connection, notification) {
                    eprintln!("phm lsp: {error}");
                }
            }
            Message::Response(..) => {}
        }
    }

    // the writer thread ends when every sender is gone
    drop(connection);
    io_threads.join()?;
    Ok(())
}

struct Server {
    /// The workspace folders models are discovered within.
    folders: Vec<PathBuf>,
    /// Unsaved editor buffers, by canonical path — read before the disk.
    overlays: HashMap<PathBuf, String>,
    /// Where diagnostics were last published, for clearing healed files.
    published: HashSet<Uri>,
    /// The last evaluation of every device: sources and analysis, for
    /// answering hover, definition, and completion requests.
    snapshots: Vec<Snapshot>,
    /// Whether the editor honors `workspace/inlayHint/refresh`.
    refreshes: bool,
    /// Server-issued request ids.
    revision: i32,
    /// What the vectors hashed to at the last publish — `None` before the
    /// first, whose picture the editor fetches on its own.
    vectors: Option<u64>,
    /// The session transcript.
    trace: Trace,
}

/// One evaluated device: its loaded sources and what elaboration learned.
struct Snapshot {
    files: Vec<SnapshotFile>,
    analysis: Analysis,
}

struct SnapshotFile {
    /// Canonical, when the file came from disk.
    path: Option<PathBuf>,
    content: String,
    index: LineIndex,
}

/// What a reference under the cursor names.
enum Named {
    /// A model context path — resolvable through [`Analysis::locations`].
    Path(Vec<String>),
    /// A definition site, directly.
    Site(Span),
}

impl Server {
    fn notified(
        &mut self,
        connection: &Connection,
        notification: lsp_server::Notification,
    ) -> Result<(), Failure> {
        self.trace.note(format!("→ {}", notification.method));

        match notification.method.as_str() {
            DidOpenTextDocument::METHOD => {
                let params: lsp_types::DidOpenTextDocumentParams =
                    serde_json::from_value(notification.params)?;

                if let Some(path) = path_of(&params.text_document.uri) {
                    self.overlays
                        .insert(canonical(&path), params.text_document.text);
                    self.publish(connection)?;
                }
            }
            DidChangeTextDocument::METHOD => {
                let params: lsp_types::DidChangeTextDocumentParams =
                    serde_json::from_value(notification.params)?;

                // full synchronization: the last change is the whole text
                if let (Some(path), Some(change)) = (
                    path_of(&params.text_document.uri),
                    params.content_changes.into_iter().next_back(),
                ) {
                    self.overlays.insert(canonical(&path), change.text);
                    self.publish(connection)?;
                }
            }
            DidCloseTextDocument::METHOD => {
                let params: lsp_types::DidCloseTextDocumentParams =
                    serde_json::from_value(notification.params)?;

                if let Some(path) = path_of(&params.text_document.uri) {
                    self.overlays.remove(&canonical(&path));
                    self.publish(connection)?;
                }
            }
            DidSaveTextDocument::METHOD => self.publish(connection)?,
            _ => {}
        }

        Ok(())
    }

    fn requested(&self, connection: &Connection, request: Request) -> Result<(), Failure> {
        let Request { id, method, params } = request;

        self.trace.note(format!("→ {method} {params}"));

        let response = match method.as_str() {
            HoverRequest::METHOD => match serde_json::from_value::<HoverParams>(params) {
                Ok(params) => {
                    Response::new_ok(id, self.hover(&params.text_document_position_params))
                }
                Err(error) => malformed(id, &method, error),
            },
            GotoDefinition::METHOD => {
                match serde_json::from_value::<GotoDefinitionParams>(params) {
                    Ok(params) => Response::new_ok(
                        id,
                        self.definition(&params.text_document_position_params),
                    ),
                    Err(error) => malformed(id, &method, error),
                }
            }
            Completion::METHOD => match serde_json::from_value::<CompletionParams>(params) {
                Ok(params) => Response::new_ok(id, self.complete(&params.text_document_position)),
                Err(error) => malformed(id, &method, error),
            },
            InlayHintRequest::METHOD => match serde_json::from_value::<InlayHintParams>(params) {
                Ok(params) => Response::new_ok(id, self.hints(&params)),
                Err(error) => malformed(id, &method, error),
            },
            PrepareRenameRequest::METHOD => {
                match serde_json::from_value::<TextDocumentPositionParams>(params) {
                    Ok(params) => Response::new_ok(id, self.prepare_rename(&params)),
                    Err(error) => malformed(id, &method, error),
                }
            }
            Rename::METHOD => match serde_json::from_value::<RenameParams>(params) {
                Ok(params) => Response::new_ok(
                    id,
                    self.rename(&params.text_document_position, &params.new_name),
                ),
                Err(error) => malformed(id, &method, error),
            },
            References::METHOD => match serde_json::from_value::<ReferenceParams>(params) {
                Ok(params) => Response::new_ok(
                    id,
                    self.references(
                        &params.text_document_position,
                        params.context.include_declaration,
                    ),
                ),
                Err(error) => malformed(id, &method, error),
            },
            _ => Response::new_err(
                id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                format!("unsupported request: {method}"),
            ),
        };

        self.trace.note(format!(
            "← {}",
            serde_json::to_string(&response)
                .unwrap_or_default()
                .chars()
                .take(220)
                .collect::<String>(),
        ));

        connection.sender.send(Message::Response(response))?;
        Ok(())
    }

    /// The phm model roots within the workspace folders.
    fn models(&self) -> Vec<PathBuf> {
        let mut found = Vec::new();

        for folder in &self.folders {
            descend(folder, &mut found, 0);
        }

        found
    }

    /// The snapshot file at a canonical path — from whichever snapshot
    /// holds it.
    fn file_at(&self, path: &Path) -> Option<&SnapshotFile> {
        self.snapshots.iter().find_map(|snapshot| {
            snapshot
                .files
                .iter()
                .find(|file| file.path.as_deref() == Some(path))
        })
    }
}

/// Collect `phm.toml`-marked model roots under `directory`.
fn descend(directory: &Path, found: &mut Vec<PathBuf>, depth: usize) {
    if depth > 8 {
        return;
    }

    if directory.join("phm.toml").is_file() {
        found.push(directory.to_path_buf());
        return;
    }

    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();

        if path.is_dir() && !name.starts_with('.') && name != "target" && name != "node_modules" {
            descend(&path, found, depth + 1);
        }
    }
}

/// The response to a request whose parameters do not deserialize.
fn malformed(id: lsp_server::RequestId, method: &str, error: serde_json::Error) -> Response {
    Response::new_err(
        id,
        lsp_server::ErrorCode::InvalidParams as i32,
        format!("malformed {method} parameters: {error}"),
    )
}

/// A path's canonical form — itself, when the filesystem cannot answer.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// The filesystem path a `file://` uri names.
fn path_of(uri: &Uri) -> Option<PathBuf> {
    url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()
}

/// The `file://` uri naming a filesystem path.
fn uri_of(path: &Path) -> Option<Uri> {
    Uri::from_str(url::Url::from_file_path(path).ok()?.as_str()).ok()
}
