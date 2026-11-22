use std::sync::OnceLock;

pub const DIVISION_OVERFLOW: u32 = 0x1ffff;

fn unr_table() -> &'static [u8; 0x101] {
    static TABLE: OnceLock<[u8; 0x101]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u8; 0x101];
        for (index, entry) in table.iter_mut().enumerate() {
            let value = (0x40000 / (index as i32 + 0x100) + 1) / 2 - 0x101;
            *entry = value.max(0) as u8;
        }
        table
    })
}

pub fn unr_divide(h: u16, sz: u16) -> Option<u32> {
    if (h as u32) >= (sz as u32) * 2 {
        return None;
    }
    let shift = sz.leading_zeros();
    let numerator = (h as u64) << shift;
    let divisor = ((sz as u32) << shift) as u64;
    let reciprocal = unr_table()[((divisor - 0x7fc0) >> 7) as usize] as u64 + 0x101;
    let divisor = (0x2000080 - divisor * reciprocal) >> 8;
    let divisor = (0x80 + divisor * reciprocal) >> 8;
    Some((((numerator * divisor) + 0x8000) >> 16).min(DIVISION_OVERFLOW as u64) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_matches_psx_spx_endpoints() {
        let table = unr_table();
        assert_eq!(
            &table[..8],
            &[0xff, 0xfd, 0xfb, 0xf9, 0xf7, 0xf5, 0xf3, 0xf1]
        );
        assert_eq!(table[0x100], 0);
    }

    #[test]
    fn overflow_when_h_at_least_twice_sz() {
        assert_eq!(unr_divide(200, 100), None);
        assert_eq!(unr_divide(1, 0), None);
        assert_eq!(unr_divide(0, 0), None);
        assert!(unr_divide(199, 100).is_some());
    }

    #[test]
    fn known_quotients() {
        assert_eq!(unr_divide(0, 1000), Some(0));
        assert_eq!(unr_divide(256, 256), Some(0x10000));
        assert_eq!(unr_divide(256, 512), Some(0x8000));
        assert_eq!(unr_divide(256, 1000), Some(16777));
    }

    #[test]
    fn close_to_exact_quotient() {
        let mut worst = 0i64;
        for sz in (1..=0xffffu32).step_by(7) {
            for h in [1u32, 100, 256, 512, 1000, 4095, 0x7fff, 0xffff] {
                if h >= sz * 2 {
                    continue;
                }
                let exact = ((h as i64) << 16) / sz as i64;
                let result = unr_divide(h as u16, sz as u16).unwrap() as i64;
                worst = worst.max((result - exact.min(DIVISION_OVERFLOW as i64)).abs());
            }
        }
        assert!(worst <= 2, "worst error {worst}");
    }
}
