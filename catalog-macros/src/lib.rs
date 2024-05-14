#![allow(unused_imports)]
#![allow(unused_variables)]

extern crate proc_macro;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::__private::TokenStream2;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use syn::ReturnType::Default;
use syn::{
    parse, parse_macro_input, parse_quote, AttributeArgs, Data, DeriveInput, Error, Item, ItemFn,
    ItemStatic, Lit, LitBool, LitByteStr, Meta, NestedMeta, Path, ReturnType, Token, Type,
};

fn ctor(crat: &Path, name: &Ident, body: &TokenStream2) -> TokenStream2 {
    let mut hasher = DefaultHasher::new();
    name.to_string().hash(&mut hasher);
    name.span().source_file().path().hash(&mut hasher);
    name.span().start().line.hash(&mut hasher);
    name.span().start().column.hash(&mut hasher);
    name.span().end().line.hash(&mut hasher);
    name.span().end().column.hash(&mut hasher);
    let hash = format!("{:016X}", hasher.finish());
    let pub_ident_fn = syn::parse_str::<syn::Ident>(
        format!("rust_registry___registry___{}_{}", name, hash).as_ref(),
    )
    .expect("Unable to create identifier");
    let pub_ident_static = syn::parse_str::<syn::Ident>(
        format!("RUST_REGISTRY___REGISTRY___{}_{}", name, hash).as_ref(),
    )
    .expect("Unable to create identifier");
    let bytes = format!("{} ", pub_ident_fn).into_bytes();
    let bytes = LitByteStr::new(&bytes, Span::call_site());
    quote! {
        #crat::reexport::cfg_if::cfg_if!(
            if #[cfg(any(target_arch = "wasm32", target_arch = "wasi"))] {
                const _: () = {
                    use #crat::reexport::wasm_bindgen;
                    #[wasm_bindgen::prelude::wasm_bindgen]
                    #[doc(hidden)]
                    pub fn #pub_ident_fn()  { #body }
                };

                #[used]
                #[link_section = "registry_ctors"]
                #[no_mangle]
                #[doc(hidden)]
                pub static #pub_ident_static: [u8; #bytes.len()] = *#bytes;

            } else {
                #[#crat::reexport::ctor]
                fn #pub_ident_fn() { #body }
            }
        );
    }
}

enum CustomArg {
    Registry(Path),
    Lazy(LitBool),
    Crat(Path),
}

struct CustomArgs {
    registry: Option<Path>,
    lazy: LitBool,
    crat: Path,
}

impl Parse for CustomArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(if input.peek2(Token![=]) {
            if input.peek(Token![crate]) {
                input.parse::<Token![crate]>()?;
                input.parse::<Token![=]>()?;
                CustomArg::Crat(input.parse()?)
            } else if input.peek(Ident::peek_any) {
                let key = input.parse::<Ident>()?.to_string();
                input.parse::<Token![=]>()?;
                match key.as_str() {
                    "lazy" => CustomArg::Lazy(input.parse()?),
                    _ => return Err(input.error("expected 'lazy' or 'crate'")),
                }
            } else {
                return Err(input.error("expected 'lazy' or 'crate'"));
            }
        } else {
            CustomArg::Registry(input.parse()?)
        })
    }
}

impl Parse for CustomArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut result = CustomArgs {
            registry: None,
            lazy: parse_quote!(false),
            crat: parse_quote!(::catalog),
        };
        for arg in input.parse_terminated::<CustomArg, Comma>(CustomArg::parse)? {
            match arg {
                CustomArg::Registry(x) => result.registry = Some(x),
                CustomArg::Lazy(x) => result.lazy = x,
                CustomArg::Crat(x) => result.crat = x,
            }
        }
        Ok(result)
    }
}

#[proc_macro_attribute]
pub fn register(args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as Item);
    let args = parse_macro_input!(args as CustomArgs);
    match register_impl(args, input) {
        Ok(x) => x.into(),
        Err(x) => x.to_compile_error().into(),
    }
}

fn register_impl(args: CustomArgs, input: Item) -> Result<TokenStream2, Error> {
    // let mut catalog = None;
    // let mut lazy = false;
    // let mut crat = quote!( ::catalog );
    // for arg in &args {
    //     match arg {
    //         NestedMeta::Meta(meta) => {
    //             match meta {
    //                 Meta::Path(x) => catalog = Some(x),
    //                 Meta::List(x) => return Err(Error::new(x.span(), "Cannot use List as arg")),
    //                 Meta::NameValue(namevalue) => {
    //                     if namevalue.path.segments.len() == 1 && namevalue.path.segments[0].ident.to_string() == "lazy" {
    //                         match &namevalue.lit {
    //                             Lit::Bool(v) => lazy = v.value,
    //                             _ => return Err(Error::new(namevalue.span(), "Argument to `lazy` must be bool")),
    //                         }
    //                     } else {
    //                         return Err(Error::new(namevalue.span(), "The only valid key is `lazy`"));
    //                     }
    //                 }
    //             }
    //         }
    //         NestedMeta::Lit(x) => return Err(Error::new(x.span(), "Cannot use List as arg")),
    //     }
    // }
    let registry = args
        .registry
        .ok_or_else(|| Error::new(input.span(), "Must specify catalog"))?;
    let crat = &args.crat;
    let lazy = args.lazy.value;
    match &input {
        Item::Fn(f) => {
            let name = &f.sig.ident;
            let ctored = ctor(
                crat,
                name,
                &quote! {
                    #crat::Registry::register(&#registry, |x| {
                        #crat::BuilderFrom::insert(x, #name());
                    })
                },
            );
            Ok(quote! {
                #f
                #ctored
            })
        }
        Item::Static(s) => {
            if lazy {
                let ItemStatic {
                    attrs,
                    vis,
                    static_token,
                    mutability,
                    ident,
                    colon_token,
                    ty,
                    eq_token,
                    expr,
                    semi_token,
                } = s;
                if let Some(mutability) = mutability {
                    return Err(Error::new(mutability.span(), "Cannot use mutable statics."));
                }
                let ctored = ctor(
                    crat,
                    &ident,
                    &quote! {
                        #crat::Registry::register(&#registry, |x| {
                            #crat::BuilderFrom::insert(x, #crat::LazyEntry::__private(&#ident))
                        })
                    },
                );
                Ok(quote! {
                    #ctored
                    #( #attrs )*
                    #vis #static_token #ident #colon_token #crat::LazyEntry<#ty> #eq_token
                        #crat::LazyEntry::new(
                            || #expr,
                            || {
                                ::std::mem::drop(::std::ops::Deref::deref(&#registry));
                                #crat::LazyEntry::__private(&#ident)
                            }
                        )
                     #semi_token
                })
            } else {
                let name = &s.ident;
                let ctored = ctor(
                    crat,
                    &s.ident,
                    &quote! {
                        #crat::Registry::register(&#registry, |x| {
                            #crat::BuilderFrom::insert(x, &#name)
                        })
                    },
                );
                Ok(quote! {
                    #ctored
                    #s
                })
            }
        }
        _ => {
            return Err(Error::new(
                input.span(),
                "Macro only applies to functions and statics.",
            ))
        }
    }
}
