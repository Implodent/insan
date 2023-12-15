use proc_macro::TokenStream as TS;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::ToTokens;
use syn::{
    parenthesized, parse2, parse_quote, spanned::Spanned, token, Data, DeriveInput, Expr, ExprLit,
    Field, Fields, Item, ItemStruct, Lit, Meta, MetaList, MetaNameValue, Result, Token, Type,
};

#[proc_macro]
pub fn endpoint_error(args: TS) -> TS {
    _endpoint_error(args.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn _endpoint_error(args: TokenStream) -> Result<TokenStream> {
    let er = parse2::<Type>(args)?;

    Ok(quote::quote! {
        #[doc(hidden)]
        #[allow(dead_code, non_snake_case)]
        type __Endpoint_Error = #er;
    })
}

#[proc_macro_derive(ClientEndpoint, attributes(endpoint, required))]
pub fn endpoint(item: TS) -> TS {
    _endpoint(item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[derive(Default, Clone, Copy)]
enum MetaMode {
    Json,
    Query,
    Display,
    #[default]
    Empty,
}

mod kw {
    syn::custom_keyword!(json);
    syn::custom_keyword!(query);
    syn::custom_keyword!(display);
    syn::custom_keyword!(empty);
}

impl syn::parse::Parse for MetaMode {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        let lo = input.lookahead1();

        let token = if lo.peek(kw::json) {
            input.parse::<kw::json>()?;
            Self::Json
        } else if lo.peek(kw::query) {
            input.parse::<kw::query>()?;
            Self::Query
        } else if lo.peek(kw::display) {
            input.parse::<kw::display>()?;
            Self::Display
        } else if lo.peek(kw::empty) {
            input.parse::<kw::empty>()?;
            Self::Empty
        } else {
            return Err(lo.error());
        };

        Ok(token)
    }
}

struct EndpointMeta {
    pub method: Ident,
    pub mode: (MetaMode, MetaMode),
    pub path: Expr,
    pub client: Type,
    pub output: Type,
}

impl syn::parse::Parse for EndpointMeta {
    fn parse(input: syn::parse::ParseStream) -> Result<Self> {
        Ok(Self {
            method: input.parse()?,
            mode: if input.peek(token::Paren) {
                let real;
                parenthesized!(real in input);
                let inp = real.parse::<MetaMode>()?;
                let out = if real.peek(Token![,]) {
                    real.parse::<Token![,]>()?;

                    real.parse::<MetaMode>()?
                } else {
                    if let MetaMode::Display = inp {
                        MetaMode::Display
                    } else {
                        MetaMode::Json
                    }
                };

                (inp, out)
            } else {
                (MetaMode::default(), MetaMode::Json)
            },
            path: input.parse()?,
            client: {
                input.parse::<Token![in]>()?;
                input.parse()?
            },
            output: if input.peek(Token![->]) {
                input.parse::<Token![->]>()?;
                input.parse()?
            } else {
                parse_quote!(())
            },
        })
    }
}

fn _endpoint(item: TokenStream) -> Result<TokenStream> {
    let input: DeriveInput = parse2(item)?;

    let meta = parse2::<EndpointMeta>(
        input
            .attrs
            .iter()
            .find_map(|x| match &x.meta {
                Meta::List(MetaList { path, tokens, .. }) if path.is_ident("endpoint") => {
                    Some(tokens.clone())
                }
                _ => None,
            })
            .unwrap(),
    )?;

    let ((setup, desetup), url) = match input.data {
        // wontfix
        Data::Union(_) => {
            return Err(syn::Error::new(
                input.span(),
                "unions are not supported as endpoints",
            ))
        }
        // todo
        Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                input,
                "enums are currently not supported as endpoints",
            ))
        }
        Data::Struct(s) => match s.fields {
            Fields::Unit => (
                (
                    quote::quote!(),
                    match meta.mode.1 {
                        MetaMode::Json => quote::quote!(response.body_json().await?),
                        MetaMode::Display => quote::quote!(response.body_string().await?),
                        MetaMode::Query => todo!(),
                        MetaMode::Empty => quote::quote!(()),
                    },
                ),
                meta.path.to_token_stream(),
            ),
            Fields::Named(named) => (
                (
                    match meta.mode.0 {
                        MetaMode::Json => {
                            quote::quote! { request.set_body(http_types::Body::from_json(self)?); }
                        }
                        MetaMode::Query => quote::quote! {
                            self.serialize(acril::serde_urlencoded::Serializer::new(&mut request.url_mut().query_pairs_mut()))?;

                            if let Some("") = request.url().query() {
                                request.url_mut().set_query(None);
                            }
                        },
                        MetaMode::Display => quote::quote! {
                            request.set_body(self.to_string());
                        },
                        MetaMode::Empty => quote::quote!(),
                    },
                    match meta.mode.1 {
                        MetaMode::Json => quote::quote!(response.body_json().await?),
                        MetaMode::Display => quote::quote!(response.body_string().await?),
                        MetaMode::Query => todo!(),
                        MetaMode::Empty => quote::quote!(()),
                    },
                ),
                {
                    let fields = named.named.into_iter().filter_map(|x| x.ident);
                    let murl = if !matches!(&meta.path, Expr::Paren(_)) {
                        let m = meta.path;
                        quote::quote!(format!(#m))
                    } else {
                        meta.path.into_token_stream()
                    };

                    quote::quote! {{
                        let Self { #(#fields,)* } = self;
                        #murl
                    }}
                },
            ),
            Fields::Unnamed(tup) => (
                (
                    match meta.mode.0 {
                        MetaMode::Json => {
                            quote::quote! { request.set_body(http_types::Body::from_json(self)?) }
                        }
                        MetaMode::Query => quote::quote! {
                            self.serialize(acril::serde_urlencoded::Serializer::new(&mut request.url_mut().query_pairs_mut()))?;

                            if let Some("") = request.url().query() {
                                request.url_mut().set_query(None);
                            }
                        },
                        MetaMode::Display => quote::quote! {
                            request.set_body(self.to_string());
                        },
                        MetaMode::Empty => quote::quote!(),
                    },
                    match meta.mode.1 {
                        MetaMode::Json => quote::quote!(response.body_json().await?),
                        MetaMode::Display => quote::quote!(response.body_string().await?),
                        MetaMode::Query => todo!(),
                        MetaMode::Empty => quote::quote!(()),
                    },
                ),
                {
                    let fields = tup
                        .unnamed
                        .into_iter()
                        .enumerate()
                        .map(|(idx, _)| Ident::new(&format!("_{idx}"), Span::call_site()));
                    let murl = if !matches!(&meta.path, Expr::Paren(_)) {
                        let m = meta.path;
                        quote::quote!(format!(#m))
                    } else {
                        meta.path.into_token_stream()
                    };

                    quote::quote! {{
                                            let Self(#(#fields,)*) = self;
                    #murl
                                        }}
                },
            ),
        },
    };

    let ident = input.ident;
    let EndpointMeta {
        client,
        method,
        output,
        ..
    } = meta;

    Ok(quote::quote! {
        impl Service for #ident {
            type Context = #client;
            type Error = __Endpoint_Error;
        }

        impl ClientEndpoint for #ident {
            type Output = #output;

            #[allow(unused)]
            async fn run(&self, client: &mut Self::Context) -> Result<Self::Output, Self::Error> {
                let mut request = client.new_request(Method::#method, &{#url});
                #setup

                let mut response = client.run_request(request).await?;
                Ok(#desetup)
            }
        }
    })
}

#[proc_macro_attribute]
pub fn with_builder(args: TS, item: TS) -> TS {
    _with_builder(args.into(), item.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn _with_builder(args: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let mut item = parse2::<Item>(item)?;
    let value = {
        let (stru, ident, struct_fields, meta) = match &item {
            Item::Struct(s) => (
                s,
                s.ident.clone(),
                &s.fields,
                parse2::<EndpointMeta>(
                    s.attrs
                        .iter()
                        .find_map(|x| {
                            if x.path().is_ident("endpoint") {
                                Some(x.meta.require_list().ok()?.tokens.clone())
                            } else {
                                None
                            }
                        })
                        .expect("no endpoint attribute"),
                )?,
            ),
            _ => unreachable!(),
        };
        let client = meta.client;
        let output = meta.output;
        let bld = Ident::new(&format!("{ident}Builder"), ident.span());
        let doc = stru.attrs.iter().find_map(|x| {
                    if x.path().is_ident("doc") {
                        if let Meta::NameValue(MetaNameValue { value: Expr::Lit(ExprLit { lit: Lit::Str(st), .. }), .. }) = &x.meta {
                            let value = Literal::string(&format!("{}\nThis function returns a builder, so you can configure the request and send it with [`{}::execute`].", st.value(), bld.to_string()));
                            Some(quote::quote!(#[doc = #value]))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });
        let doc_unit = stru.attrs.iter().find_map(|x| {
            if x.path().is_ident("doc") {
                if let Meta::NameValue(MetaNameValue { value, .. }) = &x.meta {
                    Some(quote::quote!(#[doc = #value]))
                } else {
                    None
                }
            } else {
                None
            }
        });
        let fields = match struct_fields {
            Fields::Unit => {
                return Ok(if let Ok(method) = parse2::<Ident>(args) {
                    quote::quote! {
                        #item

                        impl #client {
                            #doc_unit
                            pub async fn #method(&self) -> Result<#output, __Endpoint_Error> {
                                #ident.run(self).await
                            }
                        }
                    }
                } else {
                    TokenStream::new()
                })
            }
            Fields::Named(named) => &named.named,
            Fields::Unnamed(unnamed) => {
                return Err(syn::Error::new_spanned(
                    unnamed,
                    "#[with_builder] does not support tuple structs",
                ))
            }
        };

        let field_methods = fields.iter().map(
            |Field {
                 ident, ty, attrs, ..
             }| {
                let doc = attrs.iter().find_map(|x| {
                    if x.path().is_ident("doc") {
                        if let Meta::NameValue(MetaNameValue { value, .. }) = &x.meta {
                            Some(quote::quote!(#[doc = #value]))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                quote::quote! {
                #doc
                                pub fn #ident(mut self, #ident: impl Into<#ty>) -> Self {
                        self.1.#ident = Into::<#ty>::into(#ident);
                        self
                                }
                            }
            },
        );

        let client_impl = if let Ok(method) = parse2::<Ident>(args) {
            if fields
                .iter()
                .all(|x| x.attrs.iter().any(|a| a.path().is_ident("required")))
            {
                let args = fields
                    .iter()
                    .map(|Field { ident, ty, .. }| quote::quote!(#ident: #ty));
                let self_init_fields = fields
                    .iter()
                    .map(|Field { ident, .. }| quote::quote!(#ident));

                Some(quote::quote! {
                impl #client {
                pub async fn #method(&self, #(#args,)*) -> Result<#output, __Endpoint_Error> {
                    #ident {#(#self_init_fields,)*}.run(self).await
                }
                }
                            })
            } else {
                let args = fields.iter().filter_map(
                    |Field {
                         attrs, ident, ty, ..
                     }| {
                        if attrs.iter().any(|a| a.path().is_ident("required")) {
                            Some(quote::quote!(#ident: #ty))
                        } else {
                            None
                        }
                    },
                );
                let self_init_fields = fields.iter().map(
                    |Field {
                         attrs, ident, ty, ..
                     }| {
                        if attrs.iter().any(|a| a.path().is_ident("required")) {
                            quote::quote!(#ident)
                        } else {
                            quote::quote!(#ident: <#ty as Default>::default())
                        }
                    },
                );

                Some(quote::quote! {
                        impl #client {
                pub fn #method(&mut self, #(#args,)*) -> #bld<'_> {
                    #bld(self, #ident { #(#self_init_fields,)* })
                }
                        }
                                })
            }
        } else {
            None
        };

        quote::quote! {
            #item

            #doc
            pub struct #bld<'a>(&'a mut #client, #ident);
            impl<'a> #bld<'a> {
                pub async fn execute(self) -> Result<#output, __Endpoint_Error> {
                    self.1.run(self.0).await
                }

                #(#field_methods)*
            }

            #client_impl
        }
    };

    if let Item::Struct(ItemStruct {
        fields: Fields::Named(ref mut named),
        ..
    }) = item
    {
        named
            .named
            .iter_mut()
            .for_each(|x| x.attrs.retain_mut(|x| !x.path().is_ident("with_builder")));
    }

    Ok(value)
}
