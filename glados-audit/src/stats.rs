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
        all,
        latest,
        random,
        fourfours,
        all_headers,
        all_bodies,
        all_receipts,
        latest_headers,
        latest_bodies,
        latest_receipts,
        random_headers,
        random_bodies,
        random_receipts,
        fourfours_headers,
        fourfours_bodies,
        fourfours_receipts,
        state_all,
        state_latest,
        state_state_roots,
        beacon_all,
        beacon_latest,
        history_all_headers_by_number,
        history_latest_headers_by_number,
        history_random_headers_by_number,
        history_fourfours_headers_by_number,
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
                strategy: StrategyFilter::Latest,
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
                strategy: StrategyFilter::FourFours,
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
                content_type: ContentTypeFilter::Headers,
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
                strategy: StrategyFilter::Latest,
                content_type: ContentTypeFilter::Headers,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Latest,
                content_type: ContentTypeFilter::Bodies,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Latest,
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
                content_type: ContentTypeFilter::Headers,
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
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::Headers,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::Bodies,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::Receipts,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::All,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::State
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Latest,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::State
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::StateRoots,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::State
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::All,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::Beacon
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Latest,
                content_type: ContentTypeFilter::All,
                success: SuccessFilter::All,
                network: SubProtocol::Beacon
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::All,
                content_type: ContentTypeFilter::HeadersByNumber,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Latest,
                content_type: ContentTypeFilter::HeadersByNumber,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::Random,
                content_type: ContentTypeFilter::HeadersByNumber,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
        get_audit_stats(
            filter_audits(AuditFilters {
                strategy: StrategyFilter::FourFours,
                content_type: ContentTypeFilter::HeadersByNumber,
                success: SuccessFilter::All,
                network: SubProtocol::History
            },),
            Period::Hour,
            conn
        ),
    );

    // Handle errors and get success rates.
    let success_rate_all = all?.pass_percent;
    let success_rate_latest = latest?.pass_percent;
    let success_rate_random = random?.pass_percent;
    let success_rate_oldest = 0.0;
    let success_rate_fourfours = fourfours?.pass_percent;
    let success_rate_all_headers = all_headers?.pass_percent;
    let success_rate_all_bodies = all_bodies?.pass_percent;
    let success_rate_all_receipts = all_receipts?.pass_percent;
    let success_rate_latest_headers = latest_headers?.pass_percent;
    let success_rate_latest_bodies = latest_bodies?.pass_percent;
    let success_rate_latest_receipts = latest_receipts?.pass_percent;
    let success_rate_random_headers = random_headers?.pass_percent;
    let success_rate_random_bodies = random_bodies?.pass_percent;
    let success_rate_random_receipts = random_receipts?.pass_percent;
    let success_rate_fourfours_headers = fourfours_headers?.pass_percent;
    let success_rate_fourfours_bodies = fourfours_bodies?.pass_percent;
    let success_rate_fourfours_receipts = fourfours_receipts?.pass_percent;
    let success_rate_state_all = state_all?.pass_percent;
    let success_rate_state_latest = state_latest?.pass_percent;
    let success_rate_state_state_roots = state_state_roots?.pass_percent;
    let success_rate_beacon_all = beacon_all?.pass_percent;
    let success_rate_beacon_latest = beacon_latest?.pass_percent;
    let success_rate_history_all_headers_by_number = history_all_headers_by_number?.pass_percent;
    let success_rate_history_latest_headers_by_number =
        history_latest_headers_by_number?.pass_percent;
    let success_rate_history_random_headers_by_number =
        history_random_headers_by_number?.pass_percent;
    let success_rate_history_fourfours_headers_by_number =
        history_fourfours_headers_by_number?.pass_percent;

    // Record the values.
    match audit_stats::create(
        Utc::now(),
        0,
        success_rate_all,
        success_rate_latest,
        success_rate_random,
        success_rate_oldest,
        success_rate_fourfours,
        success_rate_all_headers,
        success_rate_all_bodies,
        success_rate_all_receipts,
        success_rate_latest_headers,
        success_rate_latest_bodies,
        success_rate_latest_receipts,
        success_rate_random_headers,
        success_rate_random_bodies,
        success_rate_random_receipts,
        success_rate_fourfours_headers,
        success_rate_fourfours_bodies,
        success_rate_fourfours_receipts,
        success_rate_state_all,
        success_rate_state_latest,
        success_rate_state_state_roots,
        success_rate_beacon_all,
        success_rate_beacon_latest,
        success_rate_history_all_headers_by_number,
        success_rate_history_latest_headers_by_number,
        success_rate_history_random_headers_by_number,
        success_rate_history_fourfours_headers_by_number,
        conn,
    )
    .await
    {
        Ok(_) => debug!("successfully recorded audit stats"),
        Err(e) => error!("failed to record audit stats: {e}",),
    };
    Ok(())
}
