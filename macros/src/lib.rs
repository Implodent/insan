use proc_macro::TokenStream as TS;
use proc_macro2::{Ident, Span, TokenStream};
use quote::ToTokens;
use syn::{
    parenthesized, parse2, spanned::Spanned, token, Data, DeriveInput, Expr, Fields, Meta,
    MetaList, Result, Token, Type,
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

#[proc_macro_derive(ClientEndpoint, attributes(endpoint))]
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
            output: {
                input.parse::<Token![->]>()?;
                input.parse()?
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
                (quote::quote!(), quote::quote!()),
                meta.path.to_token_stream(),
            ),
            Fields::Named(named) => (
                (
                    match meta.mode.0 {
                        MetaMode::Json => {
                            quote::quote! { request.set_body(http_types::Body::from_json(self)) }
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
                            quote::quote! { request.set_body(http_types::Body::from_json(self)) }
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

            async fn run(&self, client: &Self::Context) -> Result<Self::Output, Self::Error> {
                let mut request = client.new_request(Method::#method, &{#url});
                #setup

                let mut response = client.run_request(request).await?;
                Ok(#desetup)
            }
        }
    })
}
