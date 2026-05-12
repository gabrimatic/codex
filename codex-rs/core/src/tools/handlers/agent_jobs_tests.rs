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
        call_id: "call-agent-jobs".to_string(),
        tool_name: codex_tools::ToolName::plain(tool_name),
        source: ToolCallSource::Direct,
        payload,
    }
}

#[test]
fn parse_csv_supports_quotes_and_commas() {
    let input = "id,name\n1,\"alpha, beta\"\n2,gamma\n";
    let (headers, rows) = parse_csv(input).expect("csv parse");
    assert_eq!(headers, vec!["id".to_string(), "name".to_string()]);
    assert_eq!(
        rows,
        vec![
            vec!["1".to_string(), "alpha, beta".to_string()],
            vec!["2".to_string(), "gamma".to_string()]
        ]
    );
}

#[test]
fn csv_escape_quotes_when_needed() {
    assert_eq!(csv_escape("simple"), "simple");
    assert_eq!(csv_escape("a,b"), "\"a,b\"");
    assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
}

#[test]
fn render_instruction_template_expands_placeholders_and_escapes_braces() {
    let row = json!({
        "path": "src/lib.rs",
        "area": "test",
        "file path": "docs/readme.md",
    });
    let rendered = render_instruction_template(
        "Review {path} in {area}. Also see {file path}. Use {{literal}}.",
        &row,
    );
    assert_eq!(
        rendered,
        "Review src/lib.rs in test. Also see docs/readme.md. Use {literal}."
    );
}

#[test]
fn render_instruction_template_leaves_unknown_placeholders() {
    let row = json!({
        "path": "src/lib.rs",
    });
    let rendered = render_instruction_template("Check {path} then {missing}", &row);
    assert_eq!(rendered, "Check src/lib.rs then {missing}");
}

#[test]
fn ensure_unique_headers_rejects_duplicates() {
    let headers = vec!["path".to_string(), "path".to_string()];
    let Err(err) = ensure_unique_headers(headers.as_slice()) else {
        panic!("expected duplicate header error");
    };
    assert_eq!(
        err,
        FunctionCallError::RespondToModel("csv header path is duplicated".to_string())
    );
}

#[tokio::test]
async fn spawn_agents_on_csv_pre_tool_use_payload_emits_canonical_tool_name() {
    let arguments = json!({
        "csv_path": "work.csv",
        "instruction": "Review {path}",
    });
    let invocation = invocation_for_payload(
        "spawn_agents_on_csv",
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    )
    .await;

    assert_eq!(
        SpawnAgentsOnCsvHandler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("spawn_agents_on_csv"),
            tool_input: arguments,
        })
    );
}

#[tokio::test]
async fn report_agent_job_result_pre_tool_use_payload_emits_canonical_tool_name() {
    let arguments = json!({
        "job_id": "job-1",
        "item_id": "row-1",
        "result": { "ok": true },
    });
    let invocation = invocation_for_payload(
        "report_agent_job_result",
        ToolPayload::Function {
            arguments: arguments.to_string(),
        },
    )
    .await;

    assert_eq!(
        ReportAgentJobResultHandler.pre_tool_use_payload(&invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("report_agent_job_result"),
            tool_input: arguments,
        })
    );
}

#[tokio::test]
async fn agent_jobs_pre_tool_use_payload_skips_non_function_payloads() {
    let invocation = invocation_for_payload(
        "spawn_agents_on_csv",
        ToolPayload::Custom {
            input: "ignored".to_string(),
        },
    )
    .await;

    assert_eq!(
        SpawnAgentsOnCsvHandler.pre_tool_use_payload(&invocation),
        None
    );
}
