use super::*;

#[derive(Clone)]
pub struct PostgresRepository {
    pub(crate) pool: PgPool,
}

pub struct RunnerSingletonDbLock {
    _conn: PoolConnection<Postgres>,
    lock_key: i64,
}

impl RunnerSingletonDbLock {
    pub fn lock_key(&self) -> i64 {
        self.lock_key
    }
}

impl PostgresRepository {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(15)
            .min_connections(3)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn try_acquire_runner_singleton_lock(
        &self,
        lock_key: i64,
    ) -> Result<Option<RunnerSingletonDbLock>> {
        let mut conn = self.pool.acquire().await?;
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
            .bind(lock_key)
            .fetch_one(&mut *conn)
            .await?;
        if acquired {
            Ok(Some(RunnerSingletonDbLock {
                _conn: conn,
                lock_key,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
