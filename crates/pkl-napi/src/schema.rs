//! Conversion of `pkl_runtime::schema::ModuleSchema` into JS objects matching the M6 contract.

use crate::values::decoded_object_to_js;
use napi::{
    Env, Result,
    bindgen_prelude::{Array, Null, Object},
};
use pkl_runtime::schema::{
    ModuleSchema, PClass, PClassInfo, PClassMethod, PClassProperty, PType, SchemaStore,
    SourceLocation, TypeAlias, TypeAliasInfo, TypeParameter, Variance,
};

pub fn module_schema_to_js<'env>(env: &'env Env, schema: &ModuleSchema) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("moduleUri", schema.module_uri.as_str())?;
    out.set("moduleName", schema.module_name.as_str())?;
    out.set("shortModuleName", schema.short_module_name())?;
    out.set("isAmend", schema.is_amend)?;
    out.set("isExtend", schema.is_extend())?;
    match &schema.doc_comment {
        Some(doc) => out.set("docComment", doc.as_str())?,
        None => out.set("docComment", Null)?,
    }

    let mut anns = Vec::with_capacity(schema.annotations.len());
    for ann in &schema.annotations {
        anns.push(decoded_object_to_js(env, ann)?);
    }
    out.set("annotations", Array::from_vec(env, anns)?)?;

    match &schema.supermodule {
        Some(sup) => out.set("supermodule", module_schema_to_js(env, sup)?)?,
        None => out.set("supermodule", Null)?,
    }

    out.set(
        "moduleClass",
        pclass_to_js(env, schema.module_class(), &schema.store)?,
    )?;

    let mut classes_obj = Object::new(env)?;
    for (name, cid) in &schema.classes {
        classes_obj.set(
            name.as_str(),
            pclass_to_js(env, schema.store.class(*cid), &schema.store)?,
        )?;
    }
    out.set("classes", classes_obj)?;

    let mut all_classes_obj = Object::new(env)?;
    for (name, cid) in schema.all_classes() {
        all_classes_obj.set(
            name.as_str(),
            pclass_to_js(env, schema.store.class(cid), &schema.store)?,
        )?;
    }
    out.set("allClasses", all_classes_obj)?;

    let mut aliases_obj = Object::new(env)?;
    for (name, tid) in &schema.type_aliases {
        aliases_obj.set(
            name.as_str(),
            type_alias_to_js(env, schema.store.type_alias(*tid))?,
        )?;
    }
    out.set("typeAliases", aliases_obj)?;

    let mut all_aliases_obj = Object::new(env)?;
    for (name, tid) in schema.all_type_aliases() {
        all_aliases_obj.set(
            name.as_str(),
            type_alias_to_js(env, schema.store.type_alias(tid))?,
        )?;
    }
    out.set("allTypeAliases", all_aliases_obj)?;

    let mut imports_obj = Object::new(env)?;
    for (alias, uri) in &schema.imports {
        imports_obj.set(alias.as_str(), uri.as_str())?;
    }
    out.set("imports", imports_obj)?;

    Ok(out)
}

fn pclass_to_js<'env>(env: &'env Env, cls: &PClass, store: &SchemaStore) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("moduleName", cls.info.module_name.as_str())?;
    out.set("simpleName", cls.info.simple_name.as_str())?;
    out.set("qualifiedName", cls.info.qualified_name())?;
    out.set("displayName", cls.info.display_name())?;
    out.set("moduleUri", cls.info.module_uri.as_str())?;
    out.set("isModuleClass", cls.info.is_module_class)?;
    match &cls.doc_comment {
        Some(doc) => out.set("docComment", doc.as_str())?,
        None => out.set("docComment", Null)?,
    }
    out.set(
        "sourceLocation",
        source_location_to_js(env, cls.source_location)?,
    )?;
    out.set(
        "modifiers",
        cls.modifiers
            .iter()
            .map(|m| m.as_str().to_string())
            .collect::<Vec<_>>(),
    )?;

    let mut anns = Vec::with_capacity(cls.annotations.len());
    for ann in &cls.annotations {
        anns.push(decoded_object_to_js(env, ann)?);
    }
    out.set("annotations", Array::from_vec(env, anns)?)?;

    let mut tparams = Vec::with_capacity(cls.type_parameters.len());
    for tp in &cls.type_parameters {
        tparams.push(type_param_to_js(env, tp)?);
    }
    out.set("typeParameters", Array::from_vec(env, tparams)?)?;

    match cls.superclass {
        Some(sup_id) => out.set(
            "superclass",
            class_info_to_js(env, &store.class(sup_id).info)?,
        )?,
        None => out.set("superclass", Null)?,
    }
    match &cls.supertype {
        Some(ty) => out.set("supertype", ptype_to_js(env, ty)?)?,
        None => out.set("supertype", Null)?,
    }

    let mut props_obj = Object::new(env)?;
    for (name, prop) in &cls.properties {
        props_obj.set(name.as_str(), property_to_js(env, prop)?)?;
    }
    out.set("properties", props_obj)?;

    let mut all_props_obj = Object::new(env)?;
    for (name, prop) in store.all_properties(cls.id) {
        all_props_obj.set(name.as_str(), property_to_js(env, prop)?)?;
    }
    out.set("allProperties", all_props_obj)?;

    let mut methods_obj = Object::new(env)?;
    for (name, method) in &cls.methods {
        methods_obj.set(name.as_str(), method_to_js(env, method)?)?;
    }
    out.set("methods", methods_obj)?;

    let mut all_methods_obj = Object::new(env)?;
    for (name, method) in store.all_methods(cls.id) {
        all_methods_obj.set(name.as_str(), method_to_js(env, method)?)?;
    }
    out.set("allMethods", all_methods_obj)?;

    Ok(out)
}

fn property_to_js<'env>(env: &'env Env, prop: &PClassProperty) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("name", prop.simple_name.as_str())?;
    out.set("simpleName", prop.simple_name.as_str())?;
    out.set("moduleName", prop.owner_info.module_name.as_str())?;
    match &prop.doc_comment {
        Some(doc) => out.set("docComment", doc.as_str())?,
        None => out.set("docComment", Null)?,
    }
    out.set(
        "sourceLocation",
        source_location_to_js(env, prop.source_location)?,
    )?;
    out.set(
        "modifiers",
        prop.modifiers
            .iter()
            .map(|m| m.as_str().to_string())
            .collect::<Vec<_>>(),
    )?;
    let mut anns = Vec::with_capacity(prop.annotations.len());
    for ann in &prop.annotations {
        anns.push(decoded_object_to_js(env, ann)?);
    }
    out.set("annotations", Array::from_vec(env, anns)?)?;
    out.set("type", ptype_to_js(env, &prop.ty)?)?;
    Ok(out)
}

fn method_to_js<'env>(env: &'env Env, method: &PClassMethod) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("name", method.simple_name.as_str())?;
    out.set("simpleName", method.simple_name.as_str())?;
    out.set("moduleName", method.owner_info.module_name.as_str())?;
    match &method.doc_comment {
        Some(doc) => out.set("docComment", doc.as_str())?,
        None => out.set("docComment", Null)?,
    }
    out.set(
        "sourceLocation",
        source_location_to_js(env, method.source_location)?,
    )?;
    out.set(
        "modifiers",
        method
            .modifiers
            .iter()
            .map(|m| m.as_str().to_string())
            .collect::<Vec<_>>(),
    )?;
    let mut anns = Vec::with_capacity(method.annotations.len());
    for ann in &method.annotations {
        anns.push(decoded_object_to_js(env, ann)?);
    }
    out.set("annotations", Array::from_vec(env, anns)?)?;

    let mut tparams = Vec::with_capacity(method.type_parameters.len());
    for tp in &method.type_parameters {
        tparams.push(type_param_to_js(env, tp)?);
    }
    out.set("typeParameters", Array::from_vec(env, tparams)?)?;

    let mut params = Object::new(env)?;
    for (pname, pty) in &method.parameters {
        params.set(pname.as_str(), ptype_to_js(env, pty)?)?;
    }
    out.set("parameters", params)?;
    out.set("returnType", ptype_to_js(env, &method.return_type)?)?;
    Ok(out)
}

fn type_alias_to_js<'env>(env: &'env Env, alias: &TypeAlias) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("moduleName", alias.info.module_name.as_str())?;
    out.set("simpleName", alias.info.simple_name.as_str())?;
    out.set("qualifiedName", alias.info.qualified_name.as_str())?;
    out.set("displayName", alias.info.display_name())?;
    match &alias.doc_comment {
        Some(doc) => out.set("docComment", doc.as_str())?,
        None => out.set("docComment", Null)?,
    }
    out.set(
        "sourceLocation",
        source_location_to_js(env, alias.source_location)?,
    )?;
    out.set(
        "modifiers",
        alias
            .modifiers
            .iter()
            .map(|m| m.as_str().to_string())
            .collect::<Vec<_>>(),
    )?;
    let mut anns = Vec::with_capacity(alias.annotations.len());
    for ann in &alias.annotations {
        anns.push(decoded_object_to_js(env, ann)?);
    }
    out.set("annotations", Array::from_vec(env, anns)?)?;

    let mut tparams = Vec::with_capacity(alias.type_parameters.len());
    for tp in &alias.type_parameters {
        tparams.push(type_param_to_js(env, tp)?);
    }
    out.set("typeParameters", Array::from_vec(env, tparams)?)?;
    out.set("aliasedType", ptype_to_js(env, &alias.aliased_type)?)?;
    Ok(out)
}

fn class_info_to_js<'env>(env: &'env Env, info: &PClassInfo) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("moduleName", info.module_name.as_str())?;
    out.set("simpleName", info.simple_name.as_str())?;
    out.set("qualifiedName", info.qualified_name())?;
    out.set("displayName", info.display_name())?;
    out.set("moduleUri", info.module_uri.as_str())?;
    out.set("isModuleClass", info.is_module_class)?;
    Ok(out)
}

fn alias_info_to_js<'env>(env: &'env Env, info: &TypeAliasInfo) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("moduleName", info.module_name.as_str())?;
    out.set("simpleName", info.simple_name.as_str())?;
    out.set("qualifiedName", info.qualified_name.as_str())?;
    out.set("displayName", info.display_name())?;
    Ok(out)
}

fn source_location_to_js<'env>(env: &'env Env, loc: SourceLocation) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("startLine", loc.start_line)?;
    out.set("endLine", loc.end_line)?;
    Ok(out)
}

fn type_param_to_js<'env>(env: &'env Env, tp: &TypeParameter) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("name", tp.name.as_str())?;
    out.set(
        "variance",
        match tp.variance {
            Variance::Invariant => "invariant",
            Variance::Covariant => "covariant",
            Variance::Contravariant => "contravariant",
        },
    )?;
    Ok(out)
}

fn ptype_to_js<'env>(env: &'env Env, ty: &PType) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    match ty {
        PType::Unknown => {
            out.set("kind", "unknown")?;
        }
        PType::Nothing => {
            out.set("kind", "nothing")?;
        }
        PType::Module => {
            out.set("kind", "module")?;
        }
        PType::StringLiteral(lit) => {
            out.set("kind", "stringLiteral")?;
            out.set("literal", lit.as_str())?;
        }
        PType::Class {
            class_info,
            type_arguments,
            ..
        } => {
            out.set("kind", "class")?;
            out.set("classInfo", class_info_to_js(env, class_info)?)?;
            let mut args = Vec::with_capacity(type_arguments.len());
            for a in type_arguments {
                args.push(ptype_to_js(env, a)?);
            }
            out.set("typeArguments", Array::from_vec(env, args)?)?;
        }
        PType::Nullable(inner) => {
            out.set("kind", "nullable")?;
            out.set("baseType", ptype_to_js(env, inner)?)?;
        }
        PType::Union(variants) => {
            out.set("kind", "union")?;
            let mut items = Vec::with_capacity(variants.len());
            for v in variants {
                items.push(ptype_to_js(env, v)?);
            }
            out.set("elementTypes", Array::from_vec(env, items)?)?;
        }
        PType::Function {
            parameter_types,
            return_type,
        } => {
            out.set("kind", "function")?;
            let mut params = Vec::with_capacity(parameter_types.len());
            for p in parameter_types {
                params.push(ptype_to_js(env, p)?);
            }
            out.set("parameterTypes", Array::from_vec(env, params)?)?;
            out.set("returnType", ptype_to_js(env, return_type)?)?;
        }
        PType::Constrained {
            base_type,
            constraints,
        } => {
            out.set("kind", "constrained")?;
            out.set("baseType", ptype_to_js(env, base_type)?)?;
            out.set("constraints", constraints.clone())?;
        }
        PType::Alias {
            alias_info,
            type_arguments,
            aliased_type,
            ..
        } => {
            out.set("kind", "alias")?;
            out.set("aliasInfo", alias_info_to_js(env, alias_info)?)?;
            let mut args = Vec::with_capacity(type_arguments.len());
            for a in type_arguments {
                args.push(ptype_to_js(env, a)?);
            }
            out.set("typeArguments", Array::from_vec(env, args)?)?;
            out.set("aliasedType", ptype_to_js(env, aliased_type)?)?;
        }
        PType::TypeVariable(tp) => {
            out.set("kind", "typeVariable")?;
            out.set("typeParameter", type_param_to_js(env, tp)?)?;
        }
    }
    Ok(out)
}
