#[derive(Debug, Clone)]
pub enum Param {
    Number(i32),
    String(String),
    Boolean(bool),
}
