//! Versioned attack hit-buffer grammar shared with the card's host parser.
//! Rows are ordered writes; repeated addresses deliberately keep last-write wins.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitBuffer {
    pub addresses: Vec<u64>,
    pub rows: Vec<Vec<u32>>,
}

impl HitBuffer {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let mut lines = text.lines();
        let header = lines.next().ok_or("missing hit header")?
            .trim_matches(|c: char| c.is_ascii_whitespace());
        if header.is_empty() || !header.bytes().all(|c| c.is_ascii_digit()) {
            return Err("hit header must contain exactly one decimal count");
        }
        let count: usize = header.parse().map_err(|_| "invalid hit count")?;
        if !(1..64).contains(&count) {
            return Err("hit count is outside the shared 1..63 domain");
        }
        let hex = |token: &str| -> Result<u64, &'static str> {
            if token.is_empty() || !token.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("invalid hexadecimal hit word");
            }
            u64::from_str_radix(token, 16).map_err(|_| "hexadecimal hit word overflow")
        };
        let addresses = lines.next().ok_or("missing hit addresses")?
            .split_ascii_whitespace().map(hex).collect::<Result<Vec<_>, _>>()?;
        if addresses.is_empty() || addresses.iter().any(|a| a & 3 != 0) {
            return Err("hit addresses must be nonempty and 4-byte aligned");
        }
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            let row = lines.next().ok_or("missing hit row")?
                .split_ascii_whitespace().map(|token| {
                    u32::try_from(hex(token)?).map_err(|_| "hit value exceeds u32")
                }).collect::<Result<Vec<_>, _>>()?;
            if row.len() != addresses.len() {
                return Err("hit rows do not match the addresses");
            }
            rows.push(row);
        }
        if lines.any(|line| !line.bytes().all(|c| c.is_ascii_whitespace())) {
            return Err("unexpected trailing hit data");
        }
        Ok(Self { addresses, rows })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_legacy_cpu_card_parser_disagreements() {
        for input in ["1 2\n1000\n0\n", "1\n\n\n", "1\n1001\n0\n",
            "1\n1000\n0\n1\n", "+1\n1000\n0\n", "1\n0x1000\n0\n",
            "1\n1000\n100000000\n", "2\n1000\n0 1\n", "0\n1000\n"] {
            assert!(HitBuffer::parse(input).is_err(), "{input:?}");
        }
        assert!(HitBuffer::parse(&format!("64\n1000\n{}", "0\n".repeat(64))).is_err());
    }

    #[test]
    fn preserves_row_and_duplicate_write_order_and_crlf() {
        let hit = HitBuffer::parse("2\r\n1000 1000\r\n1 2\r\n3 4\r\n\r\n").unwrap();
        assert_eq!(hit.addresses, [0x1000, 0x1000]);
        assert_eq!(hit.rows, [vec![1, 2], vec![3, 4]]);
    }
}
