use entity::{audit, audit_transfer_failure, content, AuditResult, SelectionStrategy};
use ethportal_api::{types::query_trace::QueryTrace, utils::bytes::hex_encode};
use glados_core::jsonrpc::{JsonRpcError, PortalClient};
use sea_orm::DatabaseConnection;
use tokio::sync::OwnedSemaphorePermit;
use tracing::{debug, error, info, warn};

use crate::validation::content_is_valid;

#[derive(Debug, Clone)]
struct AuditTaskResult {
    result: AuditResult,
    trace: Option<QueryTrace>,
}

impl AuditTaskResult {
    fn new(result: AuditResult, trace: Option<QueryTrace>) -> Self {
        Self { result, trace }
    }

    /// Returns json of a trace when trace is present and there is some failure.
    fn trace_to_store(&self) -> Option<String> {
        let Some(trace) = &self.trace else {
            return None;
        };

        if self.result == AuditResult::Success && trace.failures.is_empty() {
            return None;
        }

        serde_json::to_string(trace)
            .inspect_err(|err| error!(?err, ?trace, "Failed to serialize trace to json"))
            .ok()
    }
}

#[derive(Debug, Clone)]
pub struct AuditTask {
    pub strategy: SelectionStrategy,
    pub content: content::Model,
}

impl AuditTask {
    /// Performs a single audit task and saves the result.
    ///
    /// The audit permit is released at the end.
    pub async fn perform_audit(
        &self,
        audit_permit: OwnedSemaphorePermit,
        client: PortalClient,
        conn: DatabaseConnection,
    ) {
        debug!(
            strategy = ?self.strategy,
            content.key = hex_encode(&self.content.content_key),
            client.url =? client.api.client,
            "Audit started",
        );

        let task_result = self.get_and_validate_content(&client).await;

        self.save_task_result(&task_result, &client, &conn).await;

        info!(
            strategy = ?self.strategy,
            content.key = hex_encode(&self.content.content_key),
            result = ?task_result.result,
            "Audit finished",
        );

        drop(audit_permit);
    }

    async fn get_and_validate_content(&self, client: &PortalClient) -> AuditTaskResult {
        match client.get_content(&self.content).await {
            Ok((content_bytes, trace)) => {
                AuditTaskResult::new(content_is_valid(&self.content, &content_bytes), trace)
            }
            Err(JsonRpcError::ContentNotFound { trace }) => {
                warn!(
                    content.key = hex_encode(&self.content.content_key),
                    "Content not found."
                );
                AuditTaskResult::new(AuditResult::Failure, trace)
            }
            Err(err) => {
                error!(
                    content.key = hex_encode(&self.content.content_key),
                    %err,
                    "Problem requesting content from Portal node."
                );
                AuditTaskResult::new(AuditResult::Failure, None)
            }
        }
    }

    async fn save_task_result(
        &self,
        task_result: &AuditTaskResult,
        client: &PortalClient,
        conn: &DatabaseConnection,
    ) {
        let audit = audit::create(
            self.content.id,
            client.client.id,
            client.node.id,
            task_result.result,
            self.strategy.clone(),
            task_result.trace_to_store(),
            conn,
        )
        .await;

        let audit: audit::Model = match audit {
            Ok(audit) => audit,
            Err(err) => {
                error!(
                    content.key = hex_encode(&self.content.content_key),
                    %err,
                    "Could not save audit in db."
                );
                return;
            }
        };

        if let Some(trace) = &task_result.trace {
            audit_transfer_failure::store_all_failures(audit.id, trace, conn).await;
        }
    }
}
