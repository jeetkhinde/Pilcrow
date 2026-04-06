use proc_macro::TokenStream;
use quote::quote;
use syn::{FnArg, Ident, ItemFn, Pat, PatType, parse_macro_input, visit::Visit};

pub fn expand(item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);

    let uses_client = body_uses_client(&func);

    let mut extra_params: Vec<proc_macro2::TokenStream> = vec![];

    if uses_client {
        extra_params.push(quote! {
            __pilcrow_client: ::pilcrow_client::PilcrowClient
        });
    }

    // Rewrite known params
    let mut rewritten: Vec<proc_macro2::TokenStream> = vec![];

    for param in &func.sig.inputs {
        if let FnArg::Typed(PatType { pat, ty, .. }) = param
            && let Pat::Ident(ident) = pat.as_ref()
        {
            let name = ident.ident.to_string();
            match name.as_str() {
                "form" => {
                    rewritten.push(quote! {
                        ::axum::Form(#pat): ::axum::Form<#ty>
                    });
                    continue;
                }
                "json" => {
                    rewritten.push(quote! {
                        ::axum::Json(#pat): ::axum::Json<#ty>
                    });
                    continue;
                }
                "path" => {
                    rewritten.push(quote! {
                        ::axum::extract::Path(#pat): ::axum::extract::Path<#ty>
                    });
                    continue;
                }
                _ => {}
            }
        }
        rewritten.push(quote! { #param });
    }

    // Build final param list: client first, then rewritten params
    let all_params = extra_params.iter().chain(rewritten.iter());

    // Inject `let client = __pilcrow_client;` at top of body if needed
    let client_binding = if uses_client {
        quote! { let client = __pilcrow_client; }
    } else {
        quote! {}
    };

    let vis = &func.vis;
    let sig_ident = &func.sig.ident;
    let body = &func.block;
    // let ret = &func.sig.output;

    let expanded = quote! {
        #vis async fn #sig_ident(#(#all_params),*) -> ::pilcrow_web::AppResult<::axum::response::Response> {
            use ::axum::response::IntoResponse;
            #client_binding
            let __result = (|| async move {
                #body
            })().await;
            match __result {
                Ok(r) => Ok(r.into_response()),
                Err(e) => Err(e),
            }
        }
    };

    expanded.into()
}

// Walk the function body AST looking for any `client` identifier
struct ClientVisitor {
    found: bool,
}

impl<'ast> Visit<'ast> for ClientVisitor {
    fn visit_ident(&mut self, ident: &'ast Ident) {
        if ident == "client" {
            self.found = true;
        }
    }
}

fn body_uses_client(func: &ItemFn) -> bool {
    let mut visitor = ClientVisitor { found: false };
    visitor.visit_block(&func.block);
    visitor.found
}
