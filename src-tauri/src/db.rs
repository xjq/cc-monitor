use rusqlite::{Connection, OpenFlags};
use std::path::Path;

use crate::models::UsageSummary;

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
}