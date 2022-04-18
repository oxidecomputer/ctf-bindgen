use crate::ctf;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

/// Emit the types and functions contained in the provided CTF info as Rust
/// source code.
pub fn emit(ctf_info: &ctf::Ctf) -> String {
    let mut tokens = emit_functions(ctf_info);
    tokens.extend(emit_types(ctf_info));
    let f: syn::File = syn::parse2(tokens).unwrap();
    prettyplease::unparse(&f)
}

/// Emit the types contained in the provided CTF as a Rust token stream.
pub fn emit_types(ctf_info: &ctf::Ctf) -> TokenStream {
    let mut tokens = TokenStream::new();

    for t in &ctf_info.sections.types {
        let ts = match &t.repr {
            ctf::TypeRepr::Struct(ref members) => {
                emit_struct(ctf_info, t, members)
            }
            ctf::TypeRepr::Int(_) => continue,
            ctf::TypeRepr::Float(_) => continue,
        };
        tokens.extend(ts);
    }

    tokens
}

/// Emit provided type as a Rust token stream.
pub fn emit_struct(
    ctf_info: &ctf::Ctf,
    t: &ctf::Type,
    members: &[ctf::StructMember],
) -> TokenStream {
    let name = format_ident!("{}", ctf_info.string_at(t.name_offset));
    let mut rs_members = Vec::new();

    for m in members {
        let i = m.type_offset as usize - 1;
        let ty = rust_type(ctf_info, &ctf_info.sections.types[i]);
        let name = format_ident!("{}", ctf_info.string_at(m.name_offset));
        rs_members.push(quote! { pub #name: #ty });
    }

    quote! {
        #[repr(C)]
        pub struct #name {
            #(#rs_members),*
        }
    }
}

/// Emit the functions contained in the provided CTF as a Rust token stream. All
/// functions are wrapped in an extern "C" block with a link attribute that
/// specifies the source library.
pub fn emit_functions(ctf_info: &ctf::Ctf) -> TokenStream {
    let mut tokens = TokenStream::new();

    for (i, f) in ctf_info.sections.functions.iter().enumerate() {
        if f.types.is_empty() {
            continue;
        }

        let mut types = Vec::new();
        for type_index in &f.types {
            let t = &ctf_info.sections.types[*type_index as usize - 1];
            types.push(rust_type(ctf_info, t));
        }

        let name = format_ident!("{}", &ctf_info.function_names[i]);
        let ret = &types[0];
        let args = &types[1..];

        tokens.extend(quote! {
            pub fn #name( #(_: #args),* ) -> #ret;
        });
    }

    let libname = &ctf_info.libname;
    quote! {
        #[link(name=#libname, kind="dylib")]
        extern "C" {
            #tokens
        }
    }
}

/// Map a CTF type into a Rust type.
fn rust_type(ctf_info: &ctf::Ctf, t: &ctf::Type) -> TokenStream {
    match t.type_encoding.kind() {
        //TODO
        //  - signed/unsigned
        ctf::Kind::Integer => match &t.info {
            ctf::TypeInfo::Size(1) => {
                quote! { u8 }
            }
            ctf::TypeInfo::Size(2) => {
                quote! { u16 }
            }
            ctf::TypeInfo::Size(4) => {
                quote! { u32 }
            }
            ctf::TypeInfo::Size(8) => {
                quote! { u64 }
            }
            i => todo!("unhandled info: {:?}", i),
        },
        ctf::Kind::Struct => {
            let struct_name =
                format_ident!("{}", ctf_info.string_at(t.name_offset));
            quote! { #struct_name }
        }
        ctf::Kind::Float => match &t.info {
            ctf::TypeInfo::Size(4) => {
                quote! { f32 }
            }
            ctf::TypeInfo::Size(8) => {
                quote! { f64 }
            }
            i => todo!("unhandled info: {:?}", i),
        },
        x => todo!("{:?}", x),
    }
}
