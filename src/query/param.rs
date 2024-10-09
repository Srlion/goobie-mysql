#[derive(Debug, Clone)]
pub enum Param {
    Number(i32),
    String(Vec<u8>),
    Boolean(bool),
}
