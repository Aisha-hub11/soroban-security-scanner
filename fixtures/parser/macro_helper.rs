// Parser fixture: a helper macro is defined and used without being expanded.
macro_rules! checked_add {
    ($a:expr, $b:expr) => {
        $a.checked_add($b)
    };
}

pub fn total(a: u32, b: u32) -> Option<u32> {
    checked_add!(a, b)
}
