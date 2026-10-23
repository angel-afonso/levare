use agent_host::{McpServer, Runtime, ToolInvocation};
use futures::StreamExt;
use futures::channel::mpsc;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ClientConfig};
use rmcp::service::{RoleClient, RunningService};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use std::time::Duration;

/// Stands in for the UI thread: answers every invocation with `reply`.
fn answering(reply: Result<String, String>) -> mpsc::UnboundedSender<ToolInvocation> {
    let (tools, mut invocations) = mpsc::unbounded::<ToolInvocation>();
    std::thread::spawn(move || {
        futures::executor::block_on(async move {
            while let Some(invocation) = invocations.next().await {
                let _ = invocation.reply.send(reply.clone());
            }
        })
    });
    tools
}

async fn connect(server: &McpServer) -> RunningService<RoleClient, ClientConfig> {
    let transport = StreamableHttpClientTransport::from_config(
        StreamableHttpClientTransportConfig::with_uri(server.url.clone())
            .auth_header(server.token.clone()),
    );
    ClientConfig::default().serve(transport).await.unwrap()
}

fn text(result: &CallToolResult) -> &str {
    result.content[0].as_text().unwrap().text.as_str()
}

#[tokio::test]
async fn lists_levare_tools() {
    let server = McpServer::start(answering(Ok(String::new())))
        .await
        .unwrap();
    let client = connect(&server).await;

    let tools = client.list_all_tools().await.unwrap();

    let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_ref()).collect();
    let expected: Vec<&str> = agent_tools::tool_descriptions()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(names, expected);
    let read_only: Vec<&str> = tools
        .iter()
        .filter(|tool| {
            tool.annotations
                .as_ref()
                .and_then(|annotations| annotations.read_only_hint)
                == Some(true)
        })
        .map(|tool| tool.name.as_ref())
        .collect();
    assert_eq!(read_only, ["resumen", "leer", "seleccionar", "comandos"]);
    for tool in &tools {
        assert_eq!(
            tool.input_schema.get("type"),
            Some(&serde_json::json!("object")),
            "{}",
            tool.name
        );
        for combinator in ["oneOf", "anyOf", "allOf"] {
            assert!(
                !tool.input_schema.contains_key(combinator),
                "{} has a root {combinator}",
                tool.name
            );
        }
    }
}

#[tokio::test]
async fn ajustar_schema_is_one_object_with_every_tipo() {
    let server = McpServer::start(answering(Ok(String::new())))
        .await
        .unwrap();
    let client = connect(&server).await;

    let tools = client.list_all_tools().await.unwrap();

    let ajustar = tools.iter().find(|tool| tool.name == "ajustar").unwrap();
    let schema = serde_json::Value::Object(ajustar.input_schema.as_ref().clone());
    let tipos: Vec<&str> = schema["properties"]["tipo"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tipo| tipo.as_str().unwrap())
        .collect();
    assert_eq!(
        tipos,
        [
            "compas_tiempo",
            "tonalidad",
            "tempo",
            "clave",
            "anadir_partes",
            "quitar_parte",
            "conexion",
            "texto",
            "titulo"
        ]
    );
    assert_eq!(schema["required"], serde_json::json!(["tipo"]));
    assert!(!schema.as_object().unwrap().contains_key("$defs"));
    assert!(!schema.to_string().contains("$ref"));
    let numerador = schema["properties"]["numerador"]["description"]
        .as_str()
        .unwrap();
    assert!(
        numerador.contains("Obligatorio con «tipo» compas_tiempo"),
        "{numerador}"
    );
}

#[tokio::test]
async fn ajustar_arguments_reach_the_ui_as_their_variant() {
    let (tools, mut invocations) = mpsc::unbounded::<ToolInvocation>();
    std::thread::spawn(move || {
        futures::executor::block_on(async move {
            while let Some(invocation) = invocations.next().await {
                let _ = invocation.reply.send(Ok(format!("{:?}", invocation.call)));
            }
        })
    });
    let server = McpServer::start(tools).await.unwrap();
    let client = connect(&server).await;
    let arguments = serde_json::json!({
        "tipo": "compas_tiempo",
        "compas": 5,
        "numerador": 3,
        "denominador": 4,
    });

    let result = client
        .call_tool(
            CallToolRequestParams::new("ajustar")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    assert_eq!(
        text(&result),
        "Ajustar(CompasTiempo { compas: 5, numerador: 3, denominador: 4 })"
    );
}

#[tokio::test]
async fn call_round_trips_through_channel() {
    let server = McpServer::start(answering(Ok("hecho".into())))
        .await
        .unwrap();
    let client = connect(&server).await;

    let result = client
        .call_tool(CallToolRequestParams::new("resumen"))
        .await
        .unwrap();

    assert_ne!(result.is_error, Some(true));
    assert_eq!(text(&result), "hecho");
}

#[tokio::test]
async fn tool_error_is_flagged() {
    let server = McpServer::start(answering(Err("x".into()))).await.unwrap();
    let client = connect(&server).await;

    let result = client
        .call_tool(CallToolRequestParams::new("resumen"))
        .await
        .unwrap();

    assert_eq!(result.is_error, Some(true));
    assert_eq!(text(&result), "x");
}

#[tokio::test]
async fn unavailable_ui_is_a_tool_error() {
    let (closed, invocations) = mpsc::unbounded::<ToolInvocation>();
    drop(invocations);
    let (unanswered, mut invocations) = mpsc::unbounded::<ToolInvocation>();
    std::thread::spawn(move || {
        futures::executor::block_on(async move { while invocations.next().await.is_some() {} })
    });

    for tools in [closed, unanswered] {
        let server = McpServer::start(tools).await.unwrap();
        let client = connect(&server).await;

        let result = client
            .call_tool(CallToolRequestParams::new("resumen"))
            .await
            .unwrap();

        assert_eq!(result.is_error, Some(true));
        assert_eq!(text(&result), "Levare no está disponible.");
    }
}

#[tokio::test]
async fn cancelled_call_drops_the_reply() {
    let (tools, mut invocations) = mpsc::unbounded::<ToolInvocation>();
    let server = McpServer::start(tools).await.unwrap();
    let client = connect(&server).await;

    // Dropping the client's call future sends `notifications/cancelled`.
    let call = client.call_tool(CallToolRequestParams::new("resumen"));
    let mut invocation = tokio::select! {
        _ = call => panic!("the call was answered without the UI"),
        invocation = invocations.next() => invocation.unwrap(),
    };

    tokio::time::timeout(Duration::from_secs(5), invocation.reply.cancellation())
        .await
        .expect("the server never dropped the reply's receiver");
    assert!(invocation.reply.is_canceled());
}

#[tokio::test]
async fn rejects_missing_token() {
    let server = McpServer::start(answering(Ok(String::new())))
        .await
        .unwrap();
    let request = || {
        reqwest::Client::new()
            .post(&server.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
    };

    let missing = request().send().await.unwrap();
    let wrong = request().bearer_auth("0".repeat(64)).send().await.unwrap();

    assert_eq!(missing.status(), 401);
    assert_eq!(wrong.status(), 401);
}

#[tokio::test]
async fn rejects_foreign_host() {
    let server = McpServer::start(answering(Ok(String::new())))
        .await
        .unwrap();

    let response = reqwest::Client::new()
        .post(&server.url)
        .bearer_auth(&server.token)
        .header("Host", "levare.example")
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .body(r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 403);
}

#[test]
fn runtime_runs_on_named_threads_and_drops_without_waiting() {
    let runtime = Runtime::new().unwrap();

    let name = runtime.handle().block_on(async {
        tokio::spawn(async { std::thread::current().name().map(str::to_owned) })
            .await
            .unwrap()
    });
    runtime.handle().spawn(std::future::pending::<()>());
    drop(runtime);

    assert_eq!(name.as_deref(), Some("levare-agent"));
}
