//! The user's values for a plugin's settings (P11): each one checked against its field when read; a bad or unknown
//! one is the field's default. The guest gets them with every call and cannot write them.

use sdk::manifest::{Field, FieldKind};
use serde_json::Value;

/// By field id.
pub type Values = serde_json::Map<String, Value>;

pub fn fits(field: &Field, value: &Value) -> bool {
    match &field.kind {
        FieldKind::Switch => value.is_boolean(),
        FieldKind::Text { max_length } => value.as_str().is_some_and(|text| text.chars().count() <= *max_length as usize),
        FieldKind::Number { min, max, step } => value.as_f64().is_some_and(|number| {
            let steps = (number - min) / step;
            (*min..=*max).contains(&number) && (steps - steps.round()).abs() < 1e-9
        }),
        FieldKind::Choice { options } => value.as_str().is_some_and(|id| options.iter().any(|option| option.id == id)),
    }
}

/// Every field's value: the user's when it fits, else the default.
pub fn resolve(fields: &[Field], stored: &Values) -> Values {
    let value = |field: &Field| stored.get(&field.id).filter(|value| fits(field, value)).unwrap_or(&field.default).clone();
    fields.iter().map(|field| (field.id.clone(), value(field))).collect()
}

#[cfg(test)]
mod tests {
    use sdk::manifest::Choice;
    use serde_json::json;

    use super::*;

    fn field(id: &str, kind: FieldKind, default: Value) -> Field {
        Field { id: id.into(), kind, default, label: String::new(), hint: None }
    }

    #[test]
    fn a_value_that_does_not_fit_its_field_is_the_default() {
        let fields = [
            field("night", FieldKind::Switch, json!(false)),
            field("name", FieldKind::Text { max_length: 3 }, json!("")),
            field("level", FieldKind::Number { min: 1., max: 9., step: 2. }, json!(1)),
            field("tiles", FieldKind::Choice { options: vec![Choice { id: "sat".into(), label: String::new() }] }, json!("sat")),
        ];
        let stored = |values: Value| resolve(&fields, values.as_object().unwrap());
        let good = json!({"night": true, "name": "été", "level": 7, "tiles": "sat", "gone": 1});
        assert_eq!(Value::from(stored(good)), json!({"night": true, "name": "été", "level": 7, "tiles": "sat"}));
        let bad = json!({"night": "yes", "name": "four", "level": 4, "tiles": "drawn"});
        assert_eq!(Value::from(stored(bad)), json!({"night": false, "name": "", "level": 1, "tiles": "sat"}));
    }
}
