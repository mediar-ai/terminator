//! Storage backend for traces and baselines

#[cfg(feature = "sqlite")]
use crate::{error::Result, trace::Trace, session::HumanBaseline};
#[cfg(feature = "sqlite")]
use sqlx::{sqlite::SqlitePool, Pool, Sqlite};

/// Storage backend for persisting observability data
#[cfg(feature = "sqlite")]
#[derive(Debug)]
pub struct Storage {
    pool: Pool<Sqlite>,
}

#[cfg(feature = "sqlite")]
impl Storage {
    /// Create a new storage instance
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = SqlitePool::connect(database_url).await?;
        
        // Create tables if they don't exist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS traces (
                id TEXT PRIMARY KEY,
                task_name TEXT NOT NULL,
                start_time TEXT NOT NULL,
                duration_ms INTEGER NOT NULL,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await?;
        
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS baselines (
                id TEXT PRIMARY KEY,
                task_name TEXT NOT NULL UNIQUE,
                average_duration_ms INTEGER NOT NULL,
                sample_count INTEGER NOT NULL,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&pool)
        .await?;
        
        Ok(Self { pool })
    }
    
    /// Save a trace
    pub async fn save_trace(&self, trace: &Trace) -> Result<()> {
        let data = serde_json::to_string(trace)?;
        
        sqlx::query(
            r#"
            INSERT INTO traces (id, task_name, start_time, duration_ms, data)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&trace.id.to_string())
        .bind(&trace.task_name)
        .bind(&trace.start_time.to_rfc3339())
        .bind(trace.duration.as_millis() as i64)
        .bind(&data)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Load a trace by ID
    pub async fn load_trace(&self, id: &str) -> Result<Option<Trace>> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT data FROM traces WHERE id = ?"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some((data,)) => {
                let trace = serde_json::from_str(&data)?;
                Ok(Some(trace))
            }
            None => Ok(None),
        }
    }
    
    /// Save a baseline
    pub async fn save_baseline(&self, baseline: &HumanBaseline) -> Result<()> {
        let data = serde_json::to_string(baseline)?;
        
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO baselines 
            (id, task_name, average_duration_ms, sample_count, data)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(&baseline.task_name)
        .bind(baseline.average_duration.as_millis() as i64)
        .bind(baseline.sample_count as i64)
        .bind(&data)
        .execute(&self.pool)
        .await?;
        
        Ok(())
    }
    
    /// Load a baseline by task name
    pub async fn load_baseline(&self, task_name: &str) -> Result<Option<HumanBaseline>> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT data FROM baselines WHERE task_name = ?"
        )
        .bind(task_name)
        .fetch_optional(&self.pool)
        .await?;
        
        match row {
            Some((data,)) => {
                let baseline = serde_json::from_str(&data)?;
                Ok(Some(baseline))
            }
            None => Ok(None),
        }
    }
    
    /// Close the storage connection
    pub async fn close(self) -> Result<()> {
        self.pool.close().await;
        Ok(())
    }
}