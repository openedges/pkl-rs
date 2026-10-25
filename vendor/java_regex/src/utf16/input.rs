/// Raw UTF-16 text. Positions always count code units, including unpaired
/// surrogates. Callers choose unit stepping or code-point stepping explicitly.
#[derive(Clone, Copy)]
pub struct Input<'a>(pub &'a [u16]);
impl Input<'_> {
    pub fn at(self, pos: usize) -> u32 {
        let high = u32::from(self.0[pos]);
        if (0xd800..=0xdbff).contains(&high) {
            if let Some(&low) = self.0.get(pos + 1) {
                if (0xdc00..=0xdfff).contains(&low) {
                    return 0x10000 + ((high - 0xd800) << 10) + u32::from(low - 0xdc00);
                }
            }
        }
        high
    }
    pub fn next(self, pos: usize) -> usize {
        pos + if self.at(pos) >= 0x10000 { 2 } else { 1 }
    }
    pub fn previous(self, pos: usize) -> usize {
        let last = pos - 1;
        if last > 0
            && (0xdc00..=0xdfff).contains(&self.0[last])
            && (0xd800..=0xdbff).contains(&self.0[last - 1])
        {
            last - 1
        } else {
            last
        }
    }
    pub fn retreat(self, mut pos: usize, count: usize, lower: usize) -> Option<usize> {
        for _ in 0..count {
            if pos <= lower {
                return None;
            }
            pos = self.previous(pos);
            if pos < lower {
                return None;
            }
        }
        Some(pos)
    }
}
