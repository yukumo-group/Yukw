//! Rewrite minified embind imports onto `env.<semantic>`.

use std::collections::HashMap;

use engine::{IMPORT_MODULE, ImportBinding, Semantic};
use wasm_encoder::reencode::{Error as ReencodeError, Reencode};
use wasm_encoder::{ImportSection, Module};
use wasmparser::{ImportSectionReader, Imports, Parser};

struct Renamer<'a> {
    map: &'a HashMap<&'a str, Semantic>,
}

impl Reencode for Renamer<'_> {
    type Error = String;

    fn parse_import_section(
        &mut self,
        imports: &mut ImportSection,
        section: ImportSectionReader<'_>,
    ) -> Result<(), ReencodeError<String>> {
        for group in section {
            let group = group?;
            match group {
                Imports::Single(_, import) => {
                    let (module, name) = renamed(import.module, import.name, self.map)?;
                    let ty = self.entity_type(import.ty)?;
                    imports.import(module, name, ty);
                }
                Imports::Compact1 { module, items } => {
                    for item in items {
                        let item = item?;
                        let (module, name) = renamed(module, item.name, self.map)?;
                        let ty = self.entity_type(item.ty)?;
                        imports.import(module, name, ty);
                    }
                }
                Imports::Compact2 { module, ty, names } => {
                    let ty = self.entity_type(ty)?;
                    for name in names {
                        let name = name?;
                        let (module, name) = renamed(module, name, self.map)?;
                        imports.import(module, name, ty.clone());
                    }
                }
            }
        }
        Ok(())
    }
}

fn renamed<'a>(
    module: &'a str,
    field: &'a str,
    map: &HashMap<&str, Semantic>,
) -> Result<(&'a str, &'a str), ReencodeError<String>> {
    match map.get(field) {
        Some(semantic) => Ok((IMPORT_MODULE, semantic.js_name())),
        None => Err(ReencodeError::UserError(format!(
            "import {module}.{field} has no semantic name"
        ))),
    }
}

/// Replace import field names using `bindings`. Function indices stay the same.
pub fn rewrite_imports(bytes: &[u8], bindings: &[ImportBinding]) -> Result<Vec<u8>, String> {
    let map = bindings
        .iter()
        .map(|binding| (binding.field.as_str(), binding.semantic))
        .collect();
    let mut module = Module::new();
    Renamer { map: &map }
        .parse_core_module(&mut module, Parser::new(0), bytes)
        .map_err(|err| err.to_string())?;
    let bytes = module.finish();
    wasmparser::validate(&bytes).map_err(|err| err.to_string())?;
    Ok(bytes)
}

/// `(module, field)` of each import, in order.
pub fn import_names(bytes: &[u8]) -> Result<Vec<(String, String)>, String> {
    let mut names = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        let payload = payload.map_err(|err| err.to_string())?;
        if let wasmparser::Payload::ImportSection(section) = payload {
            for group in section {
                let group = group.map_err(|err| err.to_string())?;
                match group {
                    Imports::Single(_, import) => {
                        names.push((import.module.to_string(), import.name.to_string()));
                    }
                    Imports::Compact1 { module, items } => {
                        for item in items {
                            let item = item.map_err(|err| err.to_string())?;
                            names.push((module.to_string(), item.name.to_string()));
                        }
                    }
                    Imports::Compact2 {
                        module,
                        names: fields,
                        ..
                    } => {
                        for name in fields {
                            let name = name.map_err(|err| err.to_string())?;
                            names.push((module.to_string(), name.to_string()));
                        }
                    }
                }
            }
        }
    }
    Ok(names)
}
