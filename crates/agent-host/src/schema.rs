//! The JSON schema each tool advertises for its arguments.

use std::any::Any;
use std::sync::Arc;

use agent_tools::{
    AjustarArgs, ComandosArgs, EjecutarArgs, EscribirArgs, LeerArgs, SeleccionarArgs,
};
use rmcp::handler::server::common::{schema_for_empty_input, schema_for_type};
use rmcp::model::JsonObject;
use schemars::JsonSchema;
use serde_json::{Value, json};

/// The tag of `AjustarArgs`, `#[serde(tag = "tipo")]`.
const TAG: &str = "tipo";

/// The JSON schema of a tool's arguments, as `ToolCall::parse` reads them.
pub fn input_schema(name: &str) -> Arc<JsonObject> {
    match name {
        "resumen" => schema_for_empty_input(),
        "leer" => object_schema::<LeerArgs>(),
        "seleccionar" => object_schema::<SeleccionarArgs>(),
        "comandos" => object_schema::<ComandosArgs>(),
        "ejecutar" => object_schema::<EjecutarArgs>(),
        "escribir" => object_schema::<EscribirArgs>(),
        "ajustar" => object_schema::<AjustarArgs>(),
        other => panic!("no input schema for tool {other:?}"),
    }
}

/// The title is the Rust type's name, noise to the agent.
fn object_schema<T: JsonSchema + Any>() -> Arc<JsonObject> {
    let mut schema = schema_for_type::<T>().as_ref().clone();
    schema.remove("title");
    if schema.contains_key("oneOf") {
        schema = flatten_variants(schema);
    }
    Arc::new(schema)
}

/// A tagged enum's schema is a `oneOf` of objects, which Anthropic's API
/// refuses at the root of a tool's input schema. The variants become one
/// object: `tipo` lists every variant, and each field of each variant is a
/// property that says which tags use it. `ToolCall::parse` still checks the
/// real shape.
fn flatten_variants(mut schema: JsonObject) -> JsonObject {
    let defs = schema.remove("$defs").unwrap_or_default();
    let Some(Value::Array(variants)) = schema.remove("oneOf") else {
        panic!("oneOf is not an array");
    };
    let variants: Vec<Value> = variants
        .into_iter()
        .map(|variant| inline_refs(variant, &defs))
        .collect();

    let mut tags = Vec::new();
    let mut tag_notes = Vec::new();
    let mut fields: Vec<Field> = Vec::new();
    for variant in &variants {
        let properties = variant["properties"]
            .as_object()
            .expect("variant without properties");
        let tag = properties[TAG]["const"].as_str().expect("tag is a string");
        tags.push(tag);
        if let Some(description) = variant["description"].as_str() {
            tag_notes.push(format!("{tag}: {description}"));
        }
        let required = variant["required"].as_array();
        for (name, property) in properties.iter().filter(|(name, _)| *name != TAG) {
            let index = match fields.iter().position(|field| field.name == *name) {
                Some(index) => index,
                None => {
                    fields.push(Field::new(name));
                    fields.len() - 1
                }
            };
            let is_required =
                required.is_some_and(|required| required.contains(&Value::from(name.as_str())));
            fields[index].add(tag, property, is_required);
        }
    }

    let mut properties = JsonObject::new();
    properties.insert(
        TAG.into(),
        json!({
            "type": "string",
            "enum": tags,
            "description": tag_notes.join(" "),
        }),
    );
    for field in fields {
        properties.insert(field.name.clone(), field.into_schema());
    }
    schema.insert("type".into(), "object".into());
    schema.insert("properties".into(), properties.into());
    schema.insert("required".into(), json!([TAG]));
    schema
}

/// Replaces each `{"$ref": "#/$defs/X"}` with the definition of `X`, keeping
/// the keys written beside the reference.
fn inline_refs(value: Value, defs: &Value) -> Value {
    match value {
        Value::Object(mut object) => {
            if let Some(Value::String(reference)) = object.remove("$ref") {
                let name = reference
                    .strip_prefix("#/$defs/")
                    .expect("only local references");
                let Value::Object(mut resolved) = inline_refs(defs[name].clone(), defs) else {
                    panic!("definition {name:?} is not an object");
                };
                for (key, value) in object {
                    resolved.insert(key, inline_refs(value, defs));
                }
                Value::Object(resolved)
            } else {
                object
                    .into_iter()
                    .map(|(key, value)| (key, inline_refs(value, defs)))
                    .collect()
            }
        }
        Value::Array(items) => items
            .into_iter()
            .map(|item| inline_refs(item, defs))
            .collect(),
        other => other,
    }
}

/// One property across the variants that have it.
struct Field {
    name: String,
    /// Each distinct schema, with the tags that use it.
    shapes: Vec<(Value, Vec<String>)>,
    required: Vec<String>,
    optional: Vec<String>,
}

impl Field {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            shapes: Vec::new(),
            required: Vec::new(),
            optional: Vec::new(),
        }
    }

    fn add(&mut self, tag: &str, schema: &Value, required: bool) {
        match self.shapes.iter_mut().find(|(shape, _)| shape == schema) {
            Some((_, tags)) => tags.push(tag.to_string()),
            None => self.shapes.push((schema.clone(), vec![tag.to_string()])),
        }
        if required {
            self.required.push(tag.to_string());
        } else {
            self.optional.push(tag.to_string());
        }
    }

    fn into_schema(self) -> Value {
        let mut schema = if let [(shape, _)] = self.shapes.as_slice() {
            shape.clone()
        } else {
            let shapes: Vec<Value> = self
                .shapes
                .into_iter()
                .map(|(mut shape, tags)| {
                    append_description(&mut shape, &format!("Con «tipo» {}.", tags.join(", ")));
                    shape
                })
                .collect();
            json!({ "anyOf": shapes })
        };
        let mut uses = Vec::new();
        if !self.required.is_empty() {
            uses.push(format!(
                "Obligatorio con «tipo» {}.",
                self.required.join(", ")
            ));
        }
        if !self.optional.is_empty() {
            uses.push(format!("Opcional con «tipo» {}.", self.optional.join(", ")));
        }
        append_description(&mut schema, &uses.join(" "));
        schema
    }
}

fn append_description(schema: &mut Value, note: &str) {
    let description = match schema["description"].as_str() {
        Some(description) => format!("{description} {note}"),
        None => note.to_string(),
    };
    schema["description"] = description.into();
}
