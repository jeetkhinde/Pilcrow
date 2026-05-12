use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, Expr, LitStr, Token,
};
mod handler;
mod live_props_derive;

#[proc_macro_attribute]
pub fn handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    handler::expand(item)
}

#[proc_macro_derive(PilcrowProps, attributes(promote_after, patch_debounce))]
pub fn derive_pilcrow_props(input: TokenStream) -> TokenStream {
    live_props_derive::expand(input)
}

struct SseEntry {
    stream: Expr,
    target: LitStr,
}

struct SseInput {
    entries: Vec<SseEntry>,
}

impl Parse for SseEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let stream = input.parse::<Expr>()?;
        input.parse::<Token![=>]>()?;
        let target = input.parse::<LitStr>()?;
        Ok(Self { stream, target })
    }
}

impl Parse for SseInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut entries = Vec::new();
        while !input.is_empty() {
            entries.push(input.parse::<SseEntry>()?);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }
        Ok(Self { entries })
    }
}

#[proc_macro]
pub fn sse(input: TokenStream) -> TokenStream {
    let SseInput { entries } = parse_macro_input!(input as SseInput);

    let futures: Vec<proc_macro2::TokenStream> = entries
        .iter()
        .map(|e| {
            let stream = &e.stream;
            let target = &e.target;
            quote! {
                #stream.json(#target, &__emit)
            }
        })
        .collect();

    let expanded = if futures.len() == 1 {
        let f = &futures[0];
        quote! {
            ::pilcrow::sse_stream(|__emit| async move {
                #f.await
            })
        }
    } else {
        let combined =
            futures
                .iter()
                .enumerate()
                .fold(proc_macro2::TokenStream::new(), |mut acc, (i, f)| {
                    if i > 0 {
                        acc.extend(quote! { , });
                    }
                    acc.extend(quote! { #f });
                    acc
                });

        quote! {
            ::pilcrow::sse_stream(|__emit| async move {
                ::pilcrow::combine!(#combined).await
            })
        }
    };

    expanded.into()
}
