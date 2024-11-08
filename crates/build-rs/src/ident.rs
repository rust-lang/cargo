use unicode_ident::{is_xid_continue, is_xid_start};

pub(crate) fn is_feature_name(s: &str) -> bool {
    s.chars()
        .all(|ch| is_xid_continue(ch) || matches!(ch, '-' | '+' | '.'))
}

pub(crate) fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|ch| is_xid_start(ch) || matches!(ch, '_'))
        && cs.all(is_xid_continue)
}

pub(crate) fn is_ascii_ident(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|ch| ch.is_ascii_alphabetic() || matches!(ch, '_'))
        && cs.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_'))
}

pub(crate) fn is_crate_name(s: &str) -> bool {
    let mut cs = s.chars();
    cs.next()
        .is_some_and(|ch| is_xid_start(ch) || matches!(ch, '-' | '_'))
        && cs.all(|ch| is_xid_continue(ch) || matches!(ch, '-'))
}
