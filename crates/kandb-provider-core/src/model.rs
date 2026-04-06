use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime, PrimitiveDateTime, Time};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamespaceInfo {
    pub id: String,
    pub name: String,
    pub kind: NamespaceKind,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NamespaceKind {
    Database,
    Schema,
    Catalog,
    LogicalDb,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceRef {
    pub namespace_id: String,
    pub resource_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub resource: ResourceRef,
    pub name: String,
    pub kind: ResourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceKind {
    Table,
    View,
    VirtualTable,
    ShadowTable,
    Collection,
    Key,
    Unknown,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResourcesRequest {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub pattern: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListResourcesPage {
    pub items: Vec<ResourceInfo>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadRequest {
    pub limit: Option<u32>,
    pub offset: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMeta {
    pub ordinal: Option<usize>,
    pub name: String,
    pub logical_type: Option<LogicalType>,
    pub native_type: Option<String>,
    pub nullable: Option<bool>,
    pub default_value_sql: Option<String>,
    pub primary_key_ordinal: Option<u32>,
    pub hidden: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalType {
    Bool,
    Int,
    BigInt,
    Float,
    Double,
    Decimal,
    Text,
    Binary,
    Json,
    Date,
    Time,
    DateTime,
    DateTimeTz,
    Array,
    Object,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Option<Vec<FieldMeta>>,
    pub rows: Vec<QueryRow>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QueryRow {
    Fields(Vec<Value>),
    Document(IndexMap<String, Value>),
    Value(Value),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i32),
    BigInt(i64),
    Float(f32),
    Double(f64),
    Decimal(String),
    Text(String),
    Binary(Vec<u8>),
    Array(Vec<Value>),
    Object(IndexMap<String, Value>),
    Date(Date),
    Time(Time),
    DateTime(PrimitiveDateTime),
    DateTimeTz(OffsetDateTime),
    Native(NativeValue),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NativeValue {
    pub type_name: String,
    pub repr: serde_json::Value,
}
