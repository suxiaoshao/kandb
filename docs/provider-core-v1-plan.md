# `kandb-provider-core` v1 Design

## Summary

- Add a dedicated workspace crate at `crates/kandb-provider-core`.
- Keep the core abstraction neutral across SQL, MongoDB, and Redis by centering it on `ProviderFactory`, `Connection`, `NamespaceInfo`, and `ResourceInfo`.
- Limit `v1` to connection setup, connection testing, metadata listing, resource reads, and optional text-query capabilities.
- Preserve database-specific type details in metadata and `Value::Native` instead of overfitting the shared `Value` enum.

## Public API

- `ProviderFactory`
  - Owns the typed `Config`
  - Connects with `async fn connect(config) -> Result<Connection>`
  - Verifies credentials with `async fn test_connection(&config) -> Result<()>`
- `ErasedProviderFactory`
  - Adapts typed providers for runtime registration
  - Uses `serde_json::Value` to erase provider-specific config types
- `Connection`
  - `ping`
  - `list_namespaces`
  - `list_resources(namespace_id, request)`
  - `list_fields(resource)`
  - Optional capability accessors for text queries, resource reads, and query building
- Optional capability traits
  - `TextQueryExecutor`
  - `ResourceReader`
  - `TextQueryBuilder`

## Shared Models

- `NamespaceInfo` and `NamespaceKind`
- `ResourceRef`, `ResourceInfo`, and `ResourceKind`
- `ListResourcesRequest` and `ListResourcesPage`
- `ReadRequest`
- `FieldMeta` and `LogicalType`
- `QueryResult`
  - `columns: Option<Vec<FieldMeta>>`
  - `rows: Vec<QueryRow>`
- `QueryRow`
  - `Fields(Vec<Value>)`
  - `Document(IndexMap<String, Value>)`
  - `Value(Value)`
- `Value`
  - Scalars: `Null`, `Bool`, `Integer`, `BigInt`, `Float`, `Double`, `Decimal`, `Text`, `Binary`
  - Structured: `Array`, `Object`
  - Time: `Date`, `Time`, `DateTime`, `DateTimeTz`
  - Escape hatch: `Native(NativeValue)`

## Provider Mapping

### SQL

- `namespace` maps to database or schema
- `resource` maps to table or view
- `list_fields` returns full column metadata
- `read_resource` returns `QueryRow::Fields`
- Text queries are first-class

### MongoDB

- `namespace` maps to database
- `resource` maps to collection
- `list_fields` may return `None`
- `read_resource` returns `QueryRow::Document`
- Text queries are optional in `v1`

### Redis

- `namespace` maps to logical database
- `resource` maps to key
- `list_resources` must support pagination to align with `SCAN`
- `list_fields` may expose synthetic fields such as `key`, `type`, `ttl`, and `value_preview`
- `read_resource` returns `QueryRow::Document`
- Redis may be projected as a table in the UI, but the core model keeps it as a key-oriented resource model

## Errors

- `ProviderError`
- `ProviderErrorKind`
  - `InvalidConfig`
  - `Connect`
  - `Authenticate`
  - `Ping`
  - `Metadata`
  - `Query`
  - `UnsupportedCapability`
  - `UnsupportedValue`
  - `Timeout`

## Tests

- Typed config to erased config roundtrip
- Runtime registry registration and connect flow
- SQL duplicate-column result preservation
- Mongo-style document reads with missing fixed schema
- Redis-style paged resource listing and document reads
- Unsupported capability handling
- `Value` serde coverage for time, decimal, and native variants
