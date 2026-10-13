# ADR 0001: Settings state tree

Status: accepted.

Context: the CLI, scripting API, service and sidebar need one description of
case settings that is typed, introspectable, serializable and validated.

Decision: a schema (`rcfd_core::settings::Schema`: bool, int, float with
optional SI dimension and bounds, string, enum with allowed values, list,
typed object, named-object collection, command) plus data stored as
`serde_json::Value` that is *always* validated against the schema on `set`.
Dimensioned floats are stored in SI; unit strings are converted on input.
Access is by dotted path. JSON Schema export is derived from the same schema,
so UIs and external tools never diverge from the code. The content hash is FNV-1a
over key-sorted JSON.

Consequences: dynamic and easy to serialize; type errors are reported at
`set` time with the offending path rather than at compile time.
