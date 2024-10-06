use sqlx::mysql::{MySqlQueryResult, MySqlRow};

#[derive(Debug)]
pub enum QueryType {
    Execute,
    FetchOne,
    FetchAll,
}

#[derive(Debug)]
pub enum QueryResult {
    Execute(MySqlQueryResult),
    Row(Option<MySqlRow>),
    Rows(Vec<MySqlRow>),
}
