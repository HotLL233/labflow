use bytes::BytesMut;
use postgres::{Client, Config, NoTls};
use postgres_types::{IsNull, Type};
use regex::Regex;
use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::sync::OnceLock;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("query returned no rows")]
    QueryReturnedNoRows,
    #[error("PostgreSQL error: {0}")]
    Database(String),
    #[error("parameter conversion error: {0}")]
    Parameter(String),
    #[error("unsupported compatibility operation: {0}")]
    Unsupported(String),
    #[error("SQL conversion error: {0}")]
    ToSqlConversionFailure(Box<dyn std::error::Error + Send + Sync>),
}

impl From<postgres::Error> for Error {
    fn from(value: postgres::Error) -> Self {
        let detail = value
            .as_db_error()
            .map(|db| {
                let mut message = db.message().to_string();
                if let Some(detail) = db.detail() {
                    message.push_str(": ");
                    message.push_str(detail);
                }
                if let Some(hint) = db.hint() {
                    message.push_str("; hint: ");
                    message.push_str(hint);
                }
                message
            })
            .unwrap_or_else(|| value.to_string());
        Self::Database(detail)
    }
}

impl From<r2d2::Error> for Error {
    fn from(value: r2d2::Error) -> Self {
        Self::Database(value.to_string())
    }
}

#[derive(Clone, Debug)]
pub enum ParamValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl postgres_types::ToSql for ParamValue {
    fn to_sql(
        &self,
        ty: &Type,
        out: &mut BytesMut,
    ) -> std::result::Result<IsNull, Box<dyn std::error::Error + Sync + Send>> {
        match self {
            Self::Null => Ok(IsNull::Yes),
            Self::Integer(value) => match *ty {
                Type::INT2 => (*value as i16).to_sql(ty, out),
                Type::INT4 => (*value as i32).to_sql(ty, out),
                Type::FLOAT4 => (*value as f32).to_sql(ty, out),
                Type::FLOAT8 => (*value as f64).to_sql(ty, out),
                Type::BOOL => (*value != 0).to_sql(ty, out),
                _ => value.to_sql(ty, out),
            },
            Self::Real(value) => match *ty {
                Type::FLOAT4 => (*value as f32).to_sql(ty, out),
                Type::INT8 => (*value as i64).to_sql(ty, out),
                _ => value.to_sql(ty, out),
            },
            Self::Text(value) => match *ty {
                Type::INT2 => value.parse::<i16>()?.to_sql(ty, out),
                Type::INT4 => value.parse::<i32>()?.to_sql(ty, out),
                Type::INT8 => value.parse::<i64>()?.to_sql(ty, out),
                Type::FLOAT4 => value.parse::<f32>()?.to_sql(ty, out),
                Type::FLOAT8 => value.parse::<f64>()?.to_sql(ty, out),
                Type::BOOL => matches!(value.as_str(), "1" | "true" | "TRUE").to_sql(ty, out),
                _ => value.to_sql(ty, out),
            },
            Self::Blob(value) => value.to_sql(ty, out),
        }
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
    postgres_types::to_sql_checked!();
}

pub mod types {
    pub use crate::{ToSql, ValueRef};
}

pub trait ToSql: Send + Sync {
    fn to_param(&self) -> ParamValue;
}

macro_rules! integer_param {
    ($($ty:ty),+ $(,)?) => {$(
        impl ToSql for $ty {
            fn to_param(&self) -> ParamValue { ParamValue::Integer(*self as i64) }
        }
    )+};
}
integer_param!(i8, i16, i32, i64, isize, u8, u16, u32, usize);

impl ToSql for u64 {
    fn to_param(&self) -> ParamValue {
        ParamValue::Integer((*self).min(i64::MAX as u64) as i64)
    }
}
impl ToSql for f32 {
    fn to_param(&self) -> ParamValue {
        ParamValue::Real(*self as f64)
    }
}
impl ToSql for f64 {
    fn to_param(&self) -> ParamValue {
        ParamValue::Real(*self)
    }
}
impl ToSql for bool {
    fn to_param(&self) -> ParamValue {
        ParamValue::Integer(i64::from(*self))
    }
}
impl ToSql for String {
    fn to_param(&self) -> ParamValue {
        ParamValue::Text(self.clone())
    }
}
impl ToSql for str {
    fn to_param(&self) -> ParamValue {
        ParamValue::Text(self.to_owned())
    }
}
impl ToSql for Vec<u8> {
    fn to_param(&self) -> ParamValue {
        ParamValue::Blob(self.clone())
    }
}
impl ToSql for [u8] {
    fn to_param(&self) -> ParamValue {
        ParamValue::Blob(self.to_vec())
    }
}
impl ToSql for std::borrow::Cow<'_, str> {
    fn to_param(&self) -> ParamValue {
        ParamValue::Text(self.to_string())
    }
}
impl ToSql for () {
    fn to_param(&self) -> ParamValue {
        ParamValue::Null
    }
}
impl<T: ToSql + ?Sized> ToSql for &T {
    fn to_param(&self) -> ParamValue {
        (*self).to_param()
    }
}
impl<T: ToSql> ToSql for Option<T> {
    fn to_param(&self) -> ParamValue {
        self.as_ref()
            .map(ToSql::to_param)
            .unwrap_or(ParamValue::Null)
    }
}
impl<T: ToSql + ?Sized> ToSql for Box<T> {
    fn to_param(&self) -> ParamValue {
        (**self).to_param()
    }
}

#[derive(Clone, Debug, Default)]
pub struct ParamsList(pub Vec<ParamValue>);

pub trait Params {
    fn into_values(self) -> Vec<ParamValue>;
}
impl Params for ParamsList {
    fn into_values(self) -> Vec<ParamValue> {
        self.0
    }
}
impl Params for &ParamsList {
    fn into_values(self) -> Vec<ParamValue> {
        self.0.clone()
    }
}
impl Params for [(); 0] {
    fn into_values(self) -> Vec<ParamValue> {
        Vec::new()
    }
}
macro_rules! array_params {
    ($($size:expr),+ $(,)?) => {$(
        impl<T: ToSql> Params for [T; $size] {
            fn into_values(self) -> Vec<ParamValue> { self.iter().map(ToSql::to_param).collect() }
        }
    )+};
}
array_params!(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16);
impl<T: ToSql> Params for Vec<T> {
    fn into_values(self) -> Vec<ParamValue> {
        self.iter().map(ToSql::to_param).collect()
    }
}
impl<T: ToSql> Params for &[T] {
    fn into_values(self) -> Vec<ParamValue> {
        self.iter().map(ToSql::to_param).collect()
    }
}
macro_rules! tuple_params {
    ($($name:ident),+ $(,)?) => {
        impl<$($name: ToSql),+> Params for ($($name,)+) {
            #[allow(non_snake_case)]
            fn into_values(self) -> Vec<ParamValue> {
                let ($($name,)+) = self;
                vec![$($name.to_param(),)+]
            }
        }
    };
}
tuple_params!(A);
tuple_params!(A, B);
tuple_params!(A, B, C);
tuple_params!(A, B, C, D);
tuple_params!(A, B, C, D, E);
tuple_params!(A, B, C, D, E, F);
tuple_params!(A, B, C, D, E, F, G);
tuple_params!(A, B, C, D, E, F, G, H);
tuple_params!(A, B, C, D, E, F, G, H, I);
tuple_params!(A, B, C, D, E, F, G, H, I, J);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K, L);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K, L, M);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K, L, M, N);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O);
tuple_params!(A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P);

pub fn params_from_iter<I>(iter: I) -> ParamsList
where
    I: IntoIterator,
    I::Item: ToSql,
{
    ParamsList(iter.into_iter().map(|value| value.to_param()).collect())
}

#[macro_export]
macro_rules! params {
    () => { $crate::ParamsList::default() };
    ($($value:expr),+ $(,)?) => {{
        let mut values = Vec::new();
        $(values.push($crate::ToSql::to_param(&$value));)+
        $crate::ParamsList(values)
    }};
}

#[derive(Clone, Debug)]
pub enum OwnedValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(Vec<u8>),
    Blob(Vec<u8>),
}

#[derive(Clone, Copy, Debug)]
pub enum ValueRef<'a> {
    Null,
    Integer(i64),
    Real(f64),
    Text(&'a [u8]),
    Blob(&'a [u8]),
}

pub trait RowIndex {
    fn index(&self, row: &Row<'_>) -> Result<usize>;
}
impl RowIndex for usize {
    fn index(&self, _row: &Row<'_>) -> Result<usize> {
        Ok(*self)
    }
}
impl RowIndex for i32 {
    fn index(&self, _row: &Row<'_>) -> Result<usize> {
        Ok(*self as usize)
    }
}
impl RowIndex for &str {
    fn index(&self, row: &Row<'_>) -> Result<usize> {
        row.names
            .iter()
            .position(|value| value == self)
            .ok_or_else(|| Error::Database(format!("unknown column {self}")))
    }
}

pub trait FromValue: Sized {
    fn from_value(value: &OwnedValue) -> Result<Self>;
}
impl FromValue for i64 {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        match value {
            OwnedValue::Integer(v) => Ok(*v),
            OwnedValue::Real(v) => Ok(*v as i64),
            OwnedValue::Text(v) => String::from_utf8_lossy(v)
                .parse()
                .map_err(|e| Error::Parameter(format!("{e}"))),
            _ => Err(Error::Parameter("expected integer".into())),
        }
    }
}
impl FromValue for i32 {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        Ok(i64::from_value(value)? as i32)
    }
}
impl FromValue for usize {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        Ok(i64::from_value(value)? as usize)
    }
}
impl FromValue for f64 {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        match value {
            OwnedValue::Real(v) => Ok(*v),
            OwnedValue::Integer(v) => Ok(*v as f64),
            OwnedValue::Text(v) => String::from_utf8_lossy(v)
                .parse()
                .map_err(|e| Error::Parameter(format!("{e}"))),
            _ => Err(Error::Parameter("expected real".into())),
        }
    }
}
impl FromValue for bool {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        Ok(i64::from_value(value)? != 0)
    }
}
impl FromValue for String {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        match value {
            OwnedValue::Text(v) => Ok(String::from_utf8_lossy(v).into_owned()),
            OwnedValue::Integer(v) => Ok(v.to_string()),
            OwnedValue::Real(v) => Ok(v.to_string()),
            OwnedValue::Blob(v) => Ok(String::from_utf8_lossy(v).into_owned()),
            OwnedValue::Null => Err(Error::Parameter("expected text".into())),
        }
    }
}
impl FromValue for Vec<u8> {
    fn from_value(value: &OwnedValue) -> Result<Self> {
        match value {
            OwnedValue::Blob(v) | OwnedValue::Text(v) => Ok(v.clone()),
            _ => Err(Error::Parameter("expected bytes".into())),
        }
    }
}

macro_rules! optional_value {
    ($($ty:ty),+ $(,)?) => {$(
        impl FromValue for Option<$ty> {
            fn from_value(value: &OwnedValue) -> Result<Self> { match value { OwnedValue::Null => Ok(None), _ => <$ty>::from_value(value).map(Some) } }
        }
    )+};
}
optional_value!(i64, i32, usize, f64, bool, String, Vec<u8>);

pub struct Row<'row> {
    names: Vec<String>,
    values: Vec<OwnedValue>,
    marker: PhantomData<&'row ()>,
}
impl Row<'_> {
    fn from_pg(row: postgres::Row) -> Result<Row<'static>> {
        let mut names = Vec::new();
        let mut values = Vec::new();
        for (index, column) in row.columns().iter().enumerate() {
            names.push(column.name().to_owned());
            let value = match *column.type_() {
                Type::INT2 => row
                    .try_get::<_, Option<i16>>(index)?
                    .map(|v| OwnedValue::Integer(v as i64))
                    .unwrap_or(OwnedValue::Null),
                Type::INT4 => row
                    .try_get::<_, Option<i32>>(index)?
                    .map(|v| OwnedValue::Integer(v as i64))
                    .unwrap_or(OwnedValue::Null),
                Type::INT8 => row
                    .try_get::<_, Option<i64>>(index)?
                    .map(OwnedValue::Integer)
                    .unwrap_or(OwnedValue::Null),
                Type::FLOAT4 => row
                    .try_get::<_, Option<f32>>(index)?
                    .map(|v| OwnedValue::Real(v as f64))
                    .unwrap_or(OwnedValue::Null),
                Type::FLOAT8 | Type::NUMERIC => row
                    .try_get::<_, Option<f64>>(index)?
                    .map(OwnedValue::Real)
                    .unwrap_or(OwnedValue::Null),
                Type::BOOL => row
                    .try_get::<_, Option<bool>>(index)?
                    .map(|v| OwnedValue::Integer(i64::from(v)))
                    .unwrap_or(OwnedValue::Null),
                Type::BYTEA => row
                    .try_get::<_, Option<Vec<u8>>>(index)?
                    .map(OwnedValue::Blob)
                    .unwrap_or(OwnedValue::Null),
                _ => row
                    .try_get::<_, Option<String>>(index)?
                    .map(|v| OwnedValue::Text(v.into_bytes()))
                    .unwrap_or(OwnedValue::Null),
            };
            values.push(value);
        }
        Ok(Row {
            names,
            values,
            marker: PhantomData,
        })
    }

    pub fn get<I: RowIndex, T: FromValue>(&self, index: I) -> Result<T> {
        let index = index.index(self)?;
        let value = self
            .values
            .get(index)
            .ok_or_else(|| Error::Database(format!("column index {index} out of range")))?;
        T::from_value(value)
    }

    pub fn get_ref<I: RowIndex>(&self, index: I) -> Result<ValueRef<'_>> {
        let index = index.index(self)?;
        Ok(
            match self
                .values
                .get(index)
                .ok_or_else(|| Error::Database(format!("column index {index} out of range")))?
            {
                OwnedValue::Null => ValueRef::Null,
                OwnedValue::Integer(v) => ValueRef::Integer(*v),
                OwnedValue::Real(v) => ValueRef::Real(*v),
                OwnedValue::Text(v) => ValueRef::Text(v),
                OwnedValue::Blob(v) => ValueRef::Blob(v),
            },
        )
    }
}

fn convert_sql(input: &str) -> String {
    static DATETIME_VALUE: OnceLock<Regex> = OnceLock::new();
    static DATE_VALUE: OnceLock<Regex> = OnceLock::new();
    static CHAR_VALUE: OnceLock<Regex> = OnceLock::new();
    static NULL_ID: OnceLock<Regex> = OnceLock::new();
    let mut sql = input
        .replace("INSERT OR IGNORE INTO", "INSERT INTO")
        .replace("COLLATE NOCASE", "");
    sql = sql.replace(
        "datetime('now','localtime')",
        "to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')",
    );
    sql = sql.replace(
        "datetime('now')",
        "to_char(CURRENT_TIMESTAMP, 'YYYY-MM-DD HH24:MI:SS')",
    );
    sql = sql.replace("IFNULL(", "COALESCE(");
    sql = DATETIME_VALUE
        .get_or_init(|| Regex::new(r"datetime\(([A-Za-z_][A-Za-z0-9_.]*)\)").unwrap())
        .replace_all(&sql, "$1")
        .into_owned();
    sql = DATE_VALUE
        .get_or_init(|| Regex::new(r"date\(([A-Za-z_][A-Za-z0-9_.]*)\)").unwrap())
        .replace_all(&sql, "LEFT($1, 10)")
        .into_owned();
    sql = CHAR_VALUE
        .get_or_init(|| Regex::new(r"(?i)\bchar\(").unwrap())
        .replace_all(&sql, "chr(")
        .into_owned();
    sql = NULL_ID
        .get_or_init(|| Regex::new(r"\?(\d+) IS NULL").unwrap())
        .replace_all(&sql, "CAST(?$1 AS BIGINT) IS NULL")
        .into_owned();
    sql = convert_parameters(&sql);
    if input.to_ascii_uppercase().contains("INSERT OR IGNORE INTO")
        && !sql.to_ascii_uppercase().contains("ON CONFLICT")
    {
        let trimmed = sql.trim_end().trim_end_matches(';');
        sql = format!("{trimmed} ON CONFLICT DO NOTHING");
    }
    sql
}

fn convert_parameters(sql: &str) -> String {
    let chars: Vec<char> = sql.chars().collect();
    let mut result = String::with_capacity(sql.len());
    let mut index = 0;
    let mut next_parameter = 1usize;
    let mut quote: Option<char> = None;

    while index < chars.len() {
        let current = chars[index];
        if let Some(active_quote) = quote {
            result.push(current);
            if current == active_quote {
                if index + 1 < chars.len() && chars[index + 1] == active_quote {
                    result.push(chars[index + 1]);
                    index += 1;
                } else {
                    quote = None;
                }
            }
            index += 1;
            continue;
        }

        if current == '\'' || current == '"' {
            quote = Some(current);
            result.push(current);
            index += 1;
            continue;
        }

        if current == '?' {
            let mut digit_index = index + 1;
            while digit_index < chars.len() && chars[digit_index].is_ascii_digit() {
                digit_index += 1;
            }
            if digit_index > index + 1 {
                let number: String = chars[index + 1..digit_index].iter().collect();
                let position = number.parse::<usize>().unwrap_or(next_parameter);
                result.push('$');
                result.push_str(&position.to_string());
                next_parameter = next_parameter.max(position + 1);
                index = digit_index;
            } else {
                result.push('$');
                result.push_str(&next_parameter.to_string());
                next_parameter += 1;
                index += 1;
            }
            continue;
        }

        result.push(current);
        index += 1;
    }

    result
}

fn pg_params(values: &[ParamValue]) -> Vec<&(dyn postgres_types::ToSql + Sync)> {
    values
        .iter()
        .map(|value| value as &(dyn postgres_types::ToSql + Sync))
        .collect()
}

type DbTask = Box<dyn FnOnce(&mut Client) + Send + 'static>;

pub struct Connection {
    sender: mpsc::Sender<DbTask>,
    last_insert_id: Cell<i64>,
}

impl fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("PostgreSQLConnection").finish()
    }
}

impl Connection {
    pub fn connect(database_url: &str) -> Result<Self> {
        let database_url = database_url.to_owned();
        let (task_sender, task_receiver) = mpsc::channel::<DbTask>();
        let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("workload-postgres-connection".into())
            .spawn(move || {
                let result = database_url
                    .parse::<Config>()
                    .map_err(|error| Error::Database(error.to_string()))
                    .and_then(|config| config.connect(NoTls).map_err(Error::from));
                match result {
                    Ok(mut client) => {
                        let _ = ready_sender.send(Ok(()));
                        while let Ok(task) = task_receiver.recv() {
                            task(&mut client);
                        }
                    }
                    Err(error) => {
                        let _ = ready_sender.send(Err(error));
                    }
                }
            })
            .map_err(|error| Error::Database(error.to_string()))?;
        ready_receiver
            .recv()
            .map_err(|error| Error::Database(error.to_string()))??;
        Ok(Self {
            sender: task_sender,
            last_insert_id: Cell::new(0),
        })
    }
    pub fn open_test_database() -> Result<Self> {
        r2d2::ManageConnection::connect(&ConnectionManager::test_database()?)
    }
    fn run<T, F>(&self, operation: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Client) -> Result<T> + Send + 'static,
    {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .send(Box::new(move |client| {
                let _ = sender.send(operation(client));
            }))
            .map_err(|error| Error::Database(error.to_string()))?;
        receiver
            .recv()
            .map_err(|error| Error::Database(error.to_string()))?
    }
    pub fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        let sql = convert_sql(sql);
        let values = params.into_values();
        let insert = sql
            .trim_start()
            .to_ascii_uppercase()
            .starts_with("INSERT INTO");
        let (affected, last_id) = self.run(move |client| {
            let params = pg_params(&values);
            let affected = client.execute(&sql, &params)? as usize;
            let last_id = if insert {
                client
                    .query_one("SELECT lastval()", &[])
                    .ok()
                    .map(|row| row.get::<_, i64>(0))
            } else {
                None
            };
            Ok((affected, last_id))
        })?;
        if let Some(last_id) = last_id {
            self.last_insert_id.set(last_id);
        }
        Ok(affected)
    }
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        let statements: Vec<String> = sql
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(convert_sql)
            .collect();
        self.run(move |client| {
            for statement in statements {
                client.batch_execute(&statement)?;
            }
            Ok(())
        })
    }
    pub fn prepare(&self, sql: &str) -> Result<Statement<'_>> {
        let sql = convert_sql(sql);
        let prepare_sql = sql.clone();
        let columns = self.run(move |client| {
            let statement = client.prepare(&prepare_sql).map_err(|error| {
                if std::env::var("WORKLOAD_DEBUG_SQL").is_ok() {
                    Error::Database(format!("{error}; SQL: {prepare_sql}"))
                } else {
                    Error::from(error)
                }
            })?;
            Ok(statement
                .columns()
                .iter()
                .map(|column| column.name().to_owned())
                .collect())
        })?;
        Ok(Statement {
            connection: self,
            sql,
            columns,
        })
    }
    pub fn query_row<P, F, T>(&self, sql: &str, params: P, mapper: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let mut statement = self.prepare(sql)?;
        statement.query_row(params, mapper)
    }
    pub fn transaction(&mut self) -> Result<Transaction<'_>> {
        Transaction::begin(self)
    }
    pub fn unchecked_transaction(&self) -> Result<Transaction<'_>> {
        Transaction::begin(self)
    }
    pub fn last_insert_rowid(&self) -> i64 {
        self.last_insert_id.get()
    }
}

pub struct Transaction<'a> {
    connection: &'a Connection,
    complete: Cell<bool>,
}
impl<'a> Transaction<'a> {
    fn begin(connection: &'a Connection) -> Result<Self> {
        connection.run(|client| {
            client.batch_execute("BEGIN")?;
            Ok(())
        })?;
        Ok(Self {
            connection,
            complete: Cell::new(false),
        })
    }
    pub fn execute<P: Params>(&self, sql: &str, params: P) -> Result<usize> {
        self.connection.execute(sql, params)
    }
    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        self.connection.execute_batch(sql)
    }
    pub fn prepare(&self, sql: &str) -> Result<Statement<'_>> {
        self.connection.prepare(sql)
    }
    pub fn query_row<P, F, T>(&self, sql: &str, params: P, mapper: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        self.connection.query_row(sql, params, mapper)
    }
    pub fn last_insert_rowid(&self) -> i64 {
        self.connection.last_insert_rowid()
    }
    pub fn commit(self) -> Result<()> {
        self.connection.run(|client| {
            client.batch_execute("COMMIT")?;
            Ok(())
        })?;
        self.complete.set(true);
        Ok(())
    }
}
impl Drop for Transaction<'_> {
    fn drop(&mut self) {
        if !self.complete.get() {
            let _ = self.connection.run(|client| {
                client.batch_execute("ROLLBACK")?;
                Ok(())
            });
        }
    }
}
impl Deref for Transaction<'_> {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        self.connection
    }
}

pub struct Statement<'a> {
    connection: &'a Connection,
    sql: String,
    columns: Vec<String>,
}
impl<'a> Statement<'a> {
    fn fetch<P: Params>(&mut self, params: P) -> Result<Vec<Row<'static>>> {
        let values = params.into_values();
        let sql = self.sql.clone();
        self.connection.run(move |client| {
            let params = pg_params(&values);
            let rows = client.query(&sql, &params).map_err(|error| {
                if std::env::var("WORKLOAD_DEBUG_SQL").is_ok() {
                    Error::Database(format!("{error}; SQL: {sql}"))
                } else {
                    Error::from(error)
                }
            })?;
            rows
                .into_iter()
                .map(Row::from_pg)
                .collect()
        })
    }
    pub fn query_map<P, F, T>(&mut self, params: P, mapper: F) -> Result<MappedRows<T>>
    where
        P: Params,
        F: FnMut(&Row<'_>) -> Result<T>,
    {
        let mut mapper = mapper;
        let rows: Vec<Result<T>> = self.fetch(params)?.iter().map(|row| mapper(row)).collect();
        Ok(MappedRows {
            inner: rows.into_iter(),
        })
    }
    pub fn query_row<P, F, T>(&mut self, params: P, mapper: F) -> Result<T>
    where
        P: Params,
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let rows = self.fetch(params)?;
        rows.first()
            .ok_or(Error::QueryReturnedNoRows)
            .and_then(mapper)
    }
    pub fn query<P: Params>(&mut self, params: P) -> Result<Rows> {
        Ok(Rows {
            rows: self.fetch(params)?,
            index: 0,
        })
    }
    pub fn execute<P: Params>(&mut self, params: P) -> Result<usize> {
        self.connection.execute(&self.sql, params)
    }
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
    pub fn column_name(&self, index: usize) -> Result<&str> {
        self.columns
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| Error::Database(format!("column index {index} out of range")))
    }
}

pub struct MappedRows<T> {
    inner: std::vec::IntoIter<Result<T>>,
}
impl<T> Iterator for MappedRows<T> {
    type Item = Result<T>;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

pub struct Rows {
    rows: Vec<Row<'static>>,
    index: usize,
}
impl Rows {
    pub fn next(&mut self) -> Result<Option<&Row<'_>>> {
        if self.index >= self.rows.len() {
            return Ok(None);
        }
        let row = &self.rows[self.index];
        self.index += 1;
        Ok(Some(row))
    }
}

pub trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>>;
}
impl<T> OptionalExtension<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionManager {
    database_url: String,
    search_path: Option<String>,
}
impl ConnectionManager {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            search_path: None,
        }
    }

    pub fn test_database() -> Result<Self> {
        static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(1);
        let database_url = std::env::var("WORKLOAD_TEST_DATABASE_URL").map_err(|_| {
            Error::Unsupported("WORKLOAD_TEST_DATABASE_URL is required for PostgreSQL integration tests".into())
        })?;
        let sequence = NEXT_SCHEMA.fetch_add(1, Ordering::Relaxed);
        let schema = format!("workload_test_{}_{}", std::process::id(), sequence);
        let bootstrap = Connection::connect(&database_url)?;
        bootstrap.execute_batch(&format!("CREATE SCHEMA \"{schema}\""))?;
        Ok(Self {
            database_url,
            search_path: Some(schema),
        })
    }
}
impl r2d2::ManageConnection for ConnectionManager {
    type Connection = Connection;
    type Error = Error;
    fn connect(&self) -> Result<Connection> {
        let connection = Connection::connect(&self.database_url)?;
        if let Some(schema) = &self.search_path {
            connection.execute_batch(&format!("SET search_path TO \"{schema}\""))?;
        }
        Ok(connection)
    }
    fn is_valid(&self, connection: &mut Connection) -> Result<()> {
        connection
            .query_row("SELECT 1", ParamsList::default(), |row| {
                row.get::<_, i64>(0)
            })
            .map(|_| ())
    }
    fn has_broken(&self, connection: &mut Connection) -> bool {
        self.is_valid(connection).is_err()
    }
}

#[cfg(test)]
mod tests {
    use super::convert_sql;

    #[test]
    fn converts_legacy_char_function_without_touching_to_char() {
        let sql = convert_sql("SELECT m.name || char(31) || i.code, to_char(CURRENT_TIMESTAMP, 'YYYY')");
        assert!(sql.contains("m.name || chr(31) || i.code"));
        assert!(sql.contains("to_char(CURRENT_TIMESTAMP, 'YYYY')"));
    }

    #[test]
    fn converts_anonymous_and_numbered_parameters() {
        let sql = convert_sql("SELECT * FROM projects WHERE group_id=? AND id=?2 AND name=?");
        assert_eq!(
            sql,
            "SELECT * FROM projects WHERE group_id=$1 AND id=$2 AND name=$3"
        );
    }

    #[test]
    fn leaves_question_marks_inside_quoted_values_unchanged() {
        let sql = convert_sql("SELECT '?' AS marker, \"question?column\" FROM projects WHERE id=?");
        assert_eq!(
            sql,
            "SELECT '?' AS marker, \"question?column\" FROM projects WHERE id=$1"
        );
    }
}
