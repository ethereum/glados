use chrono::Utc;
use entity::{audit_stats, content::SubProtocol};
use glados_core::stats::{
    filter_audits, get_audit_stats, AuditFilters, ContentTypeFilter, Period, StrategyFilter,
    SuccessFilter,
};
use sea_orm::{DatabaseConnection, DbErr};
use tokio::time::{interval, Duration};
use tracing::{debug, error};

/// Loops indefinitely, periodically recording audit stats to the database.
pub async fn periodically_record_stats(period: Duration, conn: DatabaseConnection) -> ! {
    debug!("initializing task for logging audit stats");
    let mut interval = interval(period);

    loop {
        record_current_stats(&conn).await.unwrap_or_else(|e| {
            error!("failed to record audit stats: {e}");
        });
        interval.tick().await;
    }
}

/// Records audit stats for the current moment to the database.
/// Calculates success rate for many combinations of strategy and content type.
async fn record_current_stats(conn: &DatabaseConnection) -> Result<(), DbErr> {
    // Run audit stat queries in parallel.
    let (
        history_all,
        history_sync,
        history_random,
        history_all_bodies,
        history_all_receipts,
        history_sync_bodies,
        history_sync_receipts,
        history_random_bodies,
        history_random_receipts,
    ) = tokio::join!(
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::All,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Sync,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Random,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::All,
                content_type: ContentTypeFilter::Bodies,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::All,
                content_type: ContentTypeFilter::Receipts,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Sync,
                content_type: ContentTypeFilter::Bodies,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Sync,
                content_type: ContentTypeFilter::Receipts,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Random,
                content_type: ContentTypeFilter::Bodies,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Random,
                content_type: ContentTypeFilter::Receipts,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
    );

    // Handle errors and get success rates.
    let success_rate_history_all = history_all?.pass_percent;
    let success_rate_history_sync = history_sync?.pass_percent;
    let success_rate_history_random = history_random?.pass_percent;
    let success_rate_history_all_bodies = history_all_bodies?.pass_percent;
    let success_rate_history_all_receipts = history_all_receipts?.pass_percent;
    let success_rate_history_sync_bodies = history_sync_bodies?.pass_percent;
    let success_rate_history_sync_receipts = history_sync_receipts?.pass_percent;
    let success_rate_history_random_bodies = history_random_bodies?.pass_percent;
    let success_rate_history_random_receipts = history_random_receipts?.pass_percent;
    // Record the values.
    match audit_stats::create(
        Utc::now(),
        success_rate_history_all,
        success_rate_history_sync,
        success_rate_history_random,
        success_rate_history_all_bodies,
        success_rate_history_all_receipts,
        success_rate_history_sync_bodies,
        success_rate_history_sync_receipts,
        success_rate_history_random_bodies,
        success_rate_history_random_receipts,
        conn,
    )
    .await
    {
        Ok(_) => debug!("successfully recorded audit stats"),
        Err(e) => error!("failed to record audit stats: {e}",),
    };
    Ok(())
}
