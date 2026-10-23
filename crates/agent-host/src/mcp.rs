use std::fs::File;
use std::io::{self, Read};
use std::net::Ipv4Addr;
use std::sync::Arc;

use agent_tools::ToolCall;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use futures::channel::{mpsc, oneshot};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
    ToolAnnotations,
};
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, RoleServer, ServerHandler};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::schema::input_schema;

const UNAVAILABLE: &str = "Levare no está disponible.";
const CANCELLED: &str = "La llamada se canceló.";

/// One tool call for the UI thread to run, and where its answer goes.
pub struct ToolInvocation {
    pub call: ToolCall,
    pub reply: oneshot::Sender<Result<String, String>>,
}

/// Levare's tools served over streamable HTTP on loopback, for the agent
/// process to connect to with `Authorization: Bearer <token>`. Stops when
/// dropped.
pub struct McpServer {
    pub url: String,
    pub token: String,
    shutdown: CancellationToken,
}

impl McpServer {
    /// Starts serving on the current tokio runtime. Every tool call is sent
    /// through `tools` and answered through its `reply`.
    pub async fn start(tools: mpsc::UnboundedSender<ToolInvocation>) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let token = random_token()?;
        let shutdown = CancellationToken::new();

        // The default config keeps rmcp's check that `Host` is a loopback name.
        let service = StreamableHttpService::new(
            move || {
                Ok(LevareTools {
                    tools: tools.clone(),
                })
            },
            Arc::new(LocalSessionManager::default()),
            StreamableHttpServerConfig::default().with_cancellation_token(shutdown.child_token()),
        );
        let router = axum::Router::new().nest_service("/mcp", service).layer(
            middleware::from_fn_with_state(Arc::<str>::from(format!("Bearer {token}")), authorize),
        );
        tokio::spawn({
            let shutdown = shutdown.clone();
            async move {
                let _ = axum::serve(listener, router)
                    .with_graceful_shutdown(shutdown.cancelled_owned())
                    .await;
            }
        });

        Ok(Self {
            url: format!("http://{address}/mcp"),
            token,
            shutdown,
        })
    }
}

impl Drop for McpServer {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

/// Lets through only requests that carry the server's bearer token.
async fn authorize(State(expected): State<Arc<str>>, request: Request, next: Next) -> Response {
    let given = request
        .headers()
        .get(header::AUTHORIZATION)
        .map(|value| value.as_bytes());
    if given.is_some_and(|given| same_secret(given, expected.as_bytes())) {
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// Compares in time independent of where the first difference is.
fn same_secret(given: &[u8], expected: &[u8]) -> bool {
    given.len() == expected.len()
        && given
            .iter()
            .zip(expected)
            .fold(0, |difference, (a, b)| difference | (a ^ b))
            == 0
}

/// 32 random bytes in hex.
fn random_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The MCP side of one session: lists the tools and forwards each call.
#[derive(Clone)]
struct LevareTools {
    tools: mpsc::UnboundedSender<ToolInvocation>,
}

impl LevareTools {
    /// Sends the call to the UI thread and waits for its answer. If the
    /// agent cancels the request first, the answer's receiver is dropped, so
    /// the UI side sees `reply.is_canceled()` and can skip the call.
    async fn run(&self, call: ToolCall, cancelled: &CancellationToken) -> Result<String, String> {
        let (reply, answer) = oneshot::channel();
        self.tools
            .unbounded_send(ToolInvocation { call, reply })
            .map_err(|_| UNAVAILABLE.to_string())?;
        tokio::select! {
            answer = answer => answer.unwrap_or_else(|_| Err(UNAVAILABLE.into())),
            () = cancelled.cancelled() => Err(CANCELLED.into()),
        }
    }
}

impl ServerHandler for LevareTools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("levare", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        // Read-only tools tell the agent it may run them without asking.
        let tools = agent_tools::tool_descriptions()
            .into_iter()
            .map(|(name, description)| {
                let tool = Tool::new(name, description, input_schema(name));
                if agent_tools::READ_ONLY_TOOLS.contains(&name) {
                    tool.annotate(ToolAnnotations::new().read_only(true))
                } else {
                    tool
                }
            })
            .collect();
        Ok(ListToolsResult::with_all_items(tools))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let arguments = serde_json::Value::Object(request.arguments.unwrap_or_default());
        let reply = match ToolCall::parse(&request.name, arguments) {
            Ok(call) => self.run(call, &context.ct).await,
            Err(error) => Err(error),
        };
        let result = match reply {
            Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
            Err(error) => CallToolResult::error(vec![ContentBlock::text(error)]),
        };
        Ok(result.into())
    }
}
