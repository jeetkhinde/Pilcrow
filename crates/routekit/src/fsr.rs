use std::io;
use std::path::Path;
use quote::ToTokens;

/// Field extracted from the `Live` struct in `live.rs`.
pub struct LiveField {
    pub name: String,
    /// The inner type T in LiveProps<T>.
    pub inner_type: String,
}

/// Process a `live.rs` file: strip `#[pilcrow::*]` attrs, extract LiveProps fields,
/// and generate a `from_row()` impl.
///
/// Returns (processed_source, live_fields).
pub fn process_live_rs(path: &Path) -> io::Result<(String, Vec<LiveField>)> {
    let source = std::fs::read_to_string(path)?;
    let mut file: syn::File = syn::parse_str(&source).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse live.rs: {e}"),
        )
    })?;

    // Find the Live struct and extract + clean its fields.
    let mut live_fields: Vec<LiveField> = Vec::new();

    for item in &mut file.items {
        let syn::Item::Struct(s) = item else { continue };
        if s.ident != "Live" {
            continue;
        }

        let syn::Fields::Named(named) = &mut s.fields else {
            continue;
        };
        for field in &mut named.named {
            // Strip all #[pilcrow::*] attributes.
            field.attrs.retain(|attr| {
                !attr
                    .path()
                    .segments
                    .first()
                    .is_some_and(|seg| seg.ident == "pilcrow")
            });

            // Collect LiveProps<T> fields.
            let Some(ident) = &field.ident else { continue };
            let is_live_props = if let syn::Type::Path(tp) = &field.ty {
                tp.path
                    .segments
                    .last()
                    .is_some_and(|seg| seg.ident == "LiveProps")
            } else {
                false
            };

            if is_live_props {
                // Extract inner type T from LiveProps<T>.
                let inner = extract_live_props_inner(&field.ty)
                    .unwrap_or_else(|| "serde_json::Value".to_string());
                live_fields.push(LiveField {
                    name: ident.to_string(),
                    inner_type: inner,
                });
            }
        }
        break;
    }

    // Generate from_row() impl and append to file.
    if !live_fields.is_empty() {
        let from_row_impl = generate_from_row_impl(&live_fields);
        let mut out = file.into_token_stream().to_string();
        out.push('\n');
        out.push_str(&from_row_impl);
        return Ok((out, live_fields));
    }

    Ok((file.into_token_stream().to_string(), live_fields))
}

fn extract_live_props_inner(ty: &syn::Type) -> Option<String> {
    if let syn::Type::Path(tp) = ty {
        let seg = tp.path.segments.last()?;
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
            let first = args.args.first()?;
            return Some(first.to_token_stream().to_string());
        }
    }
    None
}

fn generate_from_row_impl(fields: &[LiveField]) -> String {
    let mut out = String::from("impl ::pilcrow_runtime::fsr::PilcrowLive for Live {\n");
    out.push_str("    fn query(_params: &::serde_json::Map<String, ::serde_json::Value>) -> ::pilcrow_runtime::fsr::LiveQuery {\n");
    out.push_str(
        "        unimplemented!(\"Live::query() must be implemented in live.rs\")\n",
    );
    out.push_str("    }\n");
    out.push_str("    fn from_row(row: &::std::collections::HashMap<String, ::serde_json::Value>) -> Self {\n");
    out.push_str("        Self {\n");
    for field in fields {
        let name = &field.name;
        out.push_str(&format!(
            "            {name}: ::pilcrow_runtime::fsr::LiveProps {{\n"
        ));
        out.push_str(&format!(
            "                value: row.get(\"{name}\")\n"
        ));
        out.push_str(
            "                    .and_then(|v| ::serde_json::from_value(v.clone()).ok())\n",
        );
        out.push_str("                    .unwrap_or_default(),\n");
        out.push_str("                depends_on: ::std::vec![],\n");
        out.push_str("                promote_after: ::std::option::Option::None,\n");
        out.push_str("                patch_debounce: ::std::option::Option::None,\n");
        out.push_str("            },\n");
    }
    out.push_str("        }\n    }\n}\n");
    out
}
