//! One property change, as compact as the stream needs it.

use std::sync::Arc;

use serde_json::{json, Value};

/// `fieldsChanged` bits the stream keeps
pub const VALUE: u8 = 1;
pub const ADDRESS: u8 = 2;

/// A Poke-A-Byte `PropertiesChanged` item without its bytes (no recorder
/// reads them) and only for a changed value or address.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: Arc<str>,
    pub address: Option<u64>,
    pub value: Value,
    pub fields: u8,
}

impl Change {
    pub fn fields_from(list: &[Value]) -> u8 {
        let mut f = 0;
        for x in list {
            match x.as_str() {
                Some("value") => f |= VALUE,
                Some("address") => f |= ADDRESS,
                _ => {}
            }
        }
        f
    }

    /// The item as the GameHook client takes it.
    pub fn to_json(&self) -> Value {
        let mut fields = Vec::new();
        if self.fields & VALUE != 0 {
            fields.push("value");
        }
        if self.fields & ADDRESS != 0 {
            fields.push("address");
        }
        json!({
            "path": &*self.path,
            "address": self.address,
            "value": self.value,
            "bytes": Value::Null,
            "isFrozen": false,
            "fieldsChanged": fields,
        })
    }
}
