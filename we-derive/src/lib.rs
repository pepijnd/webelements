use std::str::FromStr;

use html_parser::{Dom, Node};
use proc_macro2::{Ident, LineColumn, TokenStream, TokenTree};

use quote::{format_ident, quote, quote_spanned};
use syn::{parse::Parser, parse_macro_input, DeriveInput};

struct DomParsed {
    fields: Vec<syn::Field>,
    root_type: Option<syn::Path>,
    root_is_element: bool,
    build: TokenStream,
    errors: TokenStream,
}

static ELEM_INPUT: &[(&str, &str, &str)] = &[
    ("body", "Base", "HtmlElement"),
    ("div", "Div", "HtmlElement"),
    ("p", "Paragraph", "HtmlElement"),
    ("span", "Span", "HtmlSpanElement"),
    ("input", "Input", "HtmlInputElement"),
    ("button", "Button", "HtmlButtonElement"),
];

fn parse_args(args: TokenStream, s_fields: &syn::FieldsNamed) -> DomParsed {
    let args: Vec<TokenTree> = args.into_iter().collect();
    let dom = parse_dom(&args);
    match dom {
        Ok(dom) => gen_element(dom, s_fields),
        Err(e) => {
            let e = e.to_string();
            let dom_start = args.first().expect("dom has a start").span();
            let dom_end = args.last().expect("dom has an end").span();
            let dom_span = dom_start.join(dom_end).expect("creating dom span");
            DomParsed {
                fields: Default::default(),
                root_type: None,
                root_is_element: true,
                build: quote! {},
                errors: quote_spanned! {
                    dom_span => compile_error!(#e)
                },
            }
        }
    }
}

fn parse_dom(input: &[TokenTree]) -> html_parser::Result<Dom> {
    let mut html = String::new();
    let mut end: Option<LineColumn> = None;
    let mut offset: Option<usize> = None;
    for token in input {
        let span = token.span().start();
        if offset.is_none() {
            offset = Some(span.column);
        }
        if let Some(end) = end {
            if span.line > end.line {
                html.push('\n');
                html.push_str(
                    &" ".repeat(
                        span.column
                            .saturating_sub(offset.expect("html span cannot underflow")),
                    ),
                )
            } else {
                html.push_str(&" ".repeat(span.column - end.column))
            }
        } else {
            html.push_str(
                &" ".repeat(
                    span.column
                        .saturating_sub(offset.expect("html span cannot underflow")),
                ),
            )
        }
        end = Some(token.span().end());
        html.push_str(&token.to_string());
    }
    Dom::parse(&html)
}

fn walk_dom(dom: &[Node], refs: &mut Vec<(Ident, syn::Path)>) -> Vec<(bool, TokenStream)> {
    let mut elements = Vec::new();
    for node in dom {
        if let Node::Element(element) = node {
            // flag for if this element will be a member field in the struct
            let mut is_field = None;

            // flag for if this element is a custom webelement that needs to be build
            let mut is_custom = None;

            // flag for if this element will be repeated
            let mut is_repeat = None;

            // list of attributes that the element will have. all crate options will be filtered out
            let mut attributes = Vec::new();

            for (key, value) in element.attributes.iter() {
                if key == "we_field" {
                    is_field = value.clone()
                } else if key == "we_element" {
                    // the custom path will be generated from the elements name
                    let custom = syn::parse2::<syn::Path>(
                        TokenStream::from_str(&element.name).expect("custom path name tokenstream"),
                    )
                    .expect("custom path tokenstream");
                    is_custom = Some(custom);

                    // the custom element cant have any children because they can't be appended to it.
                    if !element.children.is_empty() {
                        return vec![(
                            false,
                            quote! {
                                compile_error!("`we_element` element cant have any children")
                            },
                        )];
                    }
                } else if key == "we_repeat" {
                    if let Some(n) = value {
                        if let Ok(n) = n.parse::<i64>() {
                            is_repeat = Some(n);
                        } else {
                            return vec![(
                                false,
                                quote! {
                                    compile_error!("`we_repeat` mut have a positive interger value")
                                },
                            )];
                        }
                    } else {
                        return vec![(
                            false,
                            quote! {
                                compile_error!("`we_repeat` needs a value")
                            },
                        )];
                    }
                } else {
                    attributes.push((key, value));
                }
            }
            let name = &element.name;
            // find the identifier for the element type in the static list
            let field = ELEM_INPUT.iter().find_map(|s| {
                if name.to_lowercase() == s.0 {
                    Some(format_ident!("{}", s.1))
                } else {
                    None
                }
            });

            // no support for default element types yet.
            if field.is_none() && is_custom.is_none() {
                let error = format!("element `{}` not implemented", name.to_lowercase());
                return vec![(false, quote! { compile_error!(#error) })];
            }

            // if the element is not custom set the path to it to the parent crate
            let elem_type = is_custom.clone().unwrap_or_else(|| {
                let field = syn::parse2::<syn::Path>(quote! { webelements::elem::#field })
                    .expect("custom element field name");
                syn::parse2::<syn::Path>(quote! { webelements::Element<#field> })
                    .expect("custom element field path")
            });

            // if the element is to be repeated set the field type to `Vec<Field_Type>`
            let field_type = if is_repeat.is_some() {
                syn::parse2::<syn::Path>(quote! { Vec<#elem_type> }).expect("field type name")
            } else {
                elem_type.clone()
            };

            if let Some(field) = is_field.as_ref() {
                let field = format_ident!("{}", field);
                refs.push((field, field_type.clone()));
            }

            // recursivly generate code for all the children of this element;
            let children = walk_dom(&element.children, refs);

            let ident = format_ident!("_e_{}", element.name);
            let text = element.children.iter().find_map(|n| {
                if let Node::Text(s) = n {
                    Some(s.clone())
                } else {
                    None
                }
            });
            // some variables will be iterators over Options types because they are optional and when iterated will not generate any code
            let text = text.iter();

            let classes = element.classes.iter();
            let attributes = attributes.iter().map(|&(k, v)| {
                let v = v.clone().unwrap_or_else(|| "".to_owned());
                quote! { (#k, #v) }
            });
            let mut field_ident = is_field.iter().map(|s| format_ident!("_m_{}", s));
            let repeat_field = field_ident.clone();
            let element_builder = match is_custom.as_ref() {
                Some(custom) => {
                    quote! { <#custom as webelements::WebElementBuilder>::build() }
                }
                None => quote! { <#elem_type>::new() },
            };
            if is_field.is_some() && is_repeat.is_some() {
                field_ident.next();
            }
            let single = children
                .iter()
                .filter_map(|(r, c)| if !*r { Some(c) } else { None });
            let lists = children
                .iter()
                .filter_map(|(r, c)| if *r { Some(c) } else { None });

            let mut tokens = quote! {
                let mut #ident = #element_builder?;
                #( #ident.append(&{#single})?; )*
                #( #ident.append_list({#lists})?; )*
                #( #ident.add_class(#classes); )*
                #(
                    let (key, value) = #attributes;
                    #ident.set_attr(key, value)?;
                )*
                #( #ident.set_text(#text); )*
                #( #field_ident = Some(#ident.clone()); )*
                #ident
            };
            if let Some(n) = is_repeat {
                let n = n as usize;
                let iter = (0..n).map(|n| n.to_string());
                tokens = quote! {
                    let mut _elem_list = Vec::with_capacity(#n);
                    #(_elem_list.push({
                        let i = #iter;
                        #tokens
                    });)*
                    #( #repeat_field = Some(_elem_list.clone()); )*
                    _elem_list
                };
            }
            elements.push((is_repeat.is_some(), tokens));
        }
    }
    elements
}

fn gen_element(dom: Dom, s_fields: &syn::FieldsNamed) -> DomParsed {
    let mut refs: Vec<(Ident, syn::Path)> = Vec::new();
    let mut errors = quote! {};
    if dom.children.len() != 1 {
        errors = quote! {
            #errors
            compile_error!("DOM should contain 1 root")
        };
    }
    let mut root_is_element = true;
    let root_type = dom
        .children
        .first()
        .map(|e| {
            if let Node::Element(e) = e {
                let name = ELEM_INPUT.iter().find_map(|s| {
                    if e.name.to_lowercase() == s.0 {
                        Some(format_ident!("{}", s.1))
                    } else {
                        None
                    }
                });
                if let Some(name) = name {
                    syn::parse2::<syn::Path>(quote! { webelements::elem::#name }).ok()
                } else {
                    root_is_element = false;
                    let name = format_ident!("{}", e.name);
                    syn::parse2::<syn::Path>(quote! { #name }).ok()
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            errors = quote! { #errors; compile_error!("no root found") };
            None
        });
    let elements = walk_dom(&dom.children, &mut refs);
    let root = &elements.first().expect("element needs to have a root").1;
    let ref_name: Vec<Ident> = refs.iter().map(|(s, _)| format_ident!("{}", s)).collect();
    let ref_value: Vec<Ident> = refs
        .iter()
        .map(|(s, _)| format_ident!("_m_{}", s))
        .collect();
    let fields = s_fields.named.iter().map(|f| f.ident.as_ref()).flatten();
    let types = s_fields.named.iter().map(|f| &f.ty);
    let token = quote!(
        fn build() -> webelements::Result<Self> {
            #( let mut #ref_value = None; )*
            let _e_root = {#root};
            let mut element = Self {
                root: _e_root,
                #( #fields: <#types as Default>::default(),)*
                #( #ref_name: #ref_value.unwrap(),)*
            };
            <Self as webelements::WebElement>::init(&mut element)?;
            Ok(element)
        }
    );
    DomParsed {
        fields: refs
            .iter()
            .map(|(ident, ty)| {
                syn::Field::parse_named
                    .parse2(quote! { pub #ident: #ty })
                    .expect("fields name")
            })
            .collect(),
        root_type,
        root_is_element,
        build: token,
        errors,
    }
}

#[proc_macro_attribute]
pub fn we_builder(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let tokens = {
        let mut ast = parse_macro_input!(input as DeriveInput);
        let ident = ast.ident.clone();
        if let syn::Data::Struct(ref mut struct_data) = &mut ast.data {
            if let syn::Fields::Named(s_fields) = &mut struct_data.fields {
                let DomParsed {
                    fields,
                    root_type,
                    root_is_element,
                    build,
                    errors,
                } = parse_args(args.into(), s_fields);
                let elem = if root_is_element {
                    quote! { #root_type }
                } else {
                    quote! { <#root_type as WebElementBuilder>::Elem }
                };
                let root = if root_is_element {
                    quote! { webelements::Element<#root_type> }
                } else {
                    quote! { #root_type }
                };
                s_fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! { pub root: #root })
                        .expect("root field token failed"),
                );
                for field in fields.iter() {
                    s_fields.named.push(field.clone())
                }

                return quote! {
                    #errors
                    #ast

                    impl webelements::WebElementBuilder for #ident {
                        type Elem = #elem;

                        #build
                    }

                    impl AsRef<webelements::Element<<Self as webelements::WebElementBuilder>::Elem>> for #ident {
                        fn as_ref(&self) -> &webelements::Element<<Self as webelements::WebElementBuilder>::Elem> {
                            self.root.as_ref()
                        }
                    }

                    impl std::ops::Deref for #ident {
                        type Target=webelements::Element<<Self as webelements::WebElementBuilder>::Elem>;
                        fn deref(&self) -> &Self::Target {
                            self.root.as_ref()
                        }
                    }
                }
                .into();
            }
        }
        (quote! {
            compile_error!("`we_element` is only valid on structs")
        })
        .into()
    };
    println!("{}", tokens);
    tokens
}

#[proc_macro_derive(WebElement)]
pub fn we_element_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ident = ast.ident;

    (quote! {
        impl webelements::WebElement for #ident {
            fn init(&mut self) -> webelements::Result<()> { Ok(()) }
        }
    })
    .into()
}

#[proc_macro]
pub fn element_types(_input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let elems = ELEM_INPUT.iter().map(|s| s.0);
    let names = ELEM_INPUT.iter().map(|s| format_ident!("{}", s.1));
    let types = ELEM_INPUT.iter().map(|s| format_ident!("{}", s.2));
    let tokens = quote! {
        #(
        #[derive(Debug, Clone)]
        pub struct #names;
        impl ElemTy for #names {
            type Elem = web_sys::#types;

            fn make() -> crate::Result<Self::Elem> {
                crate::document()?
                    .create_element(#elems)?
                    .dyn_into::<web_sys::#types>()
                    .map_err(|e| crate::Error::Cast(std::any::type_name::<web_sys::#types>()))
            }
        }
        )*
    };
    tokens.into()
}

#[cfg(test)]
mod tests {}
