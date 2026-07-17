/// Avoid hashing `T` when included in another type
#[derive(Copy, Clone, Default, Debug)]
pub struct Unhashed<T>(pub T);

impl<T> std::hash::Hash for Unhashed<T> {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {
        // ...
    }
}

impl<T> PartialEq for Unhashed<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<T> Eq for Unhashed<T> {}

impl<T> PartialOrd for Unhashed<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(other.cmp(&self))
    }
}

impl<T> Ord for Unhashed<T> {
    fn cmp(&self, _: &Self) -> std::cmp::Ordering {
        std::cmp::Ordering::Equal
    }
}
