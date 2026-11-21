//! Projection of `pkl_runtime::schema::ModuleSchema` into finite, acyclic JS objects.

use super::decode::decoded_object_to_js;
use super::util::set_prop;
use js_sys::{Array, Object};
use pkl_runtime::schema::{
    Modifier, ModuleSchema, PClass, PClassInfo, PClassMethod, PClassProperty, PType, SchemaStore,
    SourceLocation, TypeAlias, TypeAliasInfo, TypeParameter, Variance,
};
use wasm_bindgen::JsValue;

pub fn module_schema_to_js(schema: &ModuleSchema) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "moduleUri", JsValue::from_str(&schema.module_uri))?;
    set_prop(&obj, "moduleName", JsValue::from_str(&schema.module_name))?;
    set_prop(
        &obj,
        "shortModuleName",
        JsValue::from_str(schema.short_module_name()),
    )?;
    set_prop(&obj, "isAmend", JsValue::from_bool(schema.is_amend))?;
    set_prop(&obj, "isExtend", JsValue::from_bool(schema.is_extend()))?;
    set_prop(
        &obj,
        "docComment",
        schema
            .doc_comment
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    set_prop(&obj, "annotations", annotations_to_js(&schema.annotations)?)?;
    set_prop(
        &obj,
        "supermodule",
        match &schema.supermodule {
            Some(sup) => module_schema_to_js(sup)?,
            None => JsValue::NULL,
        },
    )?;
    set_prop(
        &obj,
        "moduleClass",
        pclass_to_js(schema.module_class(), &schema.store)?,
    )?;

    let classes = Object::new();
    for (name, cid) in &schema.classes {
        set_prop(
            &classes,
            name,
            pclass_to_js(schema.class(*cid), &schema.store)?,
        )?;
    }
    set_prop(&obj, "classes", classes.into())?;

    let all_classes = Object::new();
    for (name, cid) in schema.all_classes() {
        set_prop(
            &all_classes,
            &name,
            pclass_to_js(schema.class(cid), &schema.store)?,
        )?;
    }
    set_prop(&obj, "allClasses", all_classes.into())?;

    let type_aliases = Object::new();
    for (name, tid) in &schema.type_aliases {
        set_prop(
            &type_aliases,
            name,
            type_alias_to_js(schema.type_alias(*tid))?,
        )?;
    }
    set_prop(&obj, "typeAliases", type_aliases.into())?;

    let all_type_aliases = Object::new();
    for (name, tid) in schema.all_type_aliases() {
        set_prop(
            &all_type_aliases,
            &name,
            type_alias_to_js(schema.type_alias(tid))?,
        )?;
    }
    set_prop(&obj, "allTypeAliases", all_type_aliases.into())?;

    let imports = Object::new();
    for (alias, uri) in &schema.imports {
        set_prop(&imports, alias, JsValue::from_str(uri))?;
    }
    set_prop(&obj, "imports", imports.into())?;

    Ok(obj.into())
}

fn pclass_to_js(cls: &PClass, store: &SchemaStore) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "info", class_info_to_js(&cls.info)?)?;
    set_prop(&obj, "moduleName", JsValue::from_str(cls.module_name()))?;
    set_prop(&obj, "simpleName", JsValue::from_str(cls.simple_name()))?;
    set_prop(
        &obj,
        "qualifiedName",
        JsValue::from_str(&cls.qualified_name()),
    )?;
    set_prop(&obj, "displayName", JsValue::from_str(&cls.display_name()))?;
    set_prop(
        &obj,
        "isModuleClass",
        JsValue::from_bool(cls.is_module_class()),
    )?;
    set_prop(
        &obj,
        "docComment",
        cls.doc_comment
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    set_prop(
        &obj,
        "sourceLocation",
        source_location_to_js(cls.source_location)?,
    )?;
    set_prop(&obj, "modifiers", modifiers_to_js(&cls.modifiers))?;
    set_prop(&obj, "annotations", annotations_to_js(&cls.annotations)?)?;
    set_prop(
        &obj,
        "typeParameters",
        type_parameters_to_js(&cls.type_parameters)?,
    )?;
    set_prop(
        &obj,
        "superclass",
        match cls.superclass {
            Some(sup_id) => class_info_to_js(&store.class(sup_id).info)?,
            None => JsValue::NULL,
        },
    )?;
    set_prop(
        &obj,
        "supertype",
        match &cls.supertype {
            Some(ty) => ptype_to_js(ty)?,
            None => JsValue::NULL,
        },
    )?;

    let properties = Object::new();
    for (name, prop) in &cls.properties {
        set_prop(&properties, name, property_to_js(prop)?)?;
    }
    set_prop(&obj, "properties", properties.into())?;

    let all_properties = Object::new();
    for (name, prop) in store.all_properties(cls.id) {
        set_prop(&all_properties, &name, property_to_js(prop)?)?;
    }
    set_prop(&obj, "allProperties", all_properties.into())?;

    let methods = Object::new();
    for (name, method) in &cls.methods {
        set_prop(&methods, name, method_to_js(method)?)?;
    }
    set_prop(&obj, "methods", methods.into())?;

    let all_methods = Object::new();
    for (name, method) in store.all_methods(cls.id) {
        set_prop(&all_methods, &name, method_to_js(method)?)?;
    }
    set_prop(&obj, "allMethods", all_methods.into())?;

    Ok(obj.into())
}

fn property_to_js(prop: &PClassProperty) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "name", JsValue::from_str(&prop.simple_name))?;
    set_prop(&obj, "simpleName", JsValue::from_str(&prop.simple_name))?;
    set_prop(&obj, "moduleName", JsValue::from_str(prop.module_name()))?;
    set_prop(
        &obj,
        "docComment",
        prop.doc_comment
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    set_prop(
        &obj,
        "sourceLocation",
        source_location_to_js(prop.source_location)?,
    )?;
    set_prop(&obj, "modifiers", modifiers_to_js(&prop.modifiers))?;
    set_prop(&obj, "annotations", annotations_to_js(&prop.annotations)?)?;
    set_prop(&obj, "type", ptype_to_js(&prop.ty)?)?;
    Ok(obj.into())
}

fn method_to_js(method: &PClassMethod) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "name", JsValue::from_str(&method.simple_name))?;
    set_prop(&obj, "simpleName", JsValue::from_str(&method.simple_name))?;
    set_prop(&obj, "moduleName", JsValue::from_str(method.module_name()))?;
    set_prop(
        &obj,
        "docComment",
        method
            .doc_comment
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    set_prop(
        &obj,
        "sourceLocation",
        source_location_to_js(method.source_location)?,
    )?;
    set_prop(&obj, "modifiers", modifiers_to_js(&method.modifiers))?;
    set_prop(&obj, "annotations", annotations_to_js(&method.annotations)?)?;
    set_prop(
        &obj,
        "typeParameters",
        type_parameters_to_js(&method.type_parameters)?,
    )?;
    let params = Object::new();
    for (name, ty) in &method.parameters {
        set_prop(&params, name, ptype_to_js(ty)?)?;
    }
    set_prop(&obj, "parameters", params.into())?;
    set_prop(&obj, "returnType", ptype_to_js(&method.return_type)?)?;
    Ok(obj.into())
}

fn type_alias_to_js(alias: &TypeAlias) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "info", type_alias_info_to_js(&alias.info)?)?;
    set_prop(&obj, "moduleName", JsValue::from_str(alias.module_name()))?;
    set_prop(&obj, "simpleName", JsValue::from_str(alias.simple_name()))?;
    set_prop(
        &obj,
        "qualifiedName",
        JsValue::from_str(alias.qualified_name()),
    )?;
    set_prop(
        &obj,
        "displayName",
        JsValue::from_str(&alias.display_name()),
    )?;
    set_prop(
        &obj,
        "docComment",
        alias
            .doc_comment
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    set_prop(
        &obj,
        "sourceLocation",
        source_location_to_js(alias.source_location)?,
    )?;
    set_prop(&obj, "modifiers", modifiers_to_js(&alias.modifiers))?;
    set_prop(&obj, "annotations", annotations_to_js(&alias.annotations)?)?;
    set_prop(
        &obj,
        "typeParameters",
        type_parameters_to_js(&alias.type_parameters)?,
    )?;
    set_prop(&obj, "aliasedType", ptype_to_js(&alias.aliased_type)?)?;
    Ok(obj.into())
}

fn class_info_to_js(info: &PClassInfo) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "moduleName", JsValue::from_str(&info.module_name))?;
    set_prop(&obj, "simpleName", JsValue::from_str(&info.simple_name))?;
    set_prop(&obj, "moduleUri", JsValue::from_str(&info.module_uri))?;
    set_prop(
        &obj,
        "qualifiedName",
        JsValue::from_str(&info.qualified_name()),
    )?;
    set_prop(&obj, "displayName", JsValue::from_str(&info.display_name()))?;
    set_prop(
        &obj,
        "isModuleClass",
        JsValue::from_bool(info.is_module_class),
    )?;
    Ok(obj.into())
}

fn type_alias_info_to_js(info: &TypeAliasInfo) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "moduleName", JsValue::from_str(&info.module_name))?;
    set_prop(&obj, "simpleName", JsValue::from_str(&info.simple_name))?;
    set_prop(
        &obj,
        "qualifiedName",
        JsValue::from_str(&info.qualified_name),
    )?;
    set_prop(&obj, "displayName", JsValue::from_str(&info.display_name()))?;
    Ok(obj.into())
}

fn ptype_to_js(ty: &PType) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    match ty {
        PType::Unknown => {
            set_prop(&obj, "kind", JsValue::from_str("unknown"))?;
        }
        PType::Nothing => {
            set_prop(&obj, "kind", JsValue::from_str("nothing"))?;
        }
        PType::Module => {
            set_prop(&obj, "kind", JsValue::from_str("module"))?;
        }
        PType::StringLiteral(literal) => {
            set_prop(&obj, "kind", JsValue::from_str("stringLiteral"))?;
            set_prop(&obj, "literal", JsValue::from_str(literal))?;
        }
        PType::Class {
            class_info,
            type_arguments,
            ..
        } => {
            set_prop(&obj, "kind", JsValue::from_str("class"))?;
            set_prop(&obj, "classInfo", class_info_to_js(class_info)?)?;
            set_prop(&obj, "typeArguments", ptypes_to_js(type_arguments)?)?;
        }
        PType::Nullable(base) => {
            set_prop(&obj, "kind", JsValue::from_str("nullable"))?;
            set_prop(&obj, "baseType", ptype_to_js(base)?)?;
        }
        PType::Union(elements) => {
            set_prop(&obj, "kind", JsValue::from_str("union"))?;
            set_prop(&obj, "elementTypes", ptypes_to_js(elements)?)?;
        }
        PType::Function {
            parameter_types,
            return_type,
        } => {
            set_prop(&obj, "kind", JsValue::from_str("function"))?;
            set_prop(&obj, "parameterTypes", ptypes_to_js(parameter_types)?)?;
            set_prop(&obj, "returnType", ptype_to_js(return_type)?)?;
        }
        PType::Constrained {
            base_type,
            constraints,
        } => {
            set_prop(&obj, "kind", JsValue::from_str("constrained"))?;
            set_prop(&obj, "baseType", ptype_to_js(base_type)?)?;
            let list = Array::new();
            for c in constraints {
                list.push(&JsValue::from_str(c));
            }
            set_prop(&obj, "constraints", list.into())?;
        }
        PType::Alias {
            alias_info,
            type_arguments,
            aliased_type,
            ..
        } => {
            set_prop(&obj, "kind", JsValue::from_str("alias"))?;
            set_prop(&obj, "aliasInfo", type_alias_info_to_js(alias_info)?)?;
            set_prop(&obj, "typeArguments", ptypes_to_js(type_arguments)?)?;
            set_prop(&obj, "aliasedType", ptype_to_js(aliased_type)?)?;
        }
        PType::TypeVariable(tp) => {
            set_prop(&obj, "kind", JsValue::from_str("typeVariable"))?;
            set_prop(&obj, "typeParameter", type_parameter_to_js(tp)?)?;
        }
    }
    Ok(obj.into())
}

fn ptypes_to_js(types: &[PType]) -> Result<JsValue, JsValue> {
    let arr = Array::new_with_length(types.len() as u32);
    for (idx, ty) in types.iter().enumerate() {
        arr.set(idx as u32, ptype_to_js(ty)?);
    }
    Ok(arr.into())
}

fn source_location_to_js(loc: SourceLocation) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(
        &obj,
        "startLine",
        JsValue::from_f64(f64::from(loc.start_line)),
    )?;
    set_prop(&obj, "endLine", JsValue::from_f64(f64::from(loc.end_line)))?;
    Ok(obj.into())
}

fn modifiers_to_js(modifiers: &[Modifier]) -> JsValue {
    let arr = Array::new();
    for m in modifiers {
        arr.push(&JsValue::from_str(m.as_str()));
    }
    arr.into()
}

fn type_parameter_to_js(tp: &TypeParameter) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "name", JsValue::from_str(&tp.name))?;
    let variance = match tp.variance {
        Variance::Invariant => "invariant",
        Variance::Covariant => "covariant",
        Variance::Contravariant => "contravariant",
    };
    set_prop(&obj, "variance", JsValue::from_str(variance))?;
    Ok(obj.into())
}

fn type_parameters_to_js(params: &[TypeParameter]) -> Result<JsValue, JsValue> {
    let arr = Array::new_with_length(params.len() as u32);
    for (idx, tp) in params.iter().enumerate() {
        arr.set(idx as u32, type_parameter_to_js(tp)?);
    }
    Ok(arr.into())
}

fn annotations_to_js(
    annotations: &[pkl_runtime::pkl_binary_decode::DecodedObject],
) -> Result<JsValue, JsValue> {
    let arr = Array::new_with_length(annotations.len() as u32);
    for (idx, ann) in annotations.iter().enumerate() {
        arr.set(idx as u32, decoded_object_to_js(ann)?);
    }
    Ok(arr.into())
}
