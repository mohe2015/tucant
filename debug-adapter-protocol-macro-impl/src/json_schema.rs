use std::collections::{BTreeMap, HashSet};

use itertools::Itertools;
use proc_macro2::Span;
use syn::{
    punctuated::Punctuated,
    token::{self, Brace, Comma},
    LitStr,
};

use crate::{
    json_parser::{JSONValue, KeyValue},
    utils::{extract_keys, unexpected_keys, LitStrOrd},
};

#[allow(dead_code)]
#[derive(Debug)]
pub struct JSONSchema {
    pub schema: LitStr,
    pub title: LitStr,
    pub description: LitStr,
    pub r#type: LitStr,
    pub definitions: Vec<KeyValue<Definition>>,
}

impl TryFrom<JSONValue> for JSONSchema {
    type Error = syn::Error;

    fn try_from(value: JSONValue) -> Result<Self, Self::Error> {
        if let JSONValue::Object(value) = value {
            let [schema, title, description, r#type, definitions] = extract_keys(
                value,
                ["$schema", "title", "description", "type", "definitions"],
            )?;

            let (_, definitions): (Brace, Punctuated<KeyValue<JSONValue>, Comma>) =
                definitions.try_into()?;

            let (parsed_definitions, failed_definitions): (Vec<_>, Vec<_>) = definitions
                .into_iter()
                .map(|definition| -> Result<_, syn::Error> {
                    Ok(KeyValue {
                        key: definition.key,
                        value: TryInto::<Definition>::try_into(definition.value)?,
                    })
                })
                .partition_result();

            if let Some(error) = failed_definitions.into_iter().reduce(|mut e1, e2| {
                e1.combine(e2);
                e1
            }) {
                return Err(error);
            }

            Ok(Self {
                schema: schema.try_into()?,
                title: title.try_into()?,
                description: description.try_into()?,
                r#type: r#type.try_into()?,
                definitions: parsed_definitions,
            })
        } else {
            Err(syn::Error::new(value.span(), "Expected object"))
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Definition {
    pub title: Option<LitStr>,
    pub description: Option<LitStr>,
    pub definition_type: DefinitionType,
}

impl TryFrom<JSONValue> for Definition {
    type Error = syn::Error;

    fn try_from(value: JSONValue) -> Result<Self, Self::Error> {
        let (brace, value): (token::Brace, Punctuated<KeyValue<JSONValue>, token::Comma>) =
            value.try_into()?;

        let mut map: BTreeMap<_, _> = value
            .into_iter()
            .map(|e| (LitStrOrd(e.key), e.value))
            .collect();

        let title = map
            .remove(&LitStrOrd(LitStr::new("title", Span::call_site())))
            .map(TryInto::<LitStr>::try_into)
            .transpose()?;

        let description = map
            .remove(&LitStrOrd(LitStr::new("description", Span::call_site())))
            .map(TryInto::<LitStr>::try_into)
            .transpose()?;

        Ok(Self {
            title,
            description,
            definition_type: (brace, map).try_into()?,
        })
    }
}

#[derive(Debug)]
pub enum DefinitionType {
    AllOf(AllOfDefinition),
    OneOf(OneOfDefinition),
    Ref(RefDefinition),
    ObjectType(ObjectType),
    StringType(StringType),
    ArrayType(ArrayType),
    IntegerType(IntegerType),
    DoubleType(DoubleType),
    BooleanType(BooleanType),
}

impl TryFrom<(Brace, BTreeMap<LitStrOrd, JSONValue>)> for DefinitionType {
    type Error = syn::Error;

    fn try_from(
        (brace, mut map): (Brace, BTreeMap<LitStrOrd, JSONValue>),
    ) -> Result<Self, Self::Error> {
        if map.contains_key(&LitStrOrd(LitStr::new("$ref", Span::call_site()))) {
            map.try_into().map(Self::Ref)
        } else if map.contains_key(&LitStrOrd(LitStr::new("type", Span::call_site()))) {
            let r#type = map
                .remove(&LitStrOrd(LitStr::new("type", Span::call_site())))
                .unwrap();

            // https://datatracker.ietf.org/doc/html/draft-fge-json-schema-validation-00#section-5.5.2

            match r#type {
                JSONValue::String(r#type) => {
                    // TODO FIXME run partition_result

                    // https://datatracker.ietf.org/doc/html/draft-zyp-json-schema-04#section-3.5
                    if r#type.value() == "object" {
                        map.try_into().map(Self::ObjectType)
                    } else if r#type.value() == "string" {
                        map.try_into().map(Self::StringType)
                    } else if "integer" == r#type.value() {
                        map.try_into().map(Self::IntegerType)
                    } else if "boolean" == r#type.value() {
                        map.try_into().map(Self::BooleanType)
                    } else if "number" == r#type.value() {
                        map.try_into().map(Self::DoubleType)
                    } else if r#type.value() == "array" {
                        map.try_into().map(Self::ArrayType)
                    } else {
                        return Err(syn::Error::new(
                            r#type.span(),
                            r#"Expected an array of primitive types or one of the primitive types "object", "string", "integer", "number", "boolean" or "array""#,
                        ));
                    }
                }
                JSONValue::Array(_type) => {
                    // we're doomed, this is basically a oneOf?
                    unexpected_keys(
                        map,
                        Ok(Self::OneOf(OneOfDefinition {
                            definitions: Vec::new(),
                        })),
                    )
                }
                _ => Err(syn::Error::new(r#type.span(), "Expected string or array")),
            }
        } else if map.contains_key(&LitStrOrd(LitStr::new("allOf", Span::call_site()))) {
            map.try_into().map(Self::AllOf)
        } else if map.contains_key(&LitStrOrd(LitStr::new("oneOf", Span::call_site()))) {
            map.try_into().map(Self::OneOf)
        } else {
            Err(syn::Error::new(brace.span, "Unknown definition"))
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct RefDefinition {
    pub name: LitStr,
}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for RefDefinition {
    type Error = syn::Error;

    fn try_from(mut map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        let r#ref = map
            .remove(&LitStrOrd(LitStr::new("$ref", Span::call_site())))
            .map(TryInto::<LitStr>::try_into)
            .transpose()?
            .unwrap();

        unexpected_keys(map, Ok(Self { name: r#ref }))
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct AllOfDefinition {
    pub definitions: Vec<Definition>,
}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for AllOfDefinition {
    type Error = syn::Error;

    fn try_from(mut map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        // https://datatracker.ietf.org/doc/html/draft-fge-json-schema-validation-00#section-5.5.3
        let all_of = map
            .remove(&LitStrOrd(LitStr::new("allOf", Span::call_site())))
            .unwrap();

        let (_, all_of): (token::Bracket, Punctuated<JSONValue, token::Comma>) =
            all_of.try_into()?;

        let (parsed_definitions, failed_definitions): (Vec<_>, Vec<_>) = all_of
            .into_iter()
            .map(TryInto::<Definition>::try_into)
            .partition_result();

        if let Some(error) = failed_definitions.into_iter().reduce(|mut e1, e2| {
            e1.combine(e2);
            e1
        }) {
            return Err(error);
        }

        unexpected_keys(
            map,
            Ok(Self {
                definitions: parsed_definitions,
            }),
        )
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct OneOfDefinition {
    pub definitions: Vec<Definition>,
}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for OneOfDefinition {
    type Error = syn::Error;

    fn try_from(mut map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        // https://datatracker.ietf.org/doc/html/draft-fge-json-schema-validation-00#section-5.5.5
        let one_of = map
            .remove(&LitStrOrd(LitStr::new("oneOf", Span::call_site())))
            .unwrap();

        let (_, one_of): (token::Bracket, Punctuated<JSONValue, token::Comma>) =
            one_of.try_into()?;

        let (parsed_definitions, failed_definitions): (Vec<_>, Vec<_>) = one_of
            .into_iter()
            .map(TryInto::<Definition>::try_into)
            .partition_result();

        if let Some(error) = failed_definitions.into_iter().reduce(|mut e1, e2| {
            e1.combine(e2);
            e1
        }) {
            return Err(error);
        }

        unexpected_keys(
            map,
            Ok(Self {
                definitions: parsed_definitions,
            }),
        )
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ObjectType {
    // bool required
    pub properties: Vec<KeyValue<(Definition, bool)>>,
    pub additional_properties_type: Box<Definition>,
}

/*
fn get_any_object_type() -> DefinitionType {
    DefinitionType::OneOf(OneOfDefinition {
        definitions: vec![Definition {
            title: None,
            description: None,
            definition_type: DefinitionType::ObjectType(ObjectType {
                properties: Vec::new(),
                additional_properties_type: todo!(),
            }),
        }],
    })
}
*/

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for ObjectType {
    type Error = syn::Error;

    fn try_from(mut map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        let (required, required_failures): (HashSet<_>, Vec<_>) = map
            .remove(&LitStrOrd(LitStr::new("required", Span::call_site())))
            .map(TryInto::<(token::Bracket, Punctuated<JSONValue, token::Comma>)>::try_into)
            .transpose()?
            .map(|v| v.1.into_iter())
            .into_iter()
            .flatten()
            .map(TryInto::<LitStr>::try_into)
            .partition_result();

        if let Some(error) = required_failures.into_iter().reduce(|mut e1, e2| {
            e1.combine(e2);
            e1
        }) {
            return Err(error);
        }

        let properties = map
            .remove(&LitStrOrd(LitStr::new("properties", Span::call_site())))
            .map(TryInto::<(token::Brace, Punctuated<KeyValue<JSONValue>, token::Comma>)>::try_into)
            .transpose()?;

        let properties = if let Some((_, properties)) = properties {
            let (parsed_properties, failed_properties): (Vec<_>, Vec<_>) = properties
                .into_iter()
                .map(|property| {
                    Ok(KeyValue {
                        value: (
                            TryInto::<Definition>::try_into(property.value)?,
                            required.contains(&property.key),
                        ),
                        key: property.key,
                    })
                })
                .partition_result();

            if let Some(error) =
                failed_properties
                    .into_iter()
                    .reduce(|mut e1: syn::Error, e2: syn::Error| {
                        e1.combine(e2);
                        e1
                    })
            {
                return Err(error);
            }
            Some(parsed_properties)
        } else {
            None
        };

        let additional_properties = map.remove(&LitStrOrd(LitStr::new(
            "additionalProperties",
            Span::call_site(),
        )));

        let additional_properties = match additional_properties {
            Some(JSONValue::Bool(_)) => Some(Definition {
                title: None,
                description: None,
                definition_type: DefinitionType::Ref(RefDefinition {
                    name: LitStr::new("object", Span::call_site()),
                }),
            }),
            Some(additional_properties @ JSONValue::Object(_)) => {
                Some(TryInto::<Definition>::try_into(additional_properties)?)
            }
            None => None,
            Some(value) => return Err(syn::Error::new(value.span(), "Expected boolean of object")),
        };

        let result = match (properties, additional_properties) {
            // no properties and no additionalProperties (this probably means any object)
            (None, None) => Self {
                properties: Vec::new(),
                additional_properties_type: Box::new(Definition {
                    title: None,
                    description: None,
                    definition_type: DefinitionType::Ref(RefDefinition {
                        name: LitStr::new("object", Span::call_site()),
                    }),
                }),
            },
            // no properties and additionalproperties (this means any properties of the type (same as with properties {}))
            (None, Some(additional_properties)) => Self {
                properties: Vec::new(),
                additional_properties_type: Box::new(additional_properties),
            },
            // properties and no additionalProperties (this means exactly these properties)
            (Some(properties), None) => Self {
                properties,
                additional_properties_type: Box::new(Definition {
                    title: None,
                    description: None,
                    definition_type: DefinitionType::OneOf(OneOfDefinition {
                        definitions: Vec::new(),
                    }),
                }),
            },
            // properties and additionalProperties (this means these properties and the others of the other type)
            (Some(properties), Some(additional_properties)) => Self {
                properties,
                additional_properties_type: Box::new(additional_properties),
            },
        };

        unexpected_keys(map, Ok(result))
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct StringType {
    pub enum_values: Vec<(LitStr, Option<LitStr>)>,
    pub exhaustive: bool, // if enum_values empty and this is false it's a normal string
}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for StringType {
    type Error = syn::Error;

    fn try_from(mut map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        // https://datatracker.ietf.org/doc/html/draft-fge-json-schema-validation-00#section-5.5.1
        let r#enum = map
            .remove(&LitStrOrd(LitStr::new("enum", Span::call_site())))
            .map(TryInto::<(token::Bracket, Punctuated<JSONValue, token::Comma>)>::try_into)
            .transpose()?;

        let underscore_enum = map
            .remove(&LitStrOrd(LitStr::new("_enum", Span::call_site())))
            .map(TryInto::<(token::Bracket, Punctuated<JSONValue, token::Comma>)>::try_into)
            .transpose()?;

        let exhaustive = r#enum.is_some();

        match (r#enum, underscore_enum) {
            (Some(the_enum), None) | (None, Some(the_enum)) => {
                let enum_descriptions = map
                    .remove(&LitStrOrd(LitStr::new(
                        "enumDescriptions",
                        Span::call_site(),
                    )))
                    .map(TryInto::<(token::Bracket, Punctuated<JSONValue, token::Comma>)>::try_into)
                    .transpose()?
                    .map(|v| v.1.into_iter().map(Some))
                    .into_iter()
                    .flatten();

                let (enum_values, enum_errors): (Vec<_>, Vec<_>) = the_enum
                    .1
                    .into_iter()
                    .zip(enum_descriptions.chain(std::iter::repeat_with(|| None)))
                    .map(
                        |(constant, optional_description)| -> Result<_, syn::Error> {
                            Ok((
                                TryInto::<LitStr>::try_into(constant)?,
                                optional_description
                                    .map(TryInto::<LitStr>::try_into)
                                    .transpose()?,
                            ))
                        },
                    )
                    .partition_result();

                if let Some(error) = enum_errors.into_iter().reduce(|mut e1, e2| {
                    e1.combine(e2);
                    e1
                }) {
                    return Err(error);
                }

                unexpected_keys(
                    map,
                    Ok(Self {
                        enum_values,
                        exhaustive,
                    }),
                )
            }
            (None, None) => unexpected_keys(
                map,
                Ok(Self {
                    enum_values: Vec::new(),
                    exhaustive: false,
                }),
            ),
            (Some(_), Some(_)) => todo!(),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ArrayType {
    pub item_type: Box<Definition>,
}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for ArrayType {
    type Error = syn::Error;

    fn try_from(mut map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        let item_type = map
            .remove(&LitStrOrd(LitStr::new("items", Span::call_site())))
            .map(TryInto::<Definition>::try_into)
            .transpose()?
            .map(Box::new)
            .unwrap(); // TODO FIXME

        unexpected_keys(map, Ok(Self { item_type }))
    }
}

#[derive(Debug)]
pub struct IntegerType {}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for IntegerType {
    type Error = syn::Error;

    fn try_from(map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        unexpected_keys(map, Ok(Self {}))
    }
}

#[derive(Debug)]
pub struct DoubleType {}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for DoubleType {
    type Error = syn::Error;

    fn try_from(map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        unexpected_keys(map, Ok(Self {}))
    }
}

#[derive(Debug)]
pub struct BooleanType {}

impl TryFrom<BTreeMap<LitStrOrd, JSONValue>> for BooleanType {
    type Error = syn::Error;

    fn try_from(map: BTreeMap<LitStrOrd, JSONValue>) -> Result<Self, Self::Error> {
        unexpected_keys(map, Ok(Self {}))
    }
}
