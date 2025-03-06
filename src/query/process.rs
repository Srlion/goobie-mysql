use std::ffi::CString;

use anyhow::{bail, Result};
use gmod::{push_to_lua::PushToLua, *};
use sqlx::{
    mysql::MySqlRow,
    types::{
        chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc},
        Decimal,
    },
    Column, Row, TypeInfo, ValueRef as _,
};

#[derive(Debug)]
pub struct ColumnValue {
    pub column_name: CString,
    pub value: lua::Value,
}

impl PushToLua for ColumnValue {
    fn push_to_lua(&self, l: &gmod::State) {
        self.value.push_to_lua(l);
    }
}

pub fn convert_rows(rows: &[MySqlRow]) -> Result<Vec<Vec<ColumnValue>>> {
    rows.iter().map(extract_row_values).collect()
}

pub fn convert_row(row: &Option<MySqlRow>) -> Result<Option<Vec<ColumnValue>>> {
    match row {
        Some(row) => Ok(Some(extract_row_values(row)?)),
        None => Ok(None),
    }
}

fn extract_row_values(row: &MySqlRow) -> Result<Vec<ColumnValue>> {
    let mut values = Vec::with_capacity(row.columns().len());
    for column in row.columns() {
        let name = column.name();
        let col_type = column.type_info().name();
        let column_name = cstring(name);
        let value = extract_column_value(row, name, col_type)?;
        values.push(ColumnValue { column_name, value });
    }
    Ok(values)
}

fn extract_column_value(
    row: &MySqlRow,
    column_name: &str,
    column_type: &str,
) -> Result<lua::Value> {
    let raw_value = row.try_get_raw(column_name)?;
    if raw_value.is_null() {
        return Ok(lua::Value::Nil);
    }
    let value = match column_type {
        "NULL" => lua::Value::Nil,
        "BOOLEAN" | "BOOL" => {
            let b: bool = row.get(column_name);
            lua::Value::Boolean(b)
        }
        "TINYINT" => {
            let i8: i8 = row.get(column_name);
            lua::Value::F64(i8 as f64)
        }
        "SMALLINT" => {
            let i16: i16 = row.get(column_name);
            lua::Value::F64(i16 as f64)
        }
        "INT" | "INTEGER" => {
            let i32: i32 = row.get(column_name);
            lua::Value::F64(i32 as f64)
        }
        "BIGINT" => {
            let i64: i64 = row.get(column_name);
            lua::Value::I64(i64)
        }
        "TINYINT UNSIGNED" => {
            let u8: u8 = row.get(column_name);
            lua::Value::F64(u8 as f64)
        }
        "SMALLINT UNSIGNED" => {
            let u16: u16 = row.get(column_name);
            lua::Value::F64(u16 as f64)
        }
        "INT UNSIGNED" => {
            let u32: u32 = row.get(column_name);
            lua::Value::F64(u32 as f64)
        }
        "BIGINT UNSIGNED" => {
            let u64: u64 = row.get(column_name);
            lua::Value::U64(u64)
        }
        "FLOAT" => {
            let f32: f32 = row.get(column_name);
            lua::Value::F64(f32 as f64)
        }
        "DOUBLE" => {
            let f64: f64 = row.get(column_name);
            lua::Value::F64(f64)
        }
        "DECIMAL" => {
            let decimal: Decimal = row.get(column_name);
            lua::Value::String(decimal.to_string())
        }
        "TIME" => {
            let time: NaiveTime = row.get(column_name);
            lua::Value::String(time.to_string())
        }
        "DATE" => {
            let date: NaiveDate = row.get(column_name);
            lua::Value::String(date.to_string())
        }
        "DATETIME" => {
            let datetime: NaiveDateTime = row.get(column_name);
            lua::Value::String(datetime.to_string())
        }
        "TIMESTAMP" => {
            let timestamp: DateTime<Utc> = row.get(column_name);
            lua::Value::String(timestamp.to_string())
        }
        "BINARY" | "VARBINARY" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB" | "CHAR"
        | "VARCHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "JSON" | "ENUM" | "SET" => {
            let binary: Vec<u8> = row.get(column_name);
            lua::Value::BinaryString(binary)
        }
        "BIT" => {
            // figure out what to push, string or a vector or a number
            bail!("unsupported type: {:?}", column_type);
        }
        _ => {
            bail!("unsupported column type: {}", column_type);
        }
    };
    Ok(value)
}
