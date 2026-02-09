//! macros for scalar compare order
//!
//! #[derive(ScalarPartialOrd)]
//! #[scalar_partial_ord(second, nanosecond)]
//! struct DateTime{
//!     // seconds since unix epoch
//!     pub seconds: i64,
//!     //! nanoseconds fraction, range from 0 to 999_999_999
//!     pub nanoseconds: u32,
//! }
//!
//! will generate code like
//!
//! impl ScalarPartialOrd for DateTime {
//!     fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//!         self.seconds.partial_cmp(&other.seconds)
//!             .then_with(|| self.nanoseconds.partial_cmp(&other.nanoseconds))
//!     }
//! }
//!
//! #[derive(ScalarPartialOrd)]
//! #[scalar_partial_ord(_0)]
//! struct Foo(i32);
//!
//! will generate code like
//!
//! impl ScalarPartialOrd for Foo {
//!     fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//!         self.0.partial_cmp(&other.0)
//!     }
//! }
//!
//!
//! #[derive(ScalarPartialOrd)]
//! #[scalar_partial_ord(id)]
//! struct RelValueRef<'a>{
//!    pub id: RelationshipId,
//!    pub rtype: Arc<str>,
//! }
//!
//! will generate
//!
//! impl ScalarPartialOrd for RelValueRef<'_> {
//!     fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//!         self.id.partial_cmp(&other.id)
//!     }
//! }

use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{DeriveInput, Ident, Token, parse_macro_input};

#[proc_macro_derive(ScalarPartialOrd, attributes(scalar_partial_ord))]
pub fn derive_scalar_partial_ord(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let fields_order = get_fields_order(&input.attrs);

    let comparisons = fields_order.iter().map(|field_name| {
        let field_ident = if field_name.starts_with('_') && field_name[1..].chars().all(char::is_numeric) {
            let index = field_name[1..].parse::<usize>().unwrap();
            let index = syn::Index::from(index);
            quote::ToTokens::into_token_stream(index)
        } else {
            let ident = syn::Ident::new(field_name, proc_macro2::Span::call_site());
            quote::ToTokens::into_token_stream(ident)
        };
        quote! {
            match self.#field_ident.partial_cmp(&other.#field_ident) {
                Some(std::cmp::Ordering::Equal) => {},
                ord => return ord,
            }
        }
    });

    let expanded = quote! {
        impl #impl_generics ScalarPartialOrd for #name #ty_generics #where_clause {
            fn scalar_partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                #(#comparisons)*
                Some(std::cmp::Ordering::Equal)
            }
        }
    };

    TokenStream::from(expanded)
}

fn get_fields_order(attrs: &[syn::Attribute]) -> Vec<String> {
    for attr in attrs {
        if attr.path().is_ident("scalar_partial_ord")
            && let Ok(nested) = attr.parse_args_with(Punctuated::<Ident, Token![,]>::parse_terminated)
        {
            return nested.into_iter().map(|ident| ident.to_string()).collect();
        }
    }
    vec![]
}
