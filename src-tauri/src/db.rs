use rusqlite::{Connection, OpenFlags};
use std::path::Path;

use crate::models::{UsageSummary, TodayDetail, HourBucket, ModelRow, ProviderRow};

/// Open cc-switch's DB strictly read-only.
pub fn open_readonly(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.pragma_update(None, "query_only", true)?;
    Ok(conn)
}

/// Given current unix seconds and local UTC offset (east positive, seconds),
/// return the unix timestamp of the most recent local midnight.
pub fn midnight_unix(now_unix: i64, offset_seconds: i32) -> i64 {
    let offset = offset_seconds as i64;
    let local = now_unix + offset;
    let secs_into_day = local.rem_euclid(86_400);
    local - secs_into_day - offset
}

pub fn midnight_unix_live() -> i64 {
    use chrono::Local;
    let now = Local::now();
    let ts = now.timestamp();
    let off = now.offset().local_minus_utc();
    midnight_unix(ts, off)
}

pub fn fetch_summary(conn: &Connection, since: i64) -> rusqlite::Result<UsageSummary> {
    let mut stmt = conn.prepare(
        "SELECT \
            COALESCE(SUM(input_tokens),0), \
            COALESCE(SUM(output_tokens),0), \
            COALESCE(SUM(cache_read_tokens),0), \
            COALESCE(SUM(cache_creation_tokens),0), \
            COALESCE(SUM(CAST(total_cost_usd AS REAL)),0), \
            COUNT(*), \
            COALESCE(SUM(CASE WHEN CAST(total_cost_usd AS REAL)=0 THEN 1 ELSE 0 END),0) \
         FROM proxy_request_logs WHERE created_at >= ?1",
    )?;
    let s = stmt.query_row(rusqlite::params![since], |r| {
        Ok(UsageSummary {
            input_tokens: r.get(0)?,
            output_tokens: r.get(1)?,
            cache_read_tokens: r.get(2)?,
            cache_creation_tokens: r.get(3)?,
            total_cost_usd: r.get(4)?,
            request_count: r.get(5)?,
            unpriced_rows: r.get(6)?,
        })
    })?;
    Ok(s)
}

pub fn fetch_detail(conn: &Connection, since: i64) -> rusqlite::Result<TodayDetail> {
    let mut hours_stmt = conn.prepare(
        "SELECT strftime('%Y-%m-%dT%H', datetime(created_at,'unixepoch','localtime')) AS hour, \
                COALESCE(SUM(input_tokens+output_tokens+cache_read_tokens+cache_creation_tokens),0), \
                COALESCE(SUM(CAST(total_cost_usd AS REAL)),0) \
         FROM proxy_request_logs WHERE created_at >= ?1 GROUP BY hour ORDER BY hour",
    )?;
    let hours: Vec<HourBucket> = hours_stmt.query_map(rusqlite::params![since], |r| {
        Ok(HourBucket { hour: r.get(0)?, tokens: r.get(1)?, cost: r.get(2)? })
    })?.filter_map(Result::ok).collect();

    let mut model_stmt = conn.prepare(
        "SELECT model, COUNT(*), \
                COALESCE(SUM(input_tokens+output_tokens+cache_read_tokens+cache_creation_tokens),0), \
                COALESCE(SUM(CAST(total_cost_usd AS REAL)),0) \
         FROM proxy_request_logs WHERE created_at >= ?1 GROUP BY model ORDER BY 4 DESC",
    )?;
    let by_model: Vec<ModelRow> = model_stmt.query_map(rusqlite::params![since], |r| {
        Ok(ModelRow { model: r.get::<_, Option<String>>(0)?.unwrap_or_default(), requests: r.get(1)?, tokens: r.get(2)?, cost: r.get(3)? })
    })?.filter_map(Result::ok).collect();

    let mut prov_stmt = conn.prepare(
        "SELECT r.provider_id, COALESCE(p.name, r.provider_id), COUNT(*), \
                COALESCE(SUM(r.input_tokens+r.output_tokens+r.cache_read_tokens+r.cache_creation_tokens),0), \
                COALESCE(SUM(CAST(r.total_cost_usd AS REAL)),0) \
         FROM proxy_request_logs r \
         LEFT JOIN providers p ON p.id = r.provider_id AND p.app_type = r.app_type \
         WHERE r.created_at >= ?1 GROUP BY r.provider_id ORDER BY 5 DESC",
    )?;
    let by_provider: Vec<ProviderRow> = prov_stmt.query_map(rusqlite::params![since], |r| {
        Ok(ProviderRow {
            provider_id: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            requests: r.get(2)?,
            tokens: r.get(3)?,
            cost: r.get(4)?,
        })
    })?.filter_map(Result::ok).collect();

    Ok(TodayDetail { hours, by_model, by_provider })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_db_with_rows() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE proxy_request_logs (
                input_tokens INTEGER, output_tokens INTEGER,
                cache_read_tokens INTEGER, cache_creation_tokens INTEGER,
                total_cost_usd TEXT, created_at INTEGER,
                model TEXT, provider_id TEXT, app_type TEXT)",
            [],
        ).unwrap();
        // since=1000; two rows after, one before, one unpriced (cost 0)
        conn.execute("INSERT INTO proxy_request_logs VALUES (100,10,0,0,'0.05',1200,'a','p1','claude')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (200,20,5,0,'0.10',1300,'b','p1','codex')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (50,5,0,0,'0',900,'c','p2','claude')", []).unwrap(); // before since, unpriced
        conn.execute("INSERT INTO proxy_request_logs VALUES (300,0,0,0,'0',1400,'glm-5.2','p1','claude')", []).unwrap(); // after, unpriced
        conn
    }

    #[test]
    fn midnight_unix_cst_midday() {
        assert_eq!(midnight_unix(1767240000, 28800), 1767196800);
    }
    #[test]
    fn midnight_unix_cst_just_after_midnight() {
        assert_eq!(midnight_unix(1767198600, 28800), 1767196800);
    }
    #[test]
    fn midnight_unix_negative_offset() {
        assert_eq!(midnight_unix(1767304800, -36000), 1767261600);
    }

    #[test]
    fn fetch_summary_sums_only_rows_since() {
        let conn = mem_db_with_rows();
        let s = fetch_summary(&conn, 1000).unwrap();
        assert_eq!(s.input_tokens, 100 + 200 + 300); // excludes the 50-row
        assert_eq!(s.output_tokens, 10 + 20 + 0);
        assert_eq!(s.cache_read_tokens, 0 + 5 + 0);
        assert_eq!(s.request_count, 3);
        assert!((s.total_cost_usd - 0.15).abs() < 1e-9); // 0.05+0.10+0
        assert_eq!(s.unpriced_rows, 1); // only the glm-5.2 row after since
    }

    #[test]
    fn fetch_summary_empty_returns_zeros() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE proxy_request_logs (input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_creation_tokens INTEGER, total_cost_usd TEXT, created_at INTEGER, model TEXT, provider_id TEXT, app_type TEXT)", []).unwrap();
        let s = fetch_summary(&conn, 1000).unwrap();
        assert_eq!(s.request_count, 0);
        assert_eq!(s.total_cost_usd, 0.0);
        assert_eq!(s.unpriced_rows, 0);
    }

    fn mem_db_full() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE proxy_request_logs (input_tokens INTEGER, output_tokens INTEGER, cache_read_tokens INTEGER, cache_creation_tokens INTEGER, total_cost_usd TEXT, created_at INTEGER, model TEXT, provider_id TEXT, app_type TEXT);
             CREATE TABLE providers (id TEXT, app_type TEXT, name TEXT, is_current INTEGER);"
        ).unwrap();
        conn.execute("INSERT INTO providers VALUES ('p1','claude','Bailian',1)", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (100,10,0,0,'0.05',1200,'glm-5.2','p1','claude')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (200,20,5,0,'0.10',1300,'qwen','p1','claude')", []).unwrap();
        conn.execute("INSERT INTO proxy_request_logs VALUES (50,5,0,0,'0',900,'old','p2','codex')", []).unwrap();
        conn
    }

    #[test]
    fn fetch_detail_groups_by_model() {
        let conn = mem_db_full();
        let d = fetch_detail(&conn, 1000).unwrap();
        assert_eq!(d.by_model.len(), 2); // glm-5.2 + qwen (old excluded)
        let total_tokens: i64 = d.by_model.iter().map(|m| m.tokens).sum();
        assert_eq!(total_tokens, (100+10) + (200+20+5));
        let total_cost: f64 = d.by_model.iter().map(|m| m.cost).sum();
        assert!((total_cost - 0.15).abs() < 1e-9);
    }

    #[test]
    fn fetch_detail_joins_provider_name() {
        let conn = mem_db_full();
        let d = fetch_detail(&conn, 1000).unwrap();
        // p1 has a name (Bailian); the row with provider_id from logs joins on (id, app_type)
        let p1 = d.by_provider.iter().find(|r| r.provider_id == "p1").unwrap();
        assert_eq!(p1.name, "Bailian");
        assert_eq!(p1.requests, 2);
    }

    #[test]
    fn fetch_detail_hours_nonempty() {
        let conn = mem_db_full();
        let d = fetch_detail(&conn, 1000).unwrap();
        assert!(!d.hours.is_empty());
        // hour bucket string shape YYYY-MM-DDTHH
        assert!(d.hours[0].hour.len() == 13);
    }
}