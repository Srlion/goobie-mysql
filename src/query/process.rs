use anyhow::{bail, Result};
use gmod::*;
use sqlx::{
    mysql::{MySqlQueryResult, MySqlRow},
    types::{
        chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc},
        Decimal,
    },
    Column, Row, TypeInfo, ValueRef as _,
};

pub fn process_info(l: lua::State, info: MySqlQueryResult) -> Result<i32> {
    l.create_table(0, 2);
    {
        l.push_number(info.rows_affected());
        l.set_field(-2, c"rows_affected");

        l.push_number(info.last_insert_id());
        l.set_field(-2, c"last_insert_id");
    }

    Ok(1)
}

pub fn process_rows(l: lua::State, rows: &[MySqlRow]) -> Result<i32> {
    l.create_table(rows.len() as i32, 0);

    for (idx, row) in rows.iter().enumerate() {
        push_row_to_lua(l, row)?;
        l.raw_seti(-2, idx as i32 + 1);
    }

    Ok(1)
}

pub fn process_row(l: lua::State, row: Option<MySqlRow>) -> Result<i32> {
    match row {
        Some(row) => {
            push_row_to_lua(l, &row)?;
            Ok(1)
        }
        None => {
            l.push_nil();
            Ok(1)
        }
    }
}

fn push_row_to_lua(l: lua::State, row: &MySqlRow) -> Result<()> {
    l.create_table(0, row.len() as i32);

    for column in row.columns() {
        let column_name = column.name();
        let column_type = column.type_info().name();
        push_column_value_to_lua(l, row, column_name, column_type)?;
        l.set_field(-2, &cstring(column_name));
    }

    Ok(())
}

fn push_column_value_to_lua(
    l: lua::State,
    row: &MySqlRow,
    column_name: &str,
    column_type: &str,
) -> Result<()> {
    let value = row.try_get_raw(column_name)?;
    if value.is_null() {
        l.push_nil();
        return Ok(());
    }

    match column_type {
        "NULL" => l.push_nil(),
        "BOOLEAN" | "BOOL" => {
            let b: bool = row.get(column_name);
            l.push_boolean(b);
        }
        "TINYINT" => {
            let i8: i8 = row.get(column_name);
            l.push_number(i8);
        }
        "SMALLINT" => {
            let i16: i16 = row.get(column_name);
            l.push_number(i16);
        }
        "INT" | "INTEGER" => {
            let i32: i32 = row.get(column_name);
            l.push_number(i32);
        }
        "BIGINT" => {
            let i64: i64 = row.get(column_name);
            l.push_number(i64);
        }
        "TINYINT UNSIGNED" => {
            let u8: u8 = row.get(column_name);
            l.push_number(u8);
        }
        "SMALLINT UNSIGNED" => {
            let u16: u16 = row.get(column_name);
            l.push_number(u16);
        }
        "INT UNSIGNED" => {
            let u32: u32 = row.get(column_name);
            l.push_number(u32);
        }
        "BIGINT UNSIGNED" => {
            let u64: u64 = row.get(column_name);
            l.push_number(u64);
        }
        "FLOAT" => {
            let f32: f32 = row.get(column_name);
            l.push_number(f32);
        }
        "DOUBLE" => {
            let f64: f64 = row.get(column_name);
            l.push_number(f64);
        }
        "DECIMAL" => {
            let decimal: Decimal = row.get(column_name);
            l.push_string(&decimal.to_string());
        }
        "TIME" => {
            let time: NaiveTime = row.get(column_name);
            l.push_string(&time.to_string());
        }
        "DATE" => {
            let date: NaiveDate = row.get(column_name);
            l.push_string(&date.to_string());
        }
        "DATETIME" => {
            let datetime: NaiveDateTime = row.get(column_name);
            l.push_string(&datetime.to_string());
        }
        "TIMESTAMP" => {
            let timestamp: DateTime<Utc> = row.get(column_name);
            l.push_string(&timestamp.to_string());
        }
        "BINARY" | "VARBINARY" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB" | "CHAR"
        | "VARCHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" | "JSON" | "ENUM" | "SET" => {
            let binary: Vec<u8> = row.get(column_name);
            l.push_binary_string(&binary);
        }
        "BIT" => {
            // figure out what to push, string or a vector or a number
            bail!("unsupported type: {:?}", column_type);
        }
        _ => {
            bail!("unsupported column type: {}", column_type);
        }
    }

    Ok(())
}
