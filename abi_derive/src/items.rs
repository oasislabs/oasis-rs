use crate::{quote, syn, utils};

use proc_macro2::Span;
use quote::TokenStreamExt;

/// Represents an event of a smart contract.
pub struct Event {
    /// The name of the event.
    pub name: syn::Ident,
    /// The canonalized string representation used by the keccak hash
    /// in order to retrieve the first 4 bytes required upon calling.
    pub canonical: String,
    /// The signature of the event.
    pub method_sig: syn::MethodSig,
    /// Indexed parameters.
    ///
    /// # Note
    ///
    /// Only up to 4 different parameters can be indexed
    /// for the same event.
    pub indexed: Vec<(syn::Pat, syn::Type)>,
    /// Non-indexed parameters.
    pub data: Vec<(syn::Pat, syn::Type)>,
}

/// Represents a function declared in the contracts interface.
///
/// Since this is basically just the declaration of such as function
/// without implementation we refer to it as being a signature.
#[derive(Clone)]
pub struct Signature {
    /// The name of this signature.
    pub name: syn::Ident,
    /// The canonicalized string representation of this signature.
    pub canonical: String,
    /// The parameter information of this signature.
    pub method_sig: syn::MethodSig,
    /// The function selector hash (4 bytes) of this signature.
    pub hash: u32,
    /// The arguments of this signature.
    pub arguments: Vec<(syn::Pat, syn::Type)>,
    /// The return type of this signature.
    pub return_types: Vec<syn::Type>,
    /// If this signature is constant.
    ///
    /// # Note
    ///
    /// A constant signature cannot mutate chain state.
    pub is_constant: bool,
    /// If this signature is payable.
    ///
    /// # Note
    ///
    /// Only a payable signature can be invoked with value.
    pub is_payable: bool,
}

/// An item within a contract trait.
pub enum Item {
    /// An invokable function.
    Signature(Signature),
    /// An event.
    Event(Event),
    /// Some trait item that is unsupported and unhandled as of now.
    Other(syn::TraitItem),
}

/// The entire interface that is being defined by the attributed trait.
pub struct Interface {
    /// The name of the contract trait.
    name: String,
    /// The constructor signature.
    ///
    /// # Note
    ///
    /// This is simply the signature with the identifier being equal to `constructor`.
    constructor: Option<Signature>,
    /// The set of trait items.
    ///
    /// # Note
    ///
    /// These are either
    /// - `Signature`: A function declaration
    /// - `Event`: An event
    /// - `Other`: Some unsupported and unhandled trait item
    items: Vec<Item>,
}

impl Item {
    /// Returns the name of `self`.
    ///
    /// # Note
    ///
    /// Only returns a name if it is a supported kind of item.
    /// Only `Signature` and `Event` kinds are supported.
    fn name(&self) -> Option<&syn::Ident> {
        use Item::*;
        match *self {
            Signature(ref sig) => Some(&sig.name),
            Event(ref event) => Some(&event.name),
            Other(_) => None,
        }
    }
}

impl Interface {
    pub fn from_item(source: syn::Item) -> Self {
        let item_trait = match source {
            syn::Item::Trait(item_trait) => item_trait,
            _ => panic!("Dispatch trait can work with trait declarations only!"),
        };
        let trait_items = item_trait.items;

        let (constructor_items, other_items) = trait_items
            .into_iter()
            .map(Item::from_trait_item)
            .partition::<Vec<Item>, _>(|item| {
            item.name()
                .map_or(false, |ident| ident.to_string() == "constructor")
        });

        Interface {
            constructor: constructor_items.into_iter().next().map(|item| match item {
                Item::Signature(sig) => sig,
                _ => panic!("The constructor must be function!"),
            }),
            name: item_trait.ident.to_string(),
            items: other_items,
        }
    }

    pub fn items(&self) -> &[Item] {
        &self.items
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn constructor(&self) -> Option<&Signature> {
        self.constructor.as_ref()
    }
}

fn into_signature(
    ident: syn::Ident,
    method_sig: syn::MethodSig,
    is_constant: bool,
    is_payable: bool,
) -> Signature {
    let arguments: Vec<(syn::Pat, syn::Type)> = utils::iter_signature(&method_sig).collect();
    let return_types: Vec<syn::Type> = match method_sig.decl.output.clone() {
        syn::ReturnType::Default => Vec::new(),
        syn::ReturnType::Type(_, ty) => match *ty {
            syn::Type::Tuple(tuple_type) => tuple_type.elems.into_iter().collect(),
            ty => vec![ty],
        },
    };
    let canonical = utils::canonicalize_fn(&ident, &method_sig);
    let hash = utils::function_selector(&canonical);

    Signature {
        name: ident,
        arguments: arguments,
        method_sig: method_sig,
        canonical: canonical,
        hash: hash,
        return_types: return_types,
        is_constant: is_constant,
        is_payable: is_payable,
    }
}

fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        if let Some(first_seg) = attr.path.segments.first() {
            return first_seg.value().ident == name;
        };
        false
    })
}

impl Item {
    fn event_from_trait_item(method_sig: syn::MethodSig) -> Self {
        assert!(
            method_sig.ident != "constructor",
            "The constructor can't be an event"
        );
        let (indexed, non_indexed) = utils::iter_signature(&method_sig)
            .partition(|&(ref pat, _)| quote! { #pat }.to_string().starts_with("indexed_"));
        let canonical = utils::canonicalize_fn(&method_sig.ident, &method_sig);
        let event = Event {
            name: method_sig.ident.clone(),
            canonical: canonical,
            indexed: indexed,
            data: non_indexed,
            method_sig: method_sig,
        };
        Item::Event(event)
    }

    fn signature_from_trait_item(method_trait_item: syn::TraitItemMethod) -> Self {
        let constant = has_attribute(&method_trait_item.attrs, "constant");
        let payable = has_attribute(&method_trait_item.attrs, "payable");
        assert!(
            !(constant && payable),
            format!(
                "Method {} cannot be constant and payable at the same time",
                method_trait_item.sig.ident.to_string()
            )
        );
        assert!(
            !(method_trait_item.sig.ident.to_string() == "constructor" && constant),
            "Constructor can't be constant"
        );
        Item::Signature(into_signature(
            method_trait_item.sig.ident.clone(),
            method_trait_item.sig,
            constant,
            payable,
        ))
    }

    pub fn from_trait_item(source: syn::TraitItem) -> Self {
        match source {
            syn::TraitItem::Method(method_trait_item) => {
                if method_trait_item.default.is_some() {
                    return Item::Other(syn::TraitItem::Method(method_trait_item));
                }
                if has_attribute(&method_trait_item.attrs, "event") {
                    return Self::event_from_trait_item(method_trait_item.sig);
                }
                Self::signature_from_trait_item(method_trait_item)
            }
            trait_item => Item::Other(trait_item),
        }
    }
}

impl quote::ToTokens for Item {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match *self {
            Item::Event(ref event) => {
                let method_sig = &event.method_sig;
                let name = &event.name;
                tokens.append_all(&[utils::produce_signature(name, method_sig, {
                    let keccak = utils::keccak(&event.canonical.as_bytes());
                    let hash_bytes = keccak.iter().map(|b| {
                        syn::Lit::Int(syn::LitInt::new(
                            *b as u64,
                            syn::IntSuffix::U8,
                            Span::call_site(),
                        ))
                    });

                    let indexed_pats = event.indexed.iter().map(|&(ref pat, _)| pat);

                    let data_pats = event.data.iter().map(|&(ref pat, _)| pat);

                    let data_pats_count_lit = syn::Lit::Int(syn::LitInt::new(
                        event.data.len() as u64,
                        syn::IntSuffix::Usize,
                        Span::call_site(),
                    ));

                    quote! {
                        let topics = &[
                            [#(#hash_bytes),*].into(),
                            #(::oasis_std::abi::AsLog::as_log(&#indexed_pats)),*
                        ];

                        let mut sink = ::oasis_std::abi::Sink::new(#data_pats_count_lit);
                        #(sink.push(#data_pats));*;
                        let payload = sink.finalize_panicking();

                        oasis_std::ext::log(topics, &payload);
                    }
                })]);
            }
            Item::Signature(ref signature) => {
                tokens.append_all(
                    syn::TraitItem::Method(syn::TraitItemMethod {
                        attrs: Vec::new(),
                        sig: signature.method_sig.clone(),
                        default: None,
                        semi_token: None,
                    })
                    .into_token_stream(),
                );
            }
            Item::Other(ref item) => {
                tokens.append_all(&[item]);
            }
        }
    }
}

impl quote::ToTokens for Interface {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let trait_ident = syn::Ident::new(&self.name, Span::call_site());

        let items = &self.items;
        let constructor_item = self.constructor().map(|c| Item::Signature(c.clone()));
        tokens.append_all(quote! (
            pub trait #trait_ident {
                #constructor_item
                #(#items)*
            }
        ));
    }
}
