//! Native PC 2.0.2 agility scoring (81789C), before the entity minimum-20 override.

#[derive(Clone, Copy, Debug)]
pub struct AgilityThresholds {
    pub top: u32,
    pub middle: u32,
    pub light: u32,
}

impl AgilityThresholds {
    // Both style-specific parameter triples resolve to these quantized values
    // in the pinned PC 2.0.2 resources: 6652/A241/D08B and C523/A5FD/222A.
    pub const PC202: Self = Self {
        top: 200,
        middle: 700,
        light: 300,
    };

    pub fn score(self, weight: u32, capacity: u32, improvement: i32, unlocked: bool) -> u32 {
        let mut index: i32;
        if weight > capacity {
            index = 4;
        } else if let Some(ratio) = weight.wrapping_mul(1000).checked_div(capacity) {
            index = if unlocked && ratio <= self.top {
                0
            } else if ratio <= self.light {
                1
            } else {
                2 + i32::from(ratio > self.middle)
            };
            if improvement > 0 {
                index = i32::from(!unlocked).max(index.wrapping_sub(improvement));
            }
        } else {
            index = 0;
        }
        [100, 80, 50, 20, 0][index as usize]
    }
}
