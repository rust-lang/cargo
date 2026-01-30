#[proc_macro]
pub fn identity_proc_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}

#[proc_macro_derive(Foo, attributes(foo))]
pub fn identity_derive_macro(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    input
}

#[proc_macro_attribute]
pub fn identity_attribute_macro(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let _ = args;
    input
}
