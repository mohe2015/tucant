#![warn(clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::multiple_crate_versions
)]

pub mod evaluator;
pub mod parser;

use std::{collections::HashMap, sync::Arc, vec};

use bytes::{Buf, BytesMut};
use clap::Parser;
use futures_util::{Sink, SinkExt, Stream, StreamExt};
use itertools::Itertools;
use parser::{highlight_visitor, hover_visitor, list_visitor, parse_from_str, Ast, FAKE_SPAN};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Serialize;
use serde_json::Value;
use tokio::{
    net::{TcpListener, UnixStream},
    sync::{mpsc, oneshot, RwLock},
};
use tokio_tungstenite::tungstenite::Message;
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};
use tucant_language_server_derive_output::{
    CompletionOptions, Diagnostic, DiagnosticSeverity, DocumentHighlight, DocumentHighlightKind,
    H07cfb623af7dea337d0e304325abc9453187c524fb5e436547852fdc,
    H123ba34418f5bf58482d5c391e9bc084a642c554b2ec6d589db0de1d,
    H1e2267041560020dc953eb5d9d8f0c194de0f657a1193f66abeab062,
    H2ac6f0a8906c9e0e69380d6c8ff247d1a746dae2e45f26f17eb9d93c,
    H3424688d17603d45dbf7bc9bc9337e660ef00dd90b070777859fbf1e,
    H5f8b902ef452cedc6b143f87b02d86016c018ed08ad7f26834df1d13,
    H8aab3d49c891c78738dc034cb0cb70ee2b94bf6c13a697021734fff7,
    H96adce06505d36c9b352c6cf574cc0b4715c349e1dd3bd60d1ab63f4,
    Hb33d389f4db33e188f5f7289bda48f700ee05a6244701313be32e552,
    Hb617b9fe394cc04976341932ae3d87256285a2654f1c9e6beddf7483,
    He98ccfdc940d4c1fa4b43794669192a12c560d6457d392bc00630cb4,
    Hf21695c74b3402f0de46005d3e2008486ab02d88f9adaff6b6cce6b2, Hover, HoverOptions, IncomingStuff,
    InitializeRequest, InitializeResult, InitializedNotification, MarkupContent, MarkupKind,
    MessageType, Position, PublishDiagnosticsParams, Receivable, SemanticTokens,
    SemanticTokensLegend, SemanticTokensOptions, Sendable, SendableAndForget, ServerCapabilities,
    ShowMessageParams, ShutdownRequest, StringOrNumber, TextDocumentCompletionRequest,
    TextDocumentDidChangeNotification, TextDocumentDidCloseNotification,
    TextDocumentDidOpenNotification, TextDocumentDocumentHighlightRequest,
    TextDocumentFoldingRangeRequest, TextDocumentHoverRequest,
    TextDocumentPublishDiagnosticsNotification, TextDocumentSemanticTokensFullRequest,
    TextDocumentSyncKind, TextDocumentSyncOptions, WindowShowMessageNotification,
    WorkDoneProgressOptions,
};

use crate::{
    evaluator::typecheck,
    parser::{visitor, Error},
};

#[derive(Parser)]
struct Args {
    #[arg(long)]
    pipe: Option<String>,

    #[arg(long)]
    port: Option<u16>,

    #[arg(long)]
    stdin: bool,

    #[arg(long)]
    websocket: Option<u16>,
}

// https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/

pub struct Server {
    documents: RwLock<HashMap<String, String>>,
    pending: RwLock<HashMap<String, oneshot::Sender<Value>>>,
    tx: mpsc::Sender<String>,
}

impl Server {
    async fn handle_receiving<
        R: Stream<Item = Result<String, anyhow::Error>> + std::marker::Send + std::marker::Unpin,
    >(
        self: Arc<Self>,
        mut reader: R,
    ) -> anyhow::Result<()> {
        loop {
            let request: IncomingStuff = serde_json::from_str(&reader.next().await.unwrap()?)?;

            let cloned_self = self.clone();

            // currently most of these are really not safe to run concurrently
            //tokio::spawn(async move {
            match request {
                IncomingStuff::TextDocumentSemanticTokensFullRequest(request) => cloned_self
                    .handle_text_document_semantic_tokens_full_request(request)
                    .await
                    .unwrap(),
                IncomingStuff::ShutdownRequest(request) => {
                    cloned_self.handle_shutdown_request(request).await.unwrap();
                }
                IncomingStuff::TextDocumentDidOpenNotification(notification) => cloned_self
                    .handle_text_document_did_open_notification(notification)
                    .await
                    .unwrap(),
                IncomingStuff::TextDocumentDidCloseNotification(notification) => cloned_self
                    .handle_text_document_did_close_notification(notification)
                    .await
                    .unwrap(),
                IncomingStuff::TextDocumentDidChangeNotification(notification) => cloned_self
                    .handle_text_document_did_change_notification(notification)
                    .await
                    .unwrap(),
                IncomingStuff::InitializeRequest(request) => {
                    cloned_self.handle_initialize(request).await.unwrap();
                }
                IncomingStuff::InitializedNotification(notification) => cloned_self
                    .handle_initialized_notification(notification)
                    .await
                    .unwrap(),
                IncomingStuff::TextDocumentFoldingRangeRequest(request) => cloned_self
                    .handle_text_document_folding_range_request(request)
                    .await
                    .unwrap(),
                IncomingStuff::TextDocumentDocumentHighlightRequest(request) => cloned_self
                    .handle_document_highlight_request(request)
                    .await
                    .unwrap(),
                IncomingStuff::SetTraceNotification(_notification) => {}
                IncomingStuff::CancelRequestNotification(_notification) => {}
                IncomingStuff::TextDocumentCompletionRequest(request) => cloned_self
                    .handle_text_document_completion_request(request)
                    .await
                    .unwrap(),
                IncomingStuff::TextDocumentHoverRequest(request) => cloned_self
                    .handle_text_document_hover_request(request)
                    .await
                    .unwrap(),
                _ => todo!(),
            }
            //});
        }
    }

    async fn handle_text_document_hover_request(
        self: Arc<Self>,
        request: TextDocumentHoverRequest,
    ) -> anyhow::Result<()> {
        let documents = self.documents.read().await;
        let document = documents.get(&request.params.variant0.text_document.uri);
        let document = if let Some(document) = document {
            document.clone()
        } else {
            tokio::fs::read_to_string(&request.params.variant0.text_document.uri).await?
        };
        drop(documents);

        let value = match parse_from_str(&document) {
            Ok(value) => value,
            Err(Error {
                partial_parse: _, ..
            }) => (Ast::List(vec![]), FAKE_SPAN.clone()), // TODO FIXME
        };

        let found_element = hover_visitor(value.clone(), &request.params.variant0.position);

        let response = found_element.and_then(|found_element| {
            println!("found element {found_element:?}");
            let typecheck = typecheck(value).1;

            typecheck
                .into_iter()
                .filter_map(std::result::Result::ok)
                .find(|t| t.1.range.start == found_element.1.range.start)
                .map(|found_type| {
                    println!("found type {:?}", found_type.0);
                    H96adce06505d36c9b352c6cf574cc0b4715c349e1dd3bd60d1ab63f4::Variant0(Hover {
                        contents:
                            H5f8b902ef452cedc6b143f87b02d86016c018ed08ad7f26834df1d13::Variant0(
                                MarkupContent {
                                    kind: MarkupKind::Markdown,
                                    value: format!("{:?}", found_type.0),
                                },
                            ),
                        range: Some(found_type.1.range),
                    })
                })
        });

        if let Some(response) = response {
            self.send_response(request, response).await?;
        } else {
            self.send_response(
                request,
                H96adce06505d36c9b352c6cf574cc0b4715c349e1dd3bd60d1ab63f4::Variant1(()),
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_text_document_completion_request(
        self: Arc<Self>,
        request: TextDocumentCompletionRequest,
    ) -> anyhow::Result<()> {
        // request.params.variant0.position
        // TODO FIXMe find the location

        self.send_response(
            request,
            H2ac6f0a8906c9e0e69380d6c8ff247d1a746dae2e45f26f17eb9d93c::Variant2(()),
        )
        .await?;

        Ok(())
    }

    async fn handle_document_highlight_request(
        self: Arc<Self>,
        request: TextDocumentDocumentHighlightRequest,
    ) -> anyhow::Result<()> {
        // https://github.com/microsoft/vscode-languageserver-node/tree/main/client
        // https://github.com/microsoft/vscode-languageserver-node/blob/f97bb73dbfb920af4bc8c13ecdcdc16359cdeda6/client/src/common/documentHighlight.ts
        // https://github.com/microsoft/vscode/search?q=provideDocumentHighlights
        // https://github.com/microsoft/vscode/issues/42649
        // https://github.com/microsoft/vscode/issues/51869
        let documents = self.documents.read().await;
        let document = documents.get(&request.params.variant0.text_document.uri);
        let document = if let Some(document) = document {
            document.clone()
        } else {
            tokio::fs::read_to_string(&request.params.variant0.text_document.uri).await?
        };
        drop(documents);

        let value = match parse_from_str(&document) {
            Ok(value) => value,
            Err(Error {
                partial_parse: _, ..
            }) => (Ast::List(vec![]), FAKE_SPAN.clone()), // TODO FIXME
        };

        let found_element = highlight_visitor(value.clone(), &request.params.variant0.position);

        let response = H123ba34418f5bf58482d5c391e9bc084a642c554b2ec6d589db0de1d::Variant0(
            found_element
                .into_iter()
                .map(|found_element| DocumentHighlight {
                    range: found_element.range,
                    kind: Some(DocumentHighlightKind::Text),
                })
                .collect_vec(),
        );

        self.send_response(request, response).await?;

        Ok(())
    }

    async fn handle_text_document_folding_range_request(
        self: Arc<Self>,
        request: TextDocumentFoldingRangeRequest,
    ) -> anyhow::Result<()> {
        let documents = self.documents.read().await;
        let document = documents.get(&request.params.text_document.uri);
        let document = if let Some(document) = document {
            document.clone()
        } else {
            tokio::fs::read_to_string(&request.params.text_document.uri).await?
        };
        drop(documents);

        let value = match parse_from_str(&document) {
            Ok(value) => value,
            Err(Error {
                partial_parse: _, ..
            }) => (Ast::List(vec![]), FAKE_SPAN.clone()), // TODO FIXME,
        };

        let response = H8aab3d49c891c78738dc034cb0cb70ee2b94bf6c13a697021734fff7::Variant0(
            list_visitor(value).collect(),
        );

        self.send_response(request, response).await?;

        Ok(())
    }

    pub async fn recalculate_diagnostics(
        self: Arc<Self>,
        content: &str,
        uri: String,
        version: i64,
    ) -> anyhow::Result<()> {
        let vec = {
            let value = parse_from_str(content);

            let diagnostics: Box<dyn Iterator<Item = Diagnostic>> = if let Err(ref error) = value {
                Box::new(std::iter::once(Diagnostic {
                    range: error.location.range.clone(),
                    severity: Some(DiagnosticSeverity::Error),
                    code: None,
                    code_description: None,
                    source: Some("tucant".to_string()),
                    message: error.reason.to_string(),
                    tags: None,
                    related_information: None,
                    data: None,
                }))
            } else {
                let typecheck = typecheck(value.unwrap()).1;
                Box::new(
                    typecheck
                        .into_iter()
                        .filter_map(std::result::Result::err)
                        .map(|e| Diagnostic {
                            range: e.location.range,
                            severity: Some(DiagnosticSeverity::Error),
                            code: None,
                            code_description: None,
                            source: Some("tucant".to_string()),
                            message: e.reason,
                            tags: None,
                            related_information: None,
                            data: None,
                        }),
                )
            };

            diagnostics.collect_vec()
        };

        let response = PublishDiagnosticsParams {
            uri,
            version: Some(version),
            diagnostics: vec,
        };

        self.send_notification::<TextDocumentPublishDiagnosticsNotification>(response)
            .await?;

        Ok(())
    }

    async fn handle_text_document_semantic_tokens_full_request(
        self: Arc<Self>,
        request: TextDocumentSemanticTokensFullRequest,
    ) -> anyhow::Result<()> {
        let documents = self.documents.read().await;
        let document = documents.get(&request.params.text_document.uri);
        let document = if let Some(document) = document {
            document.clone()
        } else {
            tokio::fs::read_to_string(&request.params.text_document.uri).await?
        };
        drop(documents);

        let value = match parse_from_str(&document) {
            Ok(value) => value,
            Err(Error {
                partial_parse: _, ..
            }) => (Ast::List(vec![]), FAKE_SPAN.clone()), // TODO FIXME
        };

        let result = std::iter::once((0, 0, 0, 0, 0))
            .chain(visitor(value.clone()))
            .zip(visitor(value))
            .flat_map(|(last, this)| {
                vec![
                    this.0 - last.0,
                    if this.0 == last.0 {
                        this.1 - last.1
                    } else {
                        this.1
                    },
                    this.2,
                    this.3,
                    this.4,
                ]
            })
            .collect::<Vec<_>>();

        let response =
            He98ccfdc940d4c1fa4b43794669192a12c560d6457d392bc00630cb4::Variant0(SemanticTokens {
                result_id: None,
                data: result,
            });

        self.send_response(request, response).await.unwrap();

        Ok(())
    }

    async fn handle_shutdown_request(
        self: Arc<Self>,
        request: ShutdownRequest,
    ) -> anyhow::Result<()> {
        self.send_response(request, ()).await.unwrap();

        Ok(())
    }

    async fn handle_text_document_did_open_notification(
        self: Arc<Self>,
        notification: TextDocumentDidOpenNotification,
    ) -> anyhow::Result<()> {
        let mut documents = self.documents.write().await;
        documents.insert(
            notification.params.text_document.uri.clone(),
            notification.params.text_document.text.clone(),
        );

        drop(documents);

        self.recalculate_diagnostics(
            &notification.params.text_document.text,
            notification.params.text_document.uri,
            notification.params.text_document.version,
        )
        .await?;

        Ok(())
    }

    async fn handle_text_document_did_close_notification(
        self: Arc<Self>,
        notification: TextDocumentDidCloseNotification,
    ) -> anyhow::Result<()> {
        let mut documents = self.documents.write().await;
        documents.remove(&notification.params.text_document.uri);

        Ok(())
    }

    #[must_use]
    pub fn line_column_to_offset(string: &str, position: &Position) -> usize {
        let the_line = string
            .lines()
            .nth(position.line.try_into().unwrap())
            .unwrap();
        let line_offset = the_line
            .char_indices()
            .nth(position.character.try_into().unwrap())
            .map_or(the_line.len(), |(offset, _)| offset);
        the_line.as_ptr() as usize - string.as_ptr() as usize + line_offset
    }

    // TODO FIXME these and quite some others need to respect some order
    async fn handle_text_document_did_change_notification(
        self: Arc<Self>,
        notification: TextDocumentDidChangeNotification,
    ) -> anyhow::Result<()> {
        let mut documents = self.documents.write().await;
        let mut document = documents
            .get(&notification.params.text_document.variant0.uri)
            .unwrap()
            .clone();

        for change in &notification.params.content_changes {
            match change {
                tucant_language_server_derive_output::H1e795c7a94f7c86c614f1f1590c41c0496c29d3fe5ac6533d292f0e6::Variant0(incremental_changes) => {
                    let start_offset = Self::line_column_to_offset(&document, &incremental_changes.range.start);
                    let end_offset = Self::line_column_to_offset(&document, &incremental_changes.range.end);

                    document = format!("{}{}{}", &document[..start_offset], incremental_changes.text, &document[end_offset..]);

                    documents.insert(
                        notification.params.text_document.variant0.uri.clone(),
                        document.clone(),
                    );
                },
                tucant_language_server_derive_output::H1e795c7a94f7c86c614f1f1590c41c0496c29d3fe5ac6533d292f0e6::Variant1(changes) => {
                    documents.insert(notification.params.text_document.variant0.uri.clone(), changes.text.clone());
                },
            }
        }

        let contents = documents
            .get(&notification.params.text_document.variant0.uri)
            .unwrap()
            .clone();

        drop(documents);
        /*
                if notification.params.content_changes.len() == 1 {
                    match notification.params.content_changes[0] {
                        tucant_language_server_derive_output::H25fd6c7696dff041d913d0a9d3ce2232683e5362f0d4c6ca6179cf92::Variant0(ref incremental_changes) => {
                            let _start_offset = line_column_to_offset(&document, incremental_changes.range.start.line.try_into().unwrap(), incremental_changes.range.start.character.try_into().unwrap());
                            let _end_offset = line_column_to_offset(&document, incremental_changes.range.end.line.try_into().unwrap(), incremental_changes.range.end.character.try_into().unwrap());

                            let response = ApplyWorkspaceEditParams {
                                label: Some("insert matching paren".to_string()),
                                edit: WorkspaceEdit {
                                    changes: None,
                                    document_changes: Some(vec![
                                        H1332ceed95c3cca3c02eed7277ac86fcb37ac84398216e85560c37bf::Variant0(TextDocumentEdit {
                                            text_document: OptionalVersionedTextDocumentIdentifier {
                                                variant0: TextDocumentIdentifier { uri: notification.params.text_document.variant0.uri.clone() },
                                                version: Hf7dce6b26d9e110d906dc3150d7d569f6983091049d0e763bb4a5cec::Variant0(notification.params.text_document.version)
                                            },
                                            edits: vec![
                                                Hbc05edec65fcb6ecb06a32c6c6bd742b6b3682f1da78657cd86b8f05::Variant0(TextEdit {
                                                    range: Range { start: Position { line: incremental_changes.range.end.line, character: incremental_changes.range.end.character }, end: Position { line: incremental_changes.range.end.line, character: incremental_changes.range.end.character } },
                                                    new_text: r#"""#.to_string()
                                                })
                                            ]
                                        })
                                    ]),
                                    change_annotations: None,
                                }
                            };

                            self.clone().send_request::<WorkspaceApplyEditRequest>(response).await?;
                        },
                        _ => {}
                    }
                }
        */
        self.recalculate_diagnostics(
            &contents,
            notification.params.text_document.variant0.uri,
            notification.params.text_document.version,
        )
        .await?;

        Ok(())
    }

    async fn handle_initialized_notification(
        self: Arc<Self>,
        _notification: InitializedNotification,
    ) -> anyhow::Result<()> {
        let notification = ShowMessageParams {
            r#type: MessageType::Error,
            message: "This is a test error".to_string(),
        };

        self.send_notification::<WindowShowMessageNotification>(notification)
            .await?;

        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_initialize(self: Arc<Self>, request: InitializeRequest) -> anyhow::Result<()> {
        let result = InitializeResult {
            capabilities: ServerCapabilities {
                position_encoding: None,
                text_document_sync: Some(
                    H1e2267041560020dc953eb5d9d8f0c194de0f657a1193f66abeab062::Variant0(
                        TextDocumentSyncOptions {
                            open_close: Some(true),
                            will_save: None,
                            will_save_wait_until: None,
                            change: Some(TextDocumentSyncKind::Incremental),
                            save: None, // TODO FIXME
                        },
                    ),
                ),
                notebook_document_sync: None,
                completion_provider: Some(CompletionOptions {
                    variant0: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                    trigger_characters: Some(vec![r#"""#.to_string()]),
                    all_commit_characters: Some(vec![r#"""#.to_string()]),
                    resolve_provider: None,
                    completion_item: None,
                }),
                hover_provider: Some(
                    Hb617b9fe394cc04976341932ae3d87256285a2654f1c9e6beddf7483::Variant1(
                        HoverOptions {
                            variant0: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                        },
                    ),
                ),
                signature_help_provider: None,
                declaration_provider: None,
                definition_provider: None,
                type_definition_provider: None,
                implementation_provider: None,
                references_provider: None,
                document_highlight_provider: Some(
                    Hf21695c74b3402f0de46005d3e2008486ab02d88f9adaff6b6cce6b2::Variant0(true),
                ),
                document_symbol_provider: None,
                code_action_provider: None,
                code_lens_provider: None,
                document_link_provider: None,
                color_provider: None,
                workspace_symbol_provider: None,
                document_formatting_provider: None,
                document_range_formatting_provider: None,
                document_on_type_formatting_provider: None, /*Some(Box::new(DocumentOnTypeFormattingOptions {
                                                                first_trigger_character: r#"""#.to_string(),
                                                                more_trigger_character: None,
                                                            })),*/
                rename_provider: None,
                folding_range_provider: Some(
                    H07cfb623af7dea337d0e304325abc9453187c524fb5e436547852fdc::Variant0(true),
                ),
                selection_range_provider: None,
                execute_command_provider: None,
                call_hierarchy_provider: None,
                linked_editing_range_provider: None,
                semantic_tokens_provider: Some(
                    Hb33d389f4db33e188f5f7289bda48f700ee05a6244701313be32e552::Variant0(
                        SemanticTokensOptions {
                            legend: SemanticTokensLegend {
                                token_types: vec![
                                    "string".to_string(),
                                    "number".to_string(),
                                    "type".to_string(),
                                    "operator".to_string(),
                                ],
                                token_modifiers: vec![],
                            },
                            variant0: WorkDoneProgressOptions {
                                work_done_progress: None,
                            },
                            range: Some(
                                H3424688d17603d45dbf7bc9bc9337e660ef00dd90b070777859fbf1e::Variant0(
                                    false,
                                ),
                            ),
                            full: Some(
                                tucant_language_server_derive_output::Hdf79c273aed7dd582c079302245431a12e1ba3f63722a25e8cef8db0::Variant0(
                                    true,
                                ),
                            ),
                        },
                    ),
                ),
                moniker_provider: None,
                type_hierarchy_provider: None,
                inline_value_provider: None,
                inlay_hint_provider: None,
                diagnostic_provider: None,
                workspace: None,
                experimental: None,
            },
            server_info: Some(tucant_language_server_derive_output::H880c6487247b4175461832601dd88a01930f42d1e56b2956a0727626 {
                name: "TUCaN't".to_string(),
                version: Some("0.0.1".to_string()),
            }),
        };

        self.send_response(request, result).await?;

        Ok(())
    }

    pub async fn send_request<R: Sendable>(
        self: Arc<Self>,
        request: R::Request,
    ) -> anyhow::Result<R::Response> {
        #[derive(Serialize, Debug)]
        struct TestRequest<T> {
            jsonrpc: String,
            id: String,
            method: String,
            params: T,
        }

        let (tx, rx) = oneshot::channel::<Value>();

        let id: String = thread_rng()
            .sample_iter(&Alphanumeric)
            .take(30)
            .map(char::from)
            .collect();

        let request = TestRequest::<R::Request> {
            jsonrpc: "2.0".to_string(),
            id: id.clone(),
            method: R::name(),
            params: request,
        };

        let mut pending = self.pending.write().await;
        pending.insert(id.clone(), tx);

        let result = serde_json::to_string(&request)?;

        self.tx.send(result).await?;

        Ok(serde_json::from_value(rx.await?).unwrap())
    }

    async fn send_notification<R: SendableAndForget>(
        self: Arc<Self>,
        request: R::Request,
    ) -> anyhow::Result<()> {
        #[derive(Serialize, Debug)]
        struct TestNotification<T> {
            jsonrpc: String,
            method: String,
            params: T,
        }

        let request = TestNotification::<R::Request> {
            jsonrpc: "2.0".to_string(),
            method: R::name(),
            params: request,
        };

        let result = serde_json::to_string(&request)?;

        self.tx.send(result).await?;

        Ok(())
    }

    async fn send_response<R: Receivable + std::marker::Send>(
        self: Arc<Self>,
        request: R,
        response: R::Response,
    ) -> anyhow::Result<()> {
        #[derive(Serialize, Debug)]
        struct TestResponse<T> {
            jsonrpc: String,
            id: StringOrNumber,
            result: T,
        }

        let request = TestResponse::<R::Response> {
            jsonrpc: "2.0".to_string(),
            id: request.id().clone(),
            result: response,
        };

        let result = serde_json::to_string(&request)?;

        self.tx.send(result).await?;

        Ok(())
    }

    async fn handle_sending<
        W: Sink<String, Error = anyhow::Error> + std::marker::Send + std::marker::Unpin,
    >(
        self: Arc<Self>,
        mut sender: W,
        mut rx: mpsc::Receiver<String>,
    ) -> anyhow::Result<()> {
        while let Some(result) = rx.recv().await {
            sender.send(result).await?;
        }

        Ok(())
    }

    async fn main_internal<
        R: Stream<Item = Result<String, anyhow::Error>>
            + std::marker::Unpin
            + std::marker::Send
            + 'static,
        W: Sink<String, Error = anyhow::Error> + std::marker::Unpin + std::marker::Send + 'static,
    >(
        read: R,
        write: W,
    ) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel::<String>(3);

        let arc_self = Arc::new(Self {
            documents: RwLock::new(HashMap::new()),
            pending: RwLock::new(HashMap::new()),
            tx,
        });

        let handle1 = tokio::spawn(arc_self.clone().handle_receiving(read));
        let handle2 = tokio::spawn(arc_self.handle_sending(write, rx));

        handle1.await??;
        handle2.await??;

        Ok(())
    }
}

struct MyStringEncoder;

impl Encoder<String> for MyStringEncoder {
    type Error = anyhow::Error;

    fn encode(&mut self, item: String, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.extend_from_slice(
            format!("Content-Length: {}\r\n\r\n", item.as_bytes().len()).as_bytes(),
        );
        dst.extend_from_slice(item.as_bytes());
        Ok(())
    }
}

struct MyStringDecoder;

impl Decoder for MyStringDecoder {
    type Item = String;
    type Error = anyhow::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // position, iter, split
        let mut it = buf
            .iter()
            .enumerate()
            .filter(|(_position, byte)| **byte == b'\n');
        let start = 0;
        if let Some((position, _)) = it.next() {
            //for (position, _) in it {
            let part = &buf[start..position];

            println!("Part {}", std::str::from_utf8(part).unwrap());

            let (key, value) = part.split(|b| *b == b':').tuples().exactly_one().unwrap();

            assert!(key == b"Content-Length");
            let length_string = std::str::from_utf8(value).unwrap().trim();
            let length = length_string.parse::<usize>().unwrap() + 2;

            println!(
                "len: {}, pos: {}, end: {}",
                buf.len(),
                position,
                position + length + 1
            );
            if position + length + 1 > buf.len() {
                return Ok(None);
            }
            let contents = &buf[position..=(position + length)];

            let return_value = std::str::from_utf8(contents).unwrap().to_string();
            buf.advance(position + length + 1);

            println!("{return_value}");
            return Ok(Some(return_value));

            //start = position;
            //}
        }
        Ok(None)
    }
}

// cargo doc --document-private-items --open
// cargo run -- --port 6008
// cargo watch -x 'run -- --port 6008'
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    match args {
        Args {
            pipe: Some(pipe),
            stdin: false,
            port: None,
            websocket: None,
        } => {
            let stream = UnixStream::connect(pipe).await?;
            let (read, write) = stream.into_split();
            Server::main_internal(
                FramedRead::new(read, MyStringDecoder),
                FramedWrite::new(write, MyStringEncoder),
            )
            .await
        }
        Args {
            port: Some(port),
            pipe: None,
            stdin: false,
            websocket: None,
        } => {
            let stream = TcpListener::bind(("127.0.0.1", port))
                .await?
                .accept()
                .await?
                .0;
            let (read, write) = stream.into_split();
            Server::main_internal(
                FramedRead::new(read, MyStringDecoder),
                FramedWrite::new(write, MyStringEncoder),
            )
            .await
        }
        Args {
            websocket: Some(port),
            pipe: None,
            stdin: false,
            port: None,
        } => {
            let stream = TcpListener::bind(("127.0.0.1", port))
                .await?
                .accept()
                .await?
                .0;
            let ws_stream = tokio_tungstenite::accept_async(stream).await?;
            let (write, read) = ws_stream.split();
            Server::main_internal(
                read.filter_map(|item| {
                    Box::pin(async {
                        match item {
                            Ok(Message::Text(string)) => Some(Ok(string)),
                            _ => None,
                        }
                    })
                }),
                write.with(|v| Box::pin(async { Ok(Message::Text(v)) })),
            )
            .await
        }
        Args { pipe: None, .. } => {
            Server::main_internal(
                FramedRead::new(tokio::io::stdin(), MyStringDecoder),
                FramedWrite::new(tokio::io::stdout(), MyStringEncoder),
            )
            .await
        }
        _ => {
            panic!("can't enable multiple modes at the same time")
        }
    }
}
