extern crate proc_macro;

use proc_macro::*;

#[proc_macro_attribute]
pub fn cargo_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let span = Span::call_site();
    let mut ret = TokenStream::new();
    ret.extend(Some(TokenTree::from(Punct::new('#', Spacing::Alone))));
    let test = TokenTree::from(Ident::new("test", span));
    ret.extend(Some(TokenTree::from(Group::new(
        Delimiter::Bracket,
        test.into(),
    ))));

    for token in item {
        let group = match token {
            TokenTree::Group(g) => {
                if g.delimiter() == Delimiter::Brace {
                    g
                } else {
                    ret.extend(Some(TokenTree::Group(g)));
                    continue;
                }
            }
            other => {
                ret.extend(Some(other));
                continue;
            }
        };

        let mut new_body = vec![
            TokenTree::from(Ident::new("let", span)),
            TokenTree::from(Ident::new("_test_guard", span)),
            TokenTree::from(Punct::new('=', Spacing::Alone)),
            TokenTree::from(Ident::new("crate", span)),
            TokenTree::from(Punct::new(':', Spacing::Joint)),
            TokenTree::from(Punct::new(':', Spacing::Alone)),
            TokenTree::from(Ident::new("support", span)),
            TokenTree::from(Punct::new(':', Spacing::Joint)),
            TokenTree::from(Punct::new(':', Spacing::Alone)),
            TokenTree::from(Ident::new("paths", span)),
            TokenTree::from(Punct::new(':', Spacing::Joint)),
            TokenTree::from(Punct::new(':', Spacing::Alone)),
            TokenTree::from(Ident::new("init_root", span)),
            TokenTree::from(Group::new(Delimiter::Parenthesis, TokenStream::new())),
            TokenTree::from(Punct::new(';', Spacing::Alone)),
        ]
        .into_iter()
        .collect::<TokenStream>();
        new_body.extend(group.stream());
        ret.extend(Some(TokenTree::from(Group::new(
            group.delimiter(),
            new_body,
        ))));
    }

    return ret;
}
