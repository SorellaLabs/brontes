pub mod errors;
pub(crate) mod serialize;
pub mod types;

use serde::{Deserialize, Serialize};
use std::env;

use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;

use self::errors::DatabaseError;

pub struct InspectorDataClient {
    client: Client,
}

impl Default for InspectorDataClient {
    fn default() -> Self {
        let clickhouse_path = format!(
            "{}:{}",
            &env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
            &env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
        );

        // builds the https connector
        let https = HttpsConnector::new();
        let https_client = hyper::Client::builder().build::<_, hyper::Body>(https);

        // builds the clickhouse client
        let client = Client::with_http_client(https_client)
            .with_url(clickhouse_path)
            .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
            .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
            .with_database(
                env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
            );
        Self { client }
    }
}

impl InspectorDataClient {
    pub async fn insert_one<T: Row + Serialize>(
        &self,
        query: T,
        table_db: &str,
    ) -> Result<(), DatabaseError> {
        let mut insert =
            self.client.insert(table_db).map_err(|e| DatabaseError::InsertError(Box::new(e)))?;

        insert.write(&query).await.map_err(|e| DatabaseError::InsertError(Box::new(e)))?;

        insert.end().await.map_err(|e| DatabaseError::InsertError(Box::new(e)))?;

        Ok(())
    }

    pub async fn insert_many<T: Row + Serialize>(
        &self,
        rows: Vec<T>,
        table_db: &str,
    ) -> Result<(), DatabaseError> {
        let mut insert =
            self.client.insert(&table_db).map_err(|e| DatabaseError::InsertError(Box::new(e)))?;

        for row in rows {
            insert.write(&row).await.map_err(|e| DatabaseError::InsertError(Box::new(e)))?;
        }

        insert.end().await.map_err(|e| DatabaseError::InsertError(Box::new(e)))?;

        Ok(())
    }

    pub async fn query_one<T: Row + for<'b> Deserialize<'b>>(
        &self,
        query: &str,
        params: Vec<&str>,
    ) -> Result<T, DatabaseError> {
        let mut query = self.client.query(query);
        for param in params {
            query = query.clone().bind(param);
        }

        let res =
            query.fetch_one::<T>().await.map_err(|e| DatabaseError::QueryError(Box::new(e)))?;

        Ok(res)
    }

    pub async fn query_all<T: Row + for<'b> Deserialize<'b>>(
        &self,
        query: &str,
        params: Vec<&str>,
    ) -> Result<Vec<T>, DatabaseError> {
        let mut query = self.client.query(query);
        for param in params {
            query = query.clone().bind(param);
        }

        let res =
            query.fetch_all::<T>().await.map_err(|e| DatabaseError::QueryError(Box::new(e)))?;

        Ok(res)
    }

    pub async fn execute(&self, query: &str, params: Vec<&str>) -> Result<(), DatabaseError> {
        let mut query = self.client.query(query);
        for param in params {
            query = query.clone().bind(param);
        }

        query.execute().await.map_err(|e| DatabaseError::QueryError(Box::new(e)))?;

        Ok(())
    }
}
