use super::*;
use crate::session::tests::make_session_and_context;
use crate::tools::context::ToolCallSource;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::hook_names::HookToolName;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use crate::turn_diff_tracker::TurnDiffTracker;
use pretty_assertions::assert_eq;
use rmcp::model::AnnotateAble;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

async fn invocation_for_payload(tool_name: &str, payload: ToolPayload) -> ToolInvocation {
    let (session, turn) = make_session_and_context().await;
    ToolInvocation {
        session: session.into(),
        turn: turn.into(),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
        call_id: "call-mcp-resource".to_string(),
        tool_name: codex_tools::ToolName::plain(tool_name),
        source: ToolCallSource::Direct,
        payload,
    }
}

fn resource(uri: &str, name: &str) -> Resource {
    rmcp::model::RawResource {
        uri: uri.to_string(),
        name: name.to_string(),
        title: None,
        description: None,
        mime_type: None,
        size: None,
        icons: None,
        meta: None,
    }
    .no_annotation()
}

fn template(uri_template: &str, name: &str) -> ResourceTemplate {
    rmcp::model::RawResourceTemplate {
        uri_template: uri_template.to_string(),
        name: name.to_string(),
        title: None,
        description: None,
        mime_type: None,
        icons: None,
    }
    .no_annotation()
}

#[test]
fn resource_with_server_serializes_server_field() {
    let entry = ResourceWithServer::new("test".to_string(), resource("memo://id", "memo"));
    let value = serde_json::to_value(&entry).expect("serialize resource");

    assert_eq!(value["server"], json!("test"));
    assert_eq!(value["uri"], json!("memo://id"));
    assert_eq!(value["name"], json!("memo"));
}

#[test]
fn list_resources_payload_from_single_server_copies_next_cursor() {
    let result = ListResourcesResult {
        meta: None,
        next_cursor: Some("cursor-1".to_string()),
        resources: vec![resource("memo://id", "memo")],
    };
    let payload = ListResourcesPayload::from_single_server("srv".to_string(), result);
    let value = serde_json::to_value(&payload).expect("serialize payload");

    assert_eq!(value["server"], json!("srv"));
    assert_eq!(value["nextCursor"], json!("cursor-1"));
    let resources = value["resources"].as_array().expect("resources array");
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["server"], json!("srv"));
}

#[test]
fn list_resources_payload_from_all_servers_is_sorted() {
    let mut map = HashMap::new();
    map.insert("beta".to_string(), vec![resource("memo://b-1", "b-1")]);
    map.insert(
        "alpha".to_string(),
        vec![resource("memo://a-1", "a-1"), resource("memo://a-2", "a-2")],
    );

    let payload = ListResourcesPayload::from_all_servers(map);
    let value = serde_json::to_value(&payload).expect("serialize payload");
    let uris: Vec<String> = value["resources"]
        .as_array()
        .expect("resources array")
        .iter()
        .map(|entry| entry["uri"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(
        uris,
        vec![
            "memo://a-1".to_string(),
            "memo://a-2".to_string(),
            "memo://b-1".to_string()
        ]
    );
}

#[test]
fn call_tool_result_from_content_marks_success() {
    let result = call_tool_result_from_content("{}", Some(true));
    assert_eq!(result.is_error, Some(false));
    assert_eq!(result.content.len(), 1);
}

#[test]
fn parse_arguments_handles_empty_and_json() {
    assert!(
        parse_arguments(" \n\t").unwrap().is_none(),
        "expected None for empty arguments"
    );

    assert!(
        parse_arguments("null").unwrap().is_none(),
        "expected None for null arguments"
    );

    let value = parse_arguments(r#"{"server":"figma"}"#)
        .expect("parse json")
        .expect("value present");
    assert_eq!(value["server"], json!("figma"));
}

#[test]
fn template_with_server_serializes_server_field() {
    let entry = ResourceTemplateWithServer::new("srv".to_string(), template("memo://{id}", "memo"));
    let value = serde_json::to_value(&entry).expect("serialize template");

    assert_eq!(
        value,
        json!({
            "server": "srv",
            "uriTemplate": "memo://{id}",
            "name": "memo"
        })
    );
}

#[tokio::test]
async fn list_mcp_resources_pre_tool_use_payload_emits_canonical_tool_name() {
    let arguments = json!({ "server": "memory", "cursor": "page-2" });
    let invocation = invocation_for_payload(
        "list_mcp_resources",
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    )
    .await;

    assert_eq!(
        ListMcpResourcesHandler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("list_mcp_resources"),
            tool_input: arguments,
        })
    );
}

#[tokio::test]
async fn list_mcp_resource_templates_pre_tool_use_payload_emits_canonical_tool_name() {
    let arguments = json!({ "server": "memory" });
    let invocation = invocation_for_payload(
        "list_mcp_resource_templates",
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    )
    .await;

    assert_eq!(
        ListMcpResourceTemplatesHandler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("list_mcp_resource_templates"),
            tool_input: arguments,
        })
    );
}

#[tokio::test]
async fn read_mcp_resource_pre_tool_use_payload_emits_canonical_tool_name() {
    let arguments = json!({ "server": "filesystem", "uri": "file:///etc/hosts" });
    let invocation = invocation_for_payload(
        "read_mcp_resource",
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    )
    .await;

    assert_eq!(
        ReadMcpResourceHandler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("read_mcp_resource"),
            tool_input: arguments,
        })
    );
}

#[tokio::test]
async fn mcp_resource_pre_tool_use_payload_skips_non_function_payloads() {
    let invocation = invocation_for_payload(
        "list_mcp_resources",
        ToolPayload::Custom {
            input: "ignored".to_string(),
        },
    )
    .await;

    assert_eq!(
        ListMcpResourcesHandler.pre_tool_use_payload(&invocation),
        None
    );
}
