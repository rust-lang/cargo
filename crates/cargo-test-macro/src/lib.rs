extern crate proc_macro;

use quote::{quote, ToTokens};
use syn::{parse::Parser, *};

#[proc_macro_attribute]
pub fn cargo_test(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut fn_def = parse_macro_input!(item as ItemFn);

    let attr = quote! {
        #[test]
    };
    fn_def
        .attrs
        .extend(Attribute::parse_outer.parse2(attr).unwrap());

    let stmt = quote! {
        let _test_guard = crate::support::paths::init_root();
    };
    fn_def.block.stmts.insert(0, parse2(stmt).unwrap());

    fn_def.into_token_stream().into()
}
