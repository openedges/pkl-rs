//! Property-name candidates from the pinned Pkl 0.32.0 base module.
//! These diagnostic names do not imply that their runtime values are implemented.

pub(crate) const BASE_PROPERTIES: &[&str] = &[
    "Any",
    "Null",
    "NonNull",
    "Class",
    "TypeAlias",
    "Module",
    "Annotation",
    "Since",
    "Deprecated",
    "AlsoKnownAs",
    "Unlisted",
    "DocExample",
    "SourceCode",
    "ModuleInfo",
    "FileOutput",
    "ModuleOutput",
    "BaseValueRenderer",
    "ConvertProperty",
    "ValueRenderer",
    "BytesRenderer",
    "PcfRenderer",
    "RenderDirective",
    "PcfRenderDirective",
    "JsonRenderer",
    "YamlRenderer",
    "PListRenderer",
    "PropertiesRenderer",
    "Resource",
    "Number",
    "Int",
    "Int8",
    "Int16",
    "Int32",
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt",
    "Comparable",
    "Float",
    "Boolean",
    "Char",
    "String",
    "Charset",
    "Uri",
    "Regex",
    "RegexMatch",
    "DurationUnit",
    "Duration",
    "DataSizeUnit",
    "DataSize",
    "Object",
    "Typed",
    "Dynamic",
    "Listing",
    "Mapping",
    "Function",
    "Function0",
    "Function1",
    "Function2",
    "Function3",
    "Function4",
    "Function5",
    "Mixin",
    "Pair",
    "Collection",
    "IntSeq",
    "VarArgs",
    "List",
    "Set",
    "Map",
    "Bytes",
    "NaN",
    "Infinity",
];

/// Inherited signatures from pinned base.pkl Any and Typed, in declaration
/// lookup order. Listing a diagnostic candidate does not implement its body.
pub(crate) const TYPED_METHODS: &[(&str, &str, usize)] = &[
    ("getClass", "getClass()", 0),
    ("toString", "toString()", 0),
    ("ifNonNull", "ifNonNull(transform)", 1),
    ("hasProperty", "hasProperty(name)", 1),
    ("getProperty", "getProperty(name)", 1),
    ("getPropertyOrNull", "getPropertyOrNull(name)", 1),
    ("toDynamic", "toDynamic()", 0),
    ("toMap", "toMap()", 0),
];

/// Pinned Module methods, including Any/Typed inheritance. These are diagnostic
/// signatures only; listing a candidate does not implement its callable body.
pub(crate) const MODULE_METHODS: &[(&str, &str, usize)] = &[
    ("getClass", "getClass()", 0),
    ("toString", "toString()", 0),
    ("ifNonNull", "ifNonNull(transform)", 1),
    ("hasProperty", "hasProperty(name)", 1),
    ("getProperty", "getProperty(name)", 1),
    ("getPropertyOrNull", "getPropertyOrNull(name)", 1),
    ("toDynamic", "toDynamic()", 0),
    ("toMap", "toMap()", 0),
    ("relativePathTo", "relativePathTo(other)", 1),
];

/// Top-level functions in pinned base.pkl, in declaration order.
pub(crate) const BASE_METHODS: &[(&str, &str, usize)] = &[
    ("Regex", "Regex(pattern)", 1),
    ("Undefined", "Undefined()", 0),
    ("TODO", "TODO()", 0),
    ("Null", "Null(defaultValue)", 1),
    ("Pair", "Pair(first, second)", 2),
    ("IntSeq", "IntSeq(start, end)", 2),
    ("List", "List(elements)", 1),
    ("Set", "Set(elements)", 1),
    ("Map", "Map(keysAndValues)", 1),
    ("Bytes", "Bytes(values)", 1),
];

/// Pinned Any and Dynamic method signatures, in inherited declaration order.
/// Diagnostic candidates do not imply that every method body is implemented.
pub(crate) const DYNAMIC_METHODS: &[(&str, &str, usize)] = &[
    ("getClass", "getClass()", 0),
    ("toString", "toString()", 0),
    ("ifNonNull", "ifNonNull(transform)", 1),
    ("length", "length()", 0),
    ("hasProperty", "hasProperty(name)", 1),
    ("getProperty", "getProperty(name)", 1),
    ("getPropertyOrNull", "getPropertyOrNull(name)", 1),
    ("toMap", "toMap()", 0),
    ("toList", "toList()", 0),
    ("toTyped", "toTyped(clazz)", 1),
];

/// Pinned List declarations plus inherited Any methods; diagnostic candidates only.
pub(crate) const LIST_PROPERTIES: &[&str] = &[
    "length",
    "isEmpty",
    "isNotEmpty",
    "lastIndex",
    "first",
    "firstOrNull",
    "rest",
    "restOrNull",
    "last",
    "lastOrNull",
    "single",
    "singleOrNull",
    "isDistinct",
    "distinct",
    "min",
    "minOrNull",
    "max",
    "maxOrNull",
];
pub(crate) const LIST_METHODS: &[(&str, &str, usize)] = &[
    ("getClass", "getClass()", 0),
    ("toString", "toString()", 0),
    ("ifNonNull", "ifNonNull(transform)", 1),
    ("getOrNull", "getOrNull(index)", 1),
    ("sublist", "sublist(start, exclusiveEnd)", 2),
    ("sublistOrNull", "sublistOrNull(start, exclusiveEnd)", 2),
    ("contains", "contains(element)", 1),
    ("startsWith", "startsWith(coll)", 1),
    ("endsWith", "endsWith(coll)", 1),
    ("split", "split(index)", 1),
    ("splitOrNull", "splitOrNull(index)", 1),
    ("partition", "partition(predicate)", 1),
    ("filter", "filter(predicate)", 1),
    ("filterNonNull", "filterNonNull()", 0),
    ("map", "map(transform)", 1),
    ("mapNonNull", "mapNonNull(transform)", 1),
    ("flatMap", "flatMap(transform)", 1),
    ("filterIndexed", "filterIndexed(predicate)", 1),
    ("mapIndexed", "mapIndexed(transform)", 1),
    ("mapNonNullIndexed", "mapNonNullIndexed(transform)", 1),
    ("flatMapIndexed", "flatMapIndexed(transform)", 1),
    ("filterIsInstance", "filterIsInstance(clazz)", 1),
    ("isDistinctBy", "isDistinctBy(selector)", 1),
    ("distinctBy", "distinctBy(selector)", 1),
    ("flatten", "flatten()", 0),
    ("every", "every(predicate)", 1),
    ("any", "any(predicate)", 1),
    ("add", "add(element)", 1),
    ("replace", "replace(index, replacement)", 2),
    ("replaceOrNull", "replaceOrNull(index, replacement)", 2),
    (
        "replaceRange",
        "replaceRange(start, exclusiveEnd, replacement)",
        3,
    ),
    (
        "replaceRangeOrNull",
        "replaceRangeOrNull(start, exclusiveEnd, replacement)",
        3,
    ),
    ("find", "find(predicate)", 1),
    ("findOrNull", "findOrNull(predicate)", 1),
    ("findLast", "findLast(predicate)", 1),
    ("findLastOrNull", "findLastOrNull(predicate)", 1),
    ("findIndex", "findIndex(predicate)", 1),
    ("findIndexOrNull", "findIndexOrNull(predicate)", 1),
    ("findLastIndex", "findLastIndex(predicate)", 1),
    ("findLastIndexOrNull", "findLastIndexOrNull(predicate)", 1),
    ("indexOf", "indexOf(element)", 1),
    ("indexOfOrNull", "indexOfOrNull(element)", 1),
    ("lastIndexOf", "lastIndexOf(element)", 1),
    ("lastIndexOfOrNull", "lastIndexOfOrNull(element)", 1),
    ("count", "count(predicate)", 1),
    ("take", "take(n)", 1),
    ("takeWhile", "takeWhile(predicate)", 1),
    ("takeLast", "takeLast(n)", 1),
    ("takeLastWhile", "takeLastWhile(predicate)", 1),
    ("drop", "drop(n)", 1),
    ("dropWhile", "dropWhile(predicate)", 1),
    ("dropLast", "dropLast(n)", 1),
    ("dropLastWhile", "dropLastWhile(predicate)", 1),
    ("fold", "fold(initial, operator)", 2),
    ("foldBack", "foldBack(initial, operator)", 2),
    ("foldIndexed", "foldIndexed(initial, operator)", 2),
    ("reduce", "reduce(operator)", 1),
    ("reduceOrNull", "reduceOrNull(operator)", 1),
    ("groupBy", "groupBy(selector)", 1),
    ("repeat", "repeat(n)", 1),
    ("minBy", "minBy(selector)", 1),
    ("minByOrNull", "minByOrNull(selector)", 1),
    ("minWith", "minWith(comparator)", 1),
    ("minWithOrNull", "minWithOrNull(comparator)", 1),
    ("maxBy", "maxBy(selector)", 1),
    ("maxByOrNull", "maxByOrNull(selector)", 1),
    ("maxWith", "maxWith(comparator)", 1),
    ("maxWithOrNull", "maxWithOrNull(comparator)", 1),
    ("sort", "sort()", 0),
    ("sortBy", "sortBy(selector)", 1),
    ("sortWith", "sortWith(comparator)", 1),
    ("reverse", "reverse()", 0),
    ("zip", "zip(coll)", 1),
    ("join", "join(separator)", 1),
    ("toList", "toList()", 0),
    ("toSet", "toSet()", 0),
    ("toMap", "toMap(keyExtractor, valueExtractor)", 2),
    ("toListing", "toListing()", 0),
    ("toDynamic", "toDynamic()", 0),
    ("toBytes", "toBytes()", 0),
];

/// Source-derived candidates for the lazy and strict collection classes.
/// This catalog describes declarations, including still unsupported methods.
pub(crate) fn collection_members(class: &str) -> (Vec<String>, Vec<(String, String, usize)>) {
    builtin_members(class)
}

/// Source-derived scalar and collection names, with their declared ancestry.
pub(crate) fn builtin_members(class: &str) -> (Vec<String>, Vec<(String, String, usize)>) {
    type Catalog = std::collections::BTreeMap<String, (Vec<String>, Vec<(String, String, usize)>)>;
    static CATALOG: std::sync::OnceLock<Catalog> = std::sync::OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        let parsed = crate::parser::parse_module(&crate::stdlib_sources::base().text)
            .expect("pinned base declarations");
        let mut catalog = Catalog::new();
        for declaration in &parsed.members {
            let crate::syntax::ExprKind::Class { members, .. } = &declaration.expr.kind else {
                continue;
            };
            let mut properties = Vec::new();
            let mut methods = Vec::new();
            for member in members.iter().filter(|member| !member.local) {
                if let crate::syntax::ExprKind::Function {
                    parameters,
                    method: true,
                    ..
                } = &member.expr.kind
                {
                    let signature = format!(
                        "{}({})",
                        member.name,
                        parameters
                            .iter()
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                    methods.push((member.name.clone(), signature, parameters.len()));
                } else {
                    properties.push(member.name.clone());
                }
            }
            catalog.insert(declaration.name.clone(), (properties, methods));
        }
        catalog
    });
    let mut properties = Vec::new();
    let mut methods: Vec<(String, String, usize)> = Vec::new();
    let parents: &[&str] = match class {
        "List" | "Set" => &["Any", "Collection"],
        "Int" | "Float" => &["Any", "Number"],
        "String" | "Boolean" | "Null" => &["Any"],
        _ => &["Any", "Object"],
    };
    for name in parents.iter().copied().chain(std::iter::once(class)) {
        if let Some((props, funcs)) = catalog.get(name) {
            properties.extend(props.iter().cloned());
            for candidate in funcs {
                if let Some(existing) = methods.iter_mut().find(|method| method.0 == candidate.0) {
                    *existing = candidate.clone();
                } else {
                    methods.push(candidate.clone());
                }
            }
        }
    }
    (properties, methods)
}
