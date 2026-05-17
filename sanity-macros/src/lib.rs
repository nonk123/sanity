use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
    Error, Expr, FnArg, ItemFn, Lit, Meta, Pat, ReturnType, parse_macro_input,
    spanned::Spanned as _,
};

#[proc_macro_attribute]
pub fn luafn(_: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let inputs = &input_fn.sig.inputs;
    let body = &input_fn.block;
    let visibility = &input_fn.vis;

    let fn_name_s = fn_name.to_string();

    let output = match &input_fn.sig.output {
        ReturnType::Type(_, output) => output.into_token_stream(),
        ReturnType::Default => quote! { () },
    };

    let doc_lines: Vec<String> = input_fn
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            let Meta::NameValue(meta) = &attr.meta else {
                return None;
            };

            let Expr::Lit(expr_lit) = &meta.value else {
                return None;
            };

            if let Lit::Str(lit_str) = &expr_lit.lit {
                return Some(lit_str.value().trim().to_string());
            }

            None
        })
        .collect();

    let params: Vec<_> = inputs
        .iter()
        .skip(1)
        .filter_map(|arg| {
            let FnArg::Typed(arg) = arg else {
                return None;
            };

            let Pat::Ident(ref pat) = *arg.pat else {
                return Error::new(arg.span(), "Only plain arguments are supported")
                    .to_compile_error()
                    .into();
            };

            let name = pat.ident.to_string();

            let ty = match arg.ty.to_token_stream().to_string().as_str() {
                "String" => "string",
                _ => "any",
            };

            Some(quote! {
                (#name.to_string(), #ty.to_string())
            })
        })
        .collect();

    let (lua_arg, lua_type) = if let Some(FnArg::Typed(arg)) = inputs.first() {
        (&arg.pat, &arg.ty)
    } else {
        return Error::new(
            inputs.span(),
            "The first argument needs to be of the `&mlua::Lua` type.",
        )
        .to_compile_error()
        .into();
    };

    let arg_pats: Vec<_> = inputs
        .iter()
        .skip(1)
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                Some(&pat_type.pat)
            } else {
                None
            }
        })
        .collect();

    let arg_types: Vec<_> = inputs
        .iter()
        .skip(1)
        .filter_map(|arg| {
            if let FnArg::Typed(pat_type) = arg {
                Some(&pat_type.ty)
            } else {
                None
            }
        })
        .collect();

    let expanded = quote! {
        #[allow(non_camel_case_types)]
        #visibility struct #fn_name;

        impl crate::lua::LuaFn for #fn_name {
            fn call(&self, #lua_arg: #lua_type, args: mlua::MultiValue) -> color_eyre::eyre::Result<mlua::Value> {
                let (#(#arg_pats),*): (#(#arg_types),*) =
                    <(#(#arg_types),*) as mlua::FromLuaMulti>::from_lua_multi(args, lua)?;

                let res: #output = (move || #body)();
                Ok(mlua::IntoLua::into_lua(res?, lua)?)
            }

            fn name(&self) -> String {
                #fn_name_s.to_string()
            }

            fn docs(&self) -> Vec<String> {
                vec![#(#doc_lines.to_string()),*]
            }

            fn params(&self) -> Vec<(String, String)> {
                vec![#(#params),*]
            }

            fn returns(&self) -> String {
                <#output as crate::lua::LuaFnReturn>::typename()
            }
        }
    };

    TokenStream::from(expanded)
}
