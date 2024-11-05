/*
 * Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
 * SPDX-License-Identifier: Apache-2.0
 */

//! Utilities to transform back and forth from Smithy Observability [Attributes] to
//! OTel [KeyValue]s.

use std::ops::Deref;

use aws_smithy_observability::attributes::{AttributeValue, Attributes};
use opentelemetry::{KeyValue, Value};

pub(crate) struct AttributesWrap(Attributes);
impl AttributesWrap {
    pub(crate) fn new(inner: Attributes) -> Self {
        Self(inner)
    }
}
impl Deref for AttributesWrap {
    type Target = Attributes;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(crate) fn kv_from_option_attr(input: Option<&Attributes>) -> Vec<KeyValue> {
    input
        .map(|attr| AttributesWrap::new(attr.clone()))
        .unwrap_or(AttributesWrap::new(Attributes::new()))
        .into()
}

#[allow(dead_code)]
pub(crate) fn option_attr_from_kv(input: &[KeyValue]) -> Option<Attributes> {
    if input.len() == 0 {
        return None;
    }

    Some(AttributesWrap::from(input).0)
}

impl<'a> From<AttributesWrap> for Vec<KeyValue> {
    fn from(value: AttributesWrap) -> Self {
        value
            .attributes()
            .iter()
            .map(|(k, v)| {
                KeyValue::new(
                    k.clone(),
                    match v {
                        AttributeValue::Long(val) => Value::I64(val.clone()),
                        AttributeValue::Double(val) => Value::F64(val.clone()),
                        AttributeValue::String(val) => Value::String(val.clone().into()),
                        AttributeValue::Bool(val) => Value::Bool(val.clone()),
                        _ => Value::String("UNSUPPORTED ATTRIBUTE VALUE TYPE".into()),
                    },
                )
            })
            .collect::<Vec<KeyValue>>()
    }
}

impl<'a> From<&[KeyValue]> for AttributesWrap {
    fn from(value: &[KeyValue]) -> Self {
        let mut attrs = Attributes::new();

        value.iter().for_each(|kv| {
            attrs.set(
                kv.key.clone().into(),
                match &kv.value {
                    Value::Bool(val) => AttributeValue::Bool(val.clone()),
                    Value::I64(val) => AttributeValue::Long(val.clone()),
                    Value::F64(val) => AttributeValue::Double(val.clone()),
                    Value::String(val) => AttributeValue::String(val.clone().into()),
                    Value::Array(_) => {
                        AttributeValue::String("UNSUPPORTED ATTRIBUTE VALUE TYPE".into())
                    }
                },
            )
        });

        AttributesWrap(attrs)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use aws_smithy_observability::attributes::{AttributeValue, Attributes};
    use opentelemetry::Value;

    #[test]
    fn attr_to_kv() {
        let mut attrs = Attributes::new();
        attrs.set("LONG".into(), AttributeValue::Long(64));
        attrs.set("DOUBLE".into(), AttributeValue::Double(64.0));
        attrs.set(
            "STRING".into(),
            AttributeValue::String("I AM A STRING".into()),
        );
        attrs.set("BOOLEAN".into(), AttributeValue::Bool(true));

        let kv = kv_from_option_attr(Some(&attrs));

        let kv_map: HashMap<String, Value> = kv
            .into_iter()
            .map(|kv| (kv.key.to_string(), kv.value))
            .collect();

        assert_eq!(kv_map.get("LONG").unwrap(), &Value::I64(64));
        assert_eq!(kv_map.get("DOUBLE").unwrap(), &Value::F64(64.0));
        assert_eq!(
            kv_map.get("STRING").unwrap(),
            &Value::String("I AM A STRING".into())
        );
        assert_eq!(kv_map.get("BOOLEAN").unwrap(), &Value::Bool(true));
    }

    #[test]
    fn kv_to_attr() {
        let mut kvs: Vec<KeyValue> = Vec::new();
        kvs.push(KeyValue::new("Bool", Value::Bool(true)));
        kvs.push(KeyValue::new(
            "String",
            Value::String("I AM A STRING".into()),
        ));
        kvs.push(KeyValue::new("I64", Value::I64(64)));
        kvs.push(KeyValue::new("F64", Value::F64(64.0)));

        let attrs = option_attr_from_kv(&kvs).unwrap();
        assert_eq!(
            attrs.get("Bool".into()).unwrap(),
            &AttributeValue::Bool(true)
        );
        assert_eq!(
            attrs.get("String".into()).unwrap(),
            &AttributeValue::String("I AM A STRING".into())
        );
        assert_eq!(attrs.get("I64".into()).unwrap(), &AttributeValue::Long(64));
        assert_eq!(
            attrs.get("F64".into()).unwrap(),
            &AttributeValue::Double(64.0)
        );
    }
}
