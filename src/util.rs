pub const fn ceiling_log_2(n: usize) -> u32 {
    usize::BITS - n.leading_zeros()
}

pub const fn greater_than_log_2(n: usize) -> u32 {
    ceiling_log_2(n)
        // if n can be written n = (x^2 - 1) for some integer x, it needs an overflow bit
        + if n.leading_zeros() + n.trailing_ones() == usize::BITS {
            1
        } else {
            0
        }
}
