//! Command parameters: a JSON object, checked field by field. The same description
//! ([`ParamSpec`]) drives validation, the JSON schema for agents (MCP) and, later, UI forms.
//!
//! Convention for updates: a missing key keeps the value, `null` clears it ([`Patch`]).

use std::str::FromStr;

use jiff::civil::Date;
use plnwrk_core::{EntityKind, text};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

use crate::CommandError;

/// What kind of value a parameter takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    /// A title: one line, up to 200 characters.
    Title,
    /// A long text (Markdown), up to 20 000 characters.
    LongText,
    /// A short label, up to 80 characters.
    Label,
    /// A calendar day, `YYYY-MM-DD`.
    Date,
    /// The id of an entity.
    Id(EntityKind),
    /// One of a fixed list of names.
    Choice(&'static [&'static str]),
    /// A whole number in a range.
    Integer {
        /// Smallest allowed value.
        min: i64,
        /// Largest allowed value.
        max: i64,
    },
    /// `true` or `false`.
    Bool,
    /// A list of labels (tags).
    Labels,
}

/// One parameter of a command or query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParamSpec {
    /// Key in the JSON object.
    pub name: &'static str,
    /// Value type.
    pub kind: ParamKind,
    /// Must be present (and not `null`).
    pub required: bool,
    /// `null` is accepted (clears the value in updates).
    pub nullable: bool,
    /// One line for people and agents.
    pub help: &'static str,
}

impl ParamSpec {
    /// A parameter that must be given.
    pub const fn required(name: &'static str, kind: ParamKind, help: &'static str) -> Self {
        Self {
            name,
            kind,
            required: true,
            nullable: false,
            help,
        }
    }

    /// A parameter that may be left out.
    pub const fn optional(name: &'static str, kind: ParamKind, help: &'static str) -> Self {
        Self {
            name,
            kind,
            required: false,
            nullable: false,
            help,
        }
    }

    /// A parameter that may be left out (keep) or `null` (clear).
    pub const fn clearable(name: &'static str, kind: ParamKind, help: &'static str) -> Self {
        Self {
            name,
            kind,
            required: false,
            nullable: true,
            help,
        }
    }

    /// The JSON schema of the value.
    pub fn schema(&self) -> Value {
        let mut schema = match self.kind {
            ParamKind::Title => json!({"type": "string", "maxLength": text::TITLE_MAX}),
            ParamKind::LongText => json!({"type": "string", "maxLength": text::BODY_MAX}),
            ParamKind::Label => json!({"type": "string", "maxLength": text::LABEL_MAX}),
            ParamKind::Date => json!({"type": "string", "format": "date"}),
            ParamKind::Id(kind) => json!({
                "type": "string",
                "pattern": "^[0-9A-HJKMNP-TV-Za-hjkmnp-tv-z]{26}$",
                "x-entity": kind,
            }),
            ParamKind::Choice(names) => json!({"type": "string", "enum": names}),
            ParamKind::Integer { min, max } => {
                json!({"type": "integer", "minimum": min, "maximum": max})
            }
            ParamKind::Bool => json!({"type": "boolean"}),
            ParamKind::Labels => json!({
                "type": "array",
                "items": {"type": "string", "maxLength": text::LABEL_MAX},
                "maxItems": text::TAGS_MAX,
            }),
        };
        if let Some(object) = schema.as_object_mut() {
            object.insert("description".to_owned(), Value::from(self.help));
            if self.nullable
                && let Some(kind) = object.get("type").cloned()
            {
                object.insert("type".to_owned(), json!([kind, "null"]));
            }
        }
        schema
    }
}

/// JSON schema of a parameter list: an object without other keys.
pub fn input_schema(params: &[ParamSpec]) -> Value {
    let properties: Map<String, Value> = params
        .iter()
        .map(|p| (p.name.to_owned(), p.schema()))
        .collect();
    let required: Vec<&str> = params
        .iter()
        .filter(|p| p.required)
        .map(|p| p.name)
        .collect();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false,
    })
}

/// What an update does with one field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Patch<T> {
    /// Key missing: leave as is.
    Keep,
    /// `null`: remove the value.
    Clear,
    /// A new value.
    Set(T),
}

impl<T> Patch<T> {
    /// Applies the patch to an optional field.
    pub fn apply_to(self, field: &mut Option<T>) {
        match self {
            Self::Keep => {}
            Self::Clear => *field = None,
            Self::Set(value) => *field = Some(value),
        }
    }
}

/// A parser for one value.
pub type Parse<T> = fn(&str, &Value) -> Result<T, CommandError>;

/// The parameters of one call: a JSON object already checked against the spec's keys.
#[derive(Debug, Clone, Copy)]
pub struct Params<'a> {
    map: &'a Map<String, Value>,
}

impl<'a> Params<'a> {
    /// Checks `value` against `specs`: an object, no unknown keys, required keys present and
    /// not `null`, `null` only where allowed. Value types are checked by the parsers.
    pub fn check(value: &'a Value, specs: &[ParamSpec]) -> Result<Self, CommandError> {
        let map = value
            .as_object()
            .ok_or_else(|| CommandError::bad("(params)", "parameters must be a JSON object"))?;
        for key in map.keys() {
            if !specs.iter().any(|s| s.name == key) {
                let known: Vec<&str> = specs.iter().map(|s| s.name).collect();
                return Err(CommandError::bad(
                    &truncate(key),
                    format!("unknown parameter; known: {}", known.join(", ")),
                ));
            }
        }
        for spec in specs {
            match map.get(spec.name) {
                None | Some(Value::Null) if spec.required => {
                    return Err(CommandError::bad(spec.name, "is required"));
                }
                Some(Value::Null) if !spec.nullable && !spec.required => {
                    return Err(CommandError::bad(spec.name, "must not be null"));
                }
                _ => {}
            }
        }
        Ok(Self { map })
    }

    /// A required value.
    pub fn req<T>(&self, name: &str, parse: Parse<T>) -> Result<T, CommandError> {
        match self.map.get(name) {
            None | Some(Value::Null) => Err(CommandError::bad(name, "is required")),
            Some(value) => parse(name, value),
        }
    }

    /// An optional value: missing or `null` give `None`.
    pub fn opt<T>(&self, name: &str, parse: Parse<T>) -> Result<Option<T>, CommandError> {
        match self.map.get(name) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => parse(name, value).map(Some),
        }
    }

    /// An update: missing keeps, `null` clears, a value sets.
    pub fn patch<T>(&self, name: &str, parse: Parse<T>) -> Result<Patch<T>, CommandError> {
        match self.map.get(name) {
            None => Ok(Patch::Keep),
            Some(Value::Null) => Ok(Patch::Clear),
            Some(value) => parse(name, value).map(Patch::Set),
        }
    }

    /// An update of a field that cannot be empty: `null` is refused.
    pub fn patch_required<T>(
        &self,
        name: &str,
        parse: Parse<T>,
    ) -> Result<Option<T>, CommandError> {
        match self.patch(name, parse)? {
            Patch::Keep => Ok(None),
            Patch::Clear => Err(CommandError::bad(name, "cannot be cleared")),
            Patch::Set(value) => Ok(Some(value)),
        }
    }

    /// Whether the key is present at all.
    pub fn has(&self, name: &str) -> bool {
        self.map.contains_key(name)
    }
}

fn truncate(text: &str) -> String {
    text.chars().take(60).collect()
}

fn string<'v>(name: &str, value: &'v Value) -> Result<&'v str, CommandError> {
    value
        .as_str()
        .ok_or_else(|| CommandError::bad(name, "must be a string"))
}

/// Parses a title.
pub fn title(name: &str, value: &Value) -> Result<String, CommandError> {
    text::title(string(name, value)?).map_err(|e| CommandError::bad(name, e.to_string()))
}

/// Parses a long text.
pub fn body(name: &str, value: &Value) -> Result<String, CommandError> {
    text::body(string(name, value)?).map_err(|e| CommandError::bad(name, e.to_string()))
}

/// Parses a short label.
pub fn label(name: &str, value: &Value) -> Result<String, CommandError> {
    text::label(string(name, value)?).map_err(|e| CommandError::bad(name, e.to_string()))
}

/// Parses a list of tags.
pub fn labels(name: &str, value: &Value) -> Result<Vec<String>, CommandError> {
    let items = value
        .as_array()
        .ok_or_else(|| CommandError::bad(name, "must be a list of strings"))?;
    if items.len() > text::TAGS_MAX * 4 {
        return Err(CommandError::bad(name, "too many entries"));
    }
    let strings = items
        .iter()
        .map(|item| string(name, item).map(str::to_owned))
        .collect::<Result<Vec<_>, _>>()?;
    text::tags(&strings).map_err(|e| CommandError::bad(name, e.to_string()))
}

/// Parses a calendar day `YYYY-MM-DD`.
pub fn date(name: &str, value: &Value) -> Result<Date, CommandError> {
    let text = string(name, value)?;
    let well_formed = text.len() == 10
        && text.char_indices().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !well_formed {
        return Err(CommandError::bad(name, "must be a date like 2026-10-14"));
    }
    text.parse::<Date>()
        .map_err(|_| CommandError::bad(name, "is not a valid calendar day"))
}

/// Parses an id of type `T`.
pub fn id<T: FromStr>(name: &str, value: &Value) -> Result<T, CommandError> {
    string(name, value)?
        .parse()
        .map_err(|_| CommandError::bad(name, "is not a valid id"))
}

/// Parses one of the names of an enum `T` (its serde names).
pub fn choice<T: DeserializeOwned>(name: &str, value: &Value) -> Result<T, CommandError> {
    string(name, value)?;
    serde_json::from_value(value.clone())
        .map_err(|_| CommandError::bad(name, "is not one of the allowed values"))
}

/// Parses `true` or `false`.
pub fn boolean(name: &str, value: &Value) -> Result<bool, CommandError> {
    value
        .as_bool()
        .ok_or_else(|| CommandError::bad(name, "must be true or false"))
}

/// Parses a whole number and checks the range.
pub fn integer(name: &str, value: &Value, min: i64, max: i64) -> Result<i64, CommandError> {
    let number = value
        .as_i64()
        .ok_or_else(|| CommandError::bad(name, "must be a whole number"))?;
    if (min..=max).contains(&number) {
        Ok(number)
    } else {
        Err(CommandError::bad(
            name,
            format!("must be between {min} and {max}"),
        ))
    }
}

/// Parses a percentage 0–100.
pub fn percent(name: &str, value: &Value) -> Result<u8, CommandError> {
    integer(name, value, 0, 100)
        .and_then(|n| u8::try_from(n).map_err(|_| CommandError::bad(name, "out of range")))
}

/// Parses an estimate in minutes.
pub fn minutes(name: &str, value: &Value) -> Result<u32, CommandError> {
    integer(name, value, 0, i64::from(plnwrk_core::ESTIMATE_MAX))
        .and_then(|n| u32::try_from(n).map_err(|_| CommandError::bad(name, "out of range")))
}

/// Parses a dependency lag in days.
pub fn lag(name: &str, value: &Value) -> Result<i32, CommandError> {
    let max = i64::from(plnwrk_core::LAG_MAX);
    integer(name, value, -max, max)
        .and_then(|n| i32::try_from(n).map_err(|_| CommandError::bad(name, "out of range")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use plnwrk_core::{TaskId, TaskStatus};

    const SPECS: &[ParamSpec] = &[
        ParamSpec::required("title", ParamKind::Title, "name"),
        ParamSpec::optional("status", ParamKind::Choice(&["todo", "done"]), "status"),
        ParamSpec::clearable("due", ParamKind::Date, "deadline"),
    ];

    fn check(value: Value) -> Result<(), CommandError> {
        Params::check(&value, SPECS).map(|_| ())
    }

    fn param_of(error: CommandError) -> String {
        match error {
            CommandError::BadParam { param, .. } => param,
            other => format!("{other:?}"),
        }
    }

    #[test]
    fn check_accepts_known_keys() {
        assert!(check(json!({"title": "a", "due": null})).is_ok());
    }

    #[test]
    fn check_refuses_non_objects_unknown_and_missing_keys() {
        assert_eq!(param_of(check(json!([1])).unwrap_err()), "(params)");
        assert_eq!(param_of(check(json!("x")).unwrap_err()), "(params)");
        assert_eq!(
            param_of(check(json!({"title": "a", "titel": "b"})).unwrap_err()),
            "titel"
        );
        assert_eq!(param_of(check(json!({})).unwrap_err()), "title");
        assert_eq!(
            param_of(check(json!({"title": null})).unwrap_err()),
            "title"
        );
        assert_eq!(
            param_of(check(json!({"title": "a", "status": null})).unwrap_err()),
            "status"
        );
    }

    #[test]
    fn titles() {
        assert_eq!(title("t", &json!(" a ")).unwrap(), "a");
        assert!(title("t", &json!("")).is_err());
        assert!(title("t", &json!(5)).is_err());
        assert!(title("t", &json!("x".repeat(201))).is_err());
    }

    #[test]
    fn bodies_labels_and_tags() {
        assert_eq!(body("b", &json!("a\nb")).unwrap(), "a\nb");
        assert!(body("b", &json!("\u{0}")).is_err());
        assert_eq!(label("l", &json!(" SKA ")).unwrap(), "SKA");
        assert!(label("l", &json!("")).is_err());
        assert_eq!(
            labels("t", &json!(["b", "a", "a"])).unwrap(),
            vec!["a", "b"]
        );
        assert!(labels("t", &json!("a")).is_err());
        assert!(labels("t", &json!([1])).is_err());
        assert!(labels("t", &json!(vec!["x"; 500])).is_err());
    }

    #[test]
    fn dates() {
        assert_eq!(
            date("d", &json!("2026-10-14")).unwrap().to_string(),
            "2026-10-14"
        );
        for bad in [
            json!("2026-02-30"),
            json!("14.10.2026"),
            json!("2026-1-1"),
            json!(20261014),
        ] {
            assert!(date("d", &bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn ids_choices_booleans() {
        let task = TaskId::from_ulid(ulid::Ulid(7));
        assert_eq!(id::<TaskId>("i", &json!(task.to_string())).unwrap(), task);
        assert!(id::<TaskId>("i", &json!("nope")).is_err());
        assert_eq!(
            choice::<TaskStatus>("s", &json!("in_progress")).unwrap(),
            TaskStatus::InProgress
        );
        assert!(choice::<TaskStatus>("s", &json!("fertig")).is_err());
        assert!(choice::<TaskStatus>("s", &json!({"todo": 1})).is_err());
        assert!(boolean("b", &json!(true)).unwrap());
        assert!(boolean("b", &json!("true")).is_err());
    }

    #[test]
    fn numbers_are_checked_for_type_and_range() {
        assert_eq!(percent("p", &json!(100)).unwrap(), 100);
        assert!(percent("p", &json!(101)).is_err());
        assert!(percent("p", &json!(-1)).is_err());
        assert!(percent("p", &json!(50.5)).is_err());
        assert!(percent("p", &json!(u64::MAX)).is_err());
        assert_eq!(lag("l", &json!(-365)).unwrap(), -365);
        assert!(lag("l", &json!(366)).is_err());
        assert!(minutes("m", &json!(1_000_001)).is_err());
    }

    #[test]
    fn req_opt_and_patch() {
        let value = json!({"title": "a", "due": null});
        let params = Params::check(&value, SPECS).unwrap();
        assert_eq!(params.req("title", title).unwrap(), "a");
        assert_eq!(params.opt("status", label).unwrap(), None);
        assert_eq!(params.patch("due", date).unwrap(), Patch::Clear);
        assert_eq!(params.patch("status", label).unwrap(), Patch::Keep);
        assert!(params.patch_required("due", date).is_err());
        assert!(params.has("due") && !params.has("status"));
        let mut field = Some(1);
        Patch::Clear.apply_to(&mut field);
        assert_eq!(field, None);
        Patch::Set(2).apply_to(&mut field);
        assert_eq!(field, Some(2));
    }

    #[test]
    fn schema_describes_an_object_without_extra_keys() {
        let schema = input_schema(SPECS);
        assert_eq!(schema["required"], json!(["title"]));
        assert_eq!(schema["additionalProperties"], json!(false));
        assert_eq!(
            schema["properties"]["status"]["enum"],
            json!(["todo", "done"])
        );
        assert_eq!(
            schema["properties"]["due"]["type"],
            json!(["string", "null"])
        );
    }
}
