use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatabaseError {
    #[error("database connection error: {0}")]
    ConnectionError(String),
    #[error("error building the query: {0}")]
    QueryBuildingError(String),
    #[error("error inserting into the database: {0}")]
    InsertError(Box<dyn std::error::Error>),
    #[error("error querying from the database: {0}")]
    QueryError(Box<dyn std::error::Error>),
}
