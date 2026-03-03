//! Expression macros
//!
//! input:
//!
//! #[cypher_func(sig="(any) -> any")]
//! fn date_any(arg: &DatumRef) -> DatumValue {
//!      // impl
//! }
//!
//! output:
//! fn date_any_batch(args: &[ArrayRef], vis: &BitVec, len: usize) -> AnyArray {
//!   // prepare inputs
//!    
//!   // prepare output builder
//!   
//! }
//!
//! what we know: signature input is any, output is any
//! we should have an table describe the type -> Array -> ArrayBuilder mapping
//!
//! | logical type | Array              | ArrayBuilder             |
//! | --- | --- | --- |
//! | any         | VariantArray         | VariantArrayBuilder      |
//! | bool        | BoolArray            | BoolArrayBuilder         |
//! | noderef     | VirtualNodeArray     | VirtualNodeArrayBuilder  |
//! | relref      | VirtualRelArray      | VirtualRelArrayBuilder   |
//! | pathref     | VirtualPathArray     | VirtualPathArrayBuilder  |
//! | node        | NodeArray            | NodeArrayBuilder         |
//! | rel         | RelArray             | RelArrayBuilder          |
//! | path        | PathArray            | PathArrayBuilder         |

use itertools::Itertools;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use regex::Regex;
use syn::parse::Parse;
use syn::{GenericArgument, Ident, ItemFn, LitStr, PathArguments, ReturnType, Type, parse_macro_input};

#[proc_macro_attribute]
pub fn cypher_func(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let fn_name = &input_fn.sig.ident;
    // let fn_body = &input_fn.block;

    let func_attr = parse_macro_input!(attr as CypherFuncAttr);

    // Analyze return type to determine fallible and nullable
    let (fallible, nullable) = analyze_return_type(&input_fn.sig.output);

    let batch_fn_name = format_ident!("{}", func_attr.batch_name);

    // cast input array
    // generate code like
    // let arg_0 = args[0].as_any_array();
    // let arg_1 = args[1].as_any_array();
    let mut array_cols = vec![];
    for (idx, arg_type) in func_attr.sig.inputs.iter().enumerate() {
        // convert arrayref to xxxArray by given arg type
        let arg_col = format_ident!("arg_{}", idx);
        let array_cast = gen_array_cast(arg_type, idx, &arg_col);
        array_cols.push(array_cast);
    }

    // generate code like:
    // arg_0
    // arg_1
    let arg_array_i = func_attr
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(idx, _)| format_ident!("arg_{}", idx))
        .collect_vec();

    // prepare output builder
    let output_builder = format_ident!("output_builder");
    let def_output_builder = gen_array_builder(&func_attr.sig.output, &output_builder);

    let func_args = quote! {
        #(#arg_array_i.get(i).unwrap()),*
    };

    let _debug_func_args = quote! {
        #(
            println!("{:?}", #arg_array_i.get(i));
        )*
    };

    let valid_rows = quote! {
        let valid_rows = vis.clone();
        #(
            let valid_rows = valid_rows & #arg_array_i.valid_map().clone();
        )*
    };

    // Generate different code based on fallible and nullable
    let loop_body = gen_loop_body(fn_name, &func_args, fallible, nullable);

    let expanded = quote! {
        #input_fn
        pub fn #batch_fn_name(args: &[ArrayRef], vis: &BitVec, len: usize) -> Result<ArrayImpl, EvalError> {

            #(#array_cols)*

            #def_output_builder

            #valid_rows
            // println!("valid_rows: {:?}", valid_rows);

            for i in 0..len {
                if valid_rows[i] {
                    // #debug_func_args
                    #loop_body
                } else {
                    // ternary logic, if either one of the inputs is null, return null
                    output_builder.push(None);
                }
            }

            Ok(#output_builder.finish().into())
        }
    };

    TokenStream::from(expanded)
}

/// Analyze the return type to determine if it's fallible (Result) and/or nullable (Option)
fn analyze_return_type(ret: &ReturnType) -> (bool, bool) {
    match ret {
        ReturnType::Default => (false, false),
        ReturnType::Type(_, ty) => analyze_type(ty),
    }
}

/// Recursively analyze a type to check for Result and Option wrappers
fn analyze_type(ty: &Type) -> (bool, bool) {
    if let Type::Path(type_path) = ty {
        if let Some(segment) = type_path.path.segments.last() {
            let ident = segment.ident.to_string();

            match ident.as_str() {
                "Result" => {
                    // Check if the first generic argument is Option
                    #[allow(clippy::collapsible_if)]
                    if let PathArguments::AngleBracketed(args) = &segment.arguments {
                        if let Some(GenericArgument::Type(inner_ty)) = args.args.first() {
                            let (_, inner_nullable) = analyze_type(inner_ty);
                            return (true, inner_nullable);
                        }
                    }
                    (true, false)
                }
                "Option" => (false, true),
                _ => (false, false),
            }
        } else {
            (false, false)
        }
    } else {
        (false, false)
    }
}

/// Generate the loop body based on fallible and nullable flags
fn gen_loop_body(
    fn_name: &syn::Ident,
    func_args: &proc_macro2::TokenStream,
    fallible: bool,
    nullable: bool,
) -> proc_macro2::TokenStream {
    match (fallible, nullable) {
        // T: direct value
        (false, false) => quote! {
            let ret = #fn_name(#func_args);
            output_builder.push(Some(ret.as_scalar_ref()));
        },
        // Result<T, E>: may fail, but always has value when Ok
        (true, false) => quote! {
            let ret = #fn_name(#func_args)?;
            output_builder.push(Some(ret.as_scalar_ref()));
        },
        // Option<T>: may be None
        (false, true) => quote! {
            let ret = #fn_name(#func_args);
            output_builder.push(ret.as_ref().map(|v| v.as_scalar_ref()));
        },
        // Result<Option<T>, E>: may fail and may be None
        (true, true) => quote! {
            let ret = #fn_name(#func_args)?;
            output_builder.push(ret.as_ref().map(|v| v.as_scalar_ref()));
        },
    }
}

struct CypherFuncAttr {
    sig: CypherFuncSig,
    batch_name: String,
}

struct CypherFuncSig {
    inputs: Vec<String>,
    output: String,
}

impl Parse for CypherFuncAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut sig = None;
        let mut batch_name = None;

        //#[cypher_func(key=value, key=value,...)]
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<syn::Token![=]>()?;
            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "sig" => {
                    // parse sig value
                    sig = Some(extract_sig(value.value().as_str()));
                }
                "batch_name" => {
                    // batch function name
                    batch_name = Some(value.value().as_str().to_lowercase().to_string());
                }
                _ => {
                    return Err(input.error(format!("unknown argument: {}", key)));
                }
            }
            if input.peek(syn::Token![,]) {
                input.parse::<syn::Token![,]>()?;
            }
        }

        Ok(CypherFuncAttr {
            sig: sig.ok_or_else(|| input.error("missing sig"))?,
            batch_name: batch_name.ok_or_else(|| input.error("missing batch_name"))?,
        })
    }
}

// extract (input1, input2) -> output
fn extract_sig(sig: &str) -> CypherFuncSig {
    let re = Regex::new(r"^\s*\((.*?)\)\s*->\s*(.*)\s*$").unwrap();

    // TODO(pgao): handle error
    let caps = re.captures(sig).unwrap();
    let inputs_str = caps.get(1).unwrap().as_str().trim();
    let output_str = caps.get(2).unwrap().as_str().trim();

    let inputs = if inputs_str.is_empty() {
        Vec::new()
    } else {
        inputs_str.split(',').map(|s| s.trim().to_string()).collect()
    };

    CypherFuncSig {
        inputs,
        output: output_str.to_string(),
    }
}

fn gen_array_cast(arg_type: &str, idx: usize, output: &syn::Ident) -> proc_macro2::TokenStream {
    match arg_type {
        "any" => quote! {
            let #output = args[#idx].as_variant().expect(&format!("expected variant array, got {:?}", args[#idx].logical_type()));
        },
        "bool" => quote! {
            let #output = args[#idx].as_bool().expect(&format!("expected bool array, got {:?}", args[#idx].logical_type()));
        },
        "noderef" => quote! {
            let #output= args[#idx].as_virtual_node().expect(&format!("expected noderef array, got {:?}", args[#idx].logical_type()));
        },
        "relref" => quote! {
            let #output = args[#idx].as_virtual_rel().expect(&format!("expected relref array, got {:?}", args[#idx].logical_type()));
        },
        "pathref" => quote! {
            let #output = args[#idx].as_virtual_path().expect(&format!("expected pathref array, got {:?}", args[#idx].logical_type()));
        },
        "node" => quote! {
            let #output = args[#idx].as_node().expect(&format!("expected node array, got {:?}", args[#idx].logical_type()));
        },
        "rel" => quote! {
            let #output = args[#idx].as_rel().expect(&format!("expected rel array, got {:?}", args[#idx].logical_type()));
        },
        "path" => quote! {
            let #output = args[#idx].as_path().expect(&format!("expected path array, got {:?}", args[#idx].logical_type()));
        },
        _ => quote! {compile_error!("invalid signature type")},
    }
    // TokenStream::from(expanded)
}

fn gen_array_builder(ret_type: &str, output: &syn::Ident) -> proc_macro2::TokenStream {
    match ret_type {
        "any" => quote! {
            let mut #output = VariantArrayBuilder::with_capacity(len, LogicalType::ANY);
        },
        "bool" => quote! {
            let mut #output = BoolArrayBuilder::with_capacity(len);
        },
        "noderef" => quote! {
            let mut #output = VirtualNodeArrayBuilder::with_capacity(len);
        },
        "relref" => quote! {
            let mut #output = VirtualRelArrayBuilder::with_capacity(len);
        },
        "pathref" => quote! {
            let mut #output = VirtualPathArrayBuilder::with_capacity(len);
        },
        "node" => quote! {
            let mut #output = NodeArrayBuilder::with_capacity(len);
        },
        "rel" => quote! {
            let mut #output = RelArrayBuilder::with_capacity(len);
        },
        "path" => quote! {
            let mut #output = PathArrayBuilder::with_capacity(len);
        },
        _ => quote! {compile_error!("invalid signature type")},
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn test_parse() {
        let sig = "(any, bool) -> any";
        let sig = extract_sig(sig);
        assert_eq!(sig.inputs, vec!["any", "bool"]);
        assert_eq!(sig.output, "any");
    }
}
