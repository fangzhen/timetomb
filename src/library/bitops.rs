use core::ops::*;

/// Find first set bit of @value.
/// if value is 0, return size of value.
pub fn ffs(value: usize) -> usize {
    let s = size_of::<usize>() * 8;
    for i in 0..s {
        if value & (1 << i) != 0 {
            return i;
        }
    }
    return s;
}

pub fn align_ceil<T>(value: T, a: T) -> T
where
    T: Add<T, Output = T>
        + Sub<T, Output = T>
        + Not<Output = T>
        + BitAnd<Output = T>
        + From<u8>
        + Copy,
{
    let one = T::from(1);
    (value + a - one) & (!(a - one))
}

pub fn align_floor<T>(value: T, a: T) -> T
where
    T: Sub<T, Output = T> + Not<Output = T> + BitAnd<Output = T> + From<u8> + Copy,
{
    let one = T::from(1);
    (value) & (!(a - one))
}

pub fn power_of_two_ceil(mut v: usize) -> usize {
    let s = size_of::<usize>() * 8;
    let mut p = 1;
    v -= 1;
    while p < s {
        v |= v >> p;
        p = p << 1;
    }
    v += 1;
    return v;
}
