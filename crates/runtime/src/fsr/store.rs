use sqlx::PgPool;
use std::sync::Arc;

/// Manages `pilcrow_fsr` table operations for FSR slot tracking.
#[derive(Debug, Clone)]
pub struct FsrStore {
    pool: Arc<PgPool>,
}

impl FsrStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool: Arc::new(pool) }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Upsert the route-level row (slot = '') for a page that has live.rs.
    pub async fn ensure_route_row(
        &self,
        route: &str,
        promote_after: Option<u32>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO pilcrow_fsr (route, slot, promote_after)
            VALUES ($1, '', $2)
            ON CONFLICT (route, slot) DO NOTHING
            "#,
        )
        .bind(route)
        .bind(promote_after.map(|n| n as i32))
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Upsert a slot row for a specific live field.
    pub async fn upsert_slot(
        &self,
        route: &str,
        slot: &str,
        query_sql: &str,
        query_params: &serde_json::Value,
        depends_on: &[String],
        promote_after: Option<u32>,
        debounce_secs: Option<u32>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO pilcrow_fsr
                (route, slot, query, query_params, depends_on, promote_after, debounce_secs)
            VALUES ($1, $2, $3, $4, $5::text[], $6, $7)
            ON CONFLICT (route, slot) DO UPDATE SET
                query         = EXCLUDED.query,
                query_params  = EXCLUDED.query_params,
                depends_on    = EXCLUDED.depends_on,
                promote_after = EXCLUDED.promote_after,
                debounce_secs = EXCLUDED.debounce_secs
            "#,
        )
        .bind(route)
        .bind(slot)
        .bind(query_sql)
        .bind(query_params)
        .bind(depends_on)
        .bind(promote_after.map(|n| n as i32))
        .bind(debounce_secs.map(|n| n as i32))
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Increment hit count on the route-level row (slot = '').
    ///
    /// Returns `true` if this call just crossed the `promote_after` threshold
    /// for the first time.
    pub async fn increment_hit(&self, route: &str) -> sqlx::Result<bool> {
        let row: Option<(i32, bool, Option<i32>)> = sqlx::query_as(
            r#"
            UPDATE pilcrow_fsr
            SET hit_count = hit_count + 1, last_hit = now()
            WHERE route = $1 AND slot = ''
            RETURNING hit_count, promoted, promote_after
            "#,
        )
        .bind(route)
        .fetch_optional(&*self.pool)
        .await?;

        let Some((hit_count, already_promoted, promote_after)) = row else {
            return Ok(false);
        };

        let just_crossed = if let Some(threshold) = promote_after {
            !already_promoted && hit_count >= threshold
        } else {
            false
        };

        if just_crossed {
            sqlx::query(
                "UPDATE pilcrow_fsr SET promoted = TRUE WHERE route = $1 AND slot = ''",
            )
            .bind(route)
            .execute(&*self.pool)
            .await?;
        }

        Ok(just_crossed)
    }

    /// Mark all `pilcrow_fsr` rows whose `depends_on` contains `dep_key` as stale.
    ///
    /// Returns the distinct affected routes.
    pub async fn invalidate_dep_key(&self, dep_key: &str) -> sqlx::Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            UPDATE pilcrow_fsr
            SET stale = TRUE, version = version + 1
            WHERE depends_on @> ARRAY[$1::text]
              AND slot != ''
            RETURNING route
            "#,
        )
        .bind(dep_key)
        .fetch_all(&*self.pool)
        .await?;

        let mut routes: Vec<String> = rows.into_iter().map(|(r,)| r).collect();
        routes.sort();
        routes.dedup();
        Ok(routes)
    }

    /// Mark all slot rows for a specific route as stale.
    pub async fn invalidate_route(&self, route: &str) -> sqlx::Result<()> {
        sqlx::query(
            r#"
            UPDATE pilcrow_fsr
            SET stale = TRUE, version = version + 1
            WHERE route = $1 AND slot != ''
            "#,
        )
        .bind(route)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Fetch all stale slot rows (not the route-level row) for watcher re-execution.
    pub async fn fetch_stale_slots(&self) -> sqlx::Result<Vec<StaleSlot>> {
        sqlx::query_as(
            r#"
            SELECT route, slot, query, query_params, depends_on, promoted, debounce_secs, html_path, json_path
            FROM pilcrow_fsr
            WHERE stale = TRUE AND slot != ''
            "#,
        )
        .fetch_all(&*self.pool)
        .await
    }

    /// Get html_path and json_path for a route if it has been promoted.
    ///
    /// Returns `Some((html_path, json_path))` when the route-level row exists and
    /// `promoted = TRUE`, or `None` otherwise.
    pub async fn get_promoted_paths(
        &self,
        route: &str,
    ) -> sqlx::Result<Option<(Option<String>, Option<String>)>> {
        let row: Option<(bool, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT promoted, html_path, json_path FROM pilcrow_fsr WHERE route = $1 AND slot = '' AND promoted = TRUE",
        )
        .bind(route)
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.map(|(_, h, j)| (h, j)))
    }

    /// Set html_path and json_path for a route's route-level row (slot = '').
    ///
    /// Called after a route is promoted so the watcher knows where to write
    /// baked artefacts.
    pub async fn set_baked_paths(
        &self,
        route: &str,
        html_path: &str,
        json_path: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE pilcrow_fsr SET html_path = $1, json_path = $2 WHERE route = $3 AND slot = ''",
        )
        .bind(html_path)
        .bind(json_path)
        .bind(route)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }

    /// Mark a slot as no longer stale and bump its version.
    pub async fn mark_fresh(&self, route: &str, slot: &str) -> sqlx::Result<()> {
        sqlx::query(
            "UPDATE pilcrow_fsr SET stale = FALSE, version = version + 1 WHERE route = $1 AND slot = $2",
        )
        .bind(route)
        .bind(slot)
        .execute(&*self.pool)
        .await?;
        Ok(())
    }
}

/// A stale slot fetched for watcher re-execution.
#[derive(Debug, sqlx::FromRow)]
pub struct StaleSlot {
    pub route: String,
    pub slot: String,
    pub query: Option<String>,
    pub query_params: Option<serde_json::Value>,
    pub depends_on: Vec<String>,
    pub promoted: bool,
    pub debounce_secs: Option<i32>,
    pub html_path: Option<String>,
    pub json_path: Option<String>,
}
