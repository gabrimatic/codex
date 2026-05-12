use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::function_pre_tool_use_payload;
use crate::tools::handlers::plan_spec::create_update_plan_tool;
use crate::tools::registry::PreToolUsePayload;
use crate::tools::registry::ToolHandler;
use codex_protocol::config_types::ModeKind;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol::EventMsg;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde_json::Value as JsonValue;

pub struct PlanHandler;

pub struct PlanToolOutput;

const PLAN_UPDATED_MESSAGE: &str = "Plan updated";

impl ToolOutput for PlanToolOutput {
    fn log_preview(&self) -> String {
        PLAN_UPDATED_MESSAGE.to_string()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        let mut output = FunctionCallOutputPayload::from_text(PLAN_UPDATED_MESSAGE.to_string());
        output.success = Some(true);

        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output,
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> JsonValue {
        JsonValue::Object(serde_json::Map::new())
    }
}

impl ToolHandler for PlanHandler {
    type Output = PlanToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("update_plan")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_update_plan_tool())
    }

    fn pre_tool_use_payload(&self, invocation: &ToolInvocation) -> Option<PreToolUsePayload> {
        function_pre_tool_use_payload(invocation)
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            call_id: _,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "update_plan handler received unsupported payload".to_string(),
                ));
            }
        };

        if turn.collaboration_mode.mode == ModeKind::Plan {
            return Err(FunctionCallError::RespondToModel(
                "update_plan is a TODO/checklist tool and is not allowed in Plan mode".to_string(),
            ));
        }

        let args = parse_update_plan_arguments(&arguments)?;
        session
            .send_event(turn.as_ref(), EventMsg::PlanUpdate(args))
            .await;

        Ok(PlanToolOutput)
    }
}

fn parse_update_plan_arguments(arguments: &str) -> Result<UpdatePlanArgs, FunctionCallError> {
    serde_json::from_str::<UpdatePlanArgs>(arguments).map_err(|e| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::tests::make_session_and_context;
    use crate::tools::context::ToolCallSource;
    use crate::tools::hook_names::HookToolName;
    use crate::tools::registry::PreToolUsePayload;
    use crate::turn_diff_tracker::TurnDiffTracker;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    async fn invocation_for_payload(payload: ToolPayload) -> ToolInvocation {
        let (session, turn) = make_session_and_context().await;
        ToolInvocation {
            session: session.into(),
            turn: turn.into(),
            cancellation_token: tokio_util::sync::CancellationToken::new(),
            tracker: Arc::new(Mutex::new(TurnDiffTracker::new())),
            call_id: "call-plan".to_string(),
            tool_name: codex_tools::ToolName::plain("update_plan"),
            source: ToolCallSource::Direct,
            payload,
        }
    }

    #[tokio::test]
    async fn pre_tool_use_payload_emits_parsed_arguments() {
        let arguments = json!({
            "plan": [
                { "step": "Inspect", "status": "in_progress" }
            ]
        });
        let invocation = invocation_for_payload(ToolPayload::Function {
            arguments: arguments.to_string(),
        })
        .await;

        assert_eq!(
            PlanHandler.pre_tool_use_payload(&invocation),
            Some(PreToolUsePayload {
                tool_name: HookToolName::new("update_plan"),
                tool_input: arguments,
            })
        );
    }

    #[tokio::test]
    async fn pre_tool_use_payload_skips_non_function_payloads() {
        let invocation = invocation_for_payload(ToolPayload::Custom {
            input: "ignored".to_string(),
        })
        .await;

        assert_eq!(PlanHandler.pre_tool_use_payload(&invocation), None);
    }
}
