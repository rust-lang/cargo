use std::fmt;
use std::sync::RwLock;
use std::collections::HashMap;

lazy_static! {
    static ref STRING_CASHE: RwLock<(Vec<String>, HashMap<String, usize>)> =
        RwLock::new((Vec::new(), HashMap::new()));
}

#[derive(Eq, PartialEq, Hash, Clone, Copy)]
pub struct InternedString {
    id: usize
}

impl InternedString {
    pub fn new(str: &str) -> InternedString {
        let (ref mut str_from_id, ref mut id_from_str) = *STRING_CASHE.write().unwrap();
        if let Some(&id) = id_from_str.get(str) {
            return InternedString { id };
        }
        str_from_id.push(str.to_string());
        id_from_str.insert(str.to_string(), str_from_id.len() - 1);
        return InternedString { id: str_from_id.len() - 1 }
    }
    pub fn to_inner(&self) -> String {
        STRING_CASHE.read().unwrap().0[self.id].to_string()
    }
}

impl fmt::Debug for InternedString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "InternedString {{ {} }}", STRING_CASHE.read().unwrap().0[self.id])
    }
}
