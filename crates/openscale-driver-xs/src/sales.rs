use openscale_domain::{
    DeptTotals, GrandTotals, GroupTotals, MachTotals, MoneyCents, OperatorTotals, PluKey, PluTotals,
};
use openscale_driver::DriverError;

fn read_i64_be(buf: &[u8], offset: usize) -> i64 {
    i64::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
        buf[offset + 4],
        buf[offset + 5],
        buf[offset + 6],
        buf[offset + 7],
    ])
}

fn read_u32_be(buf: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ])
}

fn decode_six_u16_datetime(buf: &[u8], offset: usize) -> Option<String> {
    if buf.len() < offset + 12 {
        return None;
    }
    let day = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
    let month = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]);
    let year = u16::from_be_bytes([buf[offset + 4], buf[offset + 5]]);
    let hour = u16::from_be_bytes([buf[offset + 6], buf[offset + 7]]);
    let minute = u16::from_be_bytes([buf[offset + 8], buf[offset + 9]]);
    let second = u16::from_be_bytes([buf[offset + 10], buf[offset + 11]]);
    if year == 0 {
        return None;
    }
    Some(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}"
    ))
}

/// Decodes `GRAND_TOTALS` (`CMD=10, CVN=11`, 169 bytes).
pub fn decode_grand_totals(period: u8, payload: &[u8]) -> Result<GrandTotals, DriverError> {
    if payload.len() < 0x55 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("GRAND_TOTALS payload too short: {} bytes", payload.len()),
        });
    }
    Ok(GrandTotals {
        period,
        counter_sales: MoneyCents(read_i64_be(payload, 0x09)),
        prepack_sales: MoneyCents(read_i64_be(payload, 0x11)),
        transactions: read_u32_be(payload, 0x19),
        gross_total: MoneyCents(read_i64_be(payload, 0x4D)),
        last_cleared_at: decode_six_u16_datetime(payload, 0x9D),
    })
}

/// Decodes `DEPT_TOTALS` (`CMD=41, CVN=11`, 87 bytes).
pub fn decode_dept_totals(period: u8, payload: &[u8]) -> Result<DeptTotals, DriverError> {
    if payload.len() < 0x1F {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("DEPT_TOTALS payload too short: {} bytes", payload.len()),
        });
    }
    let dt_offset = payload.len().saturating_sub(12);
    Ok(DeptTotals {
        period,
        did: u32::from(payload[0x01]),
        counter_sales: MoneyCents(read_i64_be(payload, 0x0B)),
        prepack_sales: MoneyCents(read_i64_be(payload, 0x13)),
        transactions: read_u32_be(payload, 0x1B),
        last_cleared_at: decode_six_u16_datetime(payload, dt_offset),
    })
}

/// Decodes `GROUP_TOTALS` (`CMD=74, CVN=11`, 83 bytes).
pub fn decode_group_totals(
    period: u8,
    group_id: u32,
    payload: &[u8],
) -> Result<GroupTotals, DriverError> {
    if payload.len() < 0x1F {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("GROUP_TOTALS payload too short: {} bytes", payload.len()),
        });
    }
    Ok(GroupTotals {
        period,
        group_id,
        counter_sales: MoneyCents(read_i64_be(payload, 0x0B)),
        prepack_sales: MoneyCents(read_i64_be(payload, 0x13)),
        transactions: read_u32_be(payload, 0x1B),
        last_cleared_at: decode_six_u16_datetime(payload, 0x48),
    })
}

/// Decodes `OPERATOR_TOTALS` (`CMD=8, CVN=11`, 311 bytes).
pub fn decode_operator_totals(
    period: u8,
    operator_id: u32,
    payload: &[u8],
) -> Result<OperatorTotals, DriverError> {
    if payload.len() < 0x23 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("OPERATOR_TOTALS payload too short: {} bytes", payload.len()),
        });
    }
    Ok(OperatorTotals {
        period,
        operator_id,
        counter_sales: MoneyCents(read_i64_be(payload, 0x0B)),
        prepack_sales: MoneyCents(read_i64_be(payload, 0x13)),
        transactions: read_u32_be(payload, 0x1F),
    })
}

/// Decodes `MACH_TOTALS` (`CMD=43, CVN=11`, 303 bytes).
pub fn decode_mach_totals(period: u8, payload: &[u8]) -> Result<MachTotals, DriverError> {
    if payload.len() < 0x55 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("MACH_TOTALS payload too short: {} bytes", payload.len()),
        });
    }
    Ok(MachTotals {
        period,
        counter_sales: MoneyCents(read_i64_be(payload, 0x09)),
        prepack_sales: MoneyCents(read_i64_be(payload, 0x11)),
        transactions: read_u32_be(payload, 0x19),
        gross_total: MoneyCents(read_i64_be(payload, 0x4D)),
    })
}

/// Decodes `PLU_TOTALS` (`CMD=9, CVN=7600`, 166 bytes).
pub fn decode_plu_totals(period: u8, payload: &[u8]) -> Result<PluTotals, DriverError> {
    if payload.len() < 82 {
        return Err(DriverError::Protocol {
            code: None,
            message: format!("PLU_TOTALS payload too short: {} bytes", payload.len()),
        });
    }
    let mut key_raw = [0u8; 8];
    key_raw.copy_from_slice(&payload[42..50]);
    let key = PluKey::from_xs_bytes(key_raw)?;

    Ok(PluTotals {
        period,
        key,
        transactions: read_u32_be(payload, 50),
        counter_sales: MoneyCents(read_i64_be(payload, 54)),
        counter_quantity: read_u32_be(payload, 62),
        counter_transactions: read_u32_be(payload, 66),
        prepack_sales: MoneyCents(read_i64_be(payload, 70)),
        prepack_quantity: read_u32_be(payload, 78),
        last_cleared_at: decode_six_u16_datetime(payload, 0x9A),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_grand_dept_and_plu_totals_byte_exact() {
        let mut grand = vec![0u8; 169];
        grand[0x09..0x11].copy_from_slice(&130_480i64.to_be_bytes());
        grand[0x19..0x1D].copy_from_slice(&34u32.to_be_bytes());
        grand[0x4D..0x55].copy_from_slice(&130_480i64.to_be_bytes());
        grand[0x9D..0x9F].copy_from_slice(&22u16.to_be_bytes());
        grand[0x9F..0xA1].copy_from_slice(&11u16.to_be_bytes());
        grand[0xA1..0xA3].copy_from_slice(&2026u16.to_be_bytes());
        let gt = decode_grand_totals(0, &grand).unwrap();
        assert_eq!(gt.counter_sales, MoneyCents(130_480));
        assert_eq!(gt.transactions, 34);
        assert_eq!(gt.last_cleared_at.as_deref(), Some("2026-11-22T00:00:00"));

        let mut dept = vec![0u8; 87];
        dept[0x01] = 1;
        dept[0x0B..0x13].copy_from_slice(&50_000i64.to_be_bytes());
        dept[0x1B..0x1F].copy_from_slice(&12u32.to_be_bytes());
        let dt = decode_dept_totals(0, &dept).unwrap();
        assert_eq!(dt.did, 1);
        assert_eq!(dt.counter_sales, MoneyCents(50_000));
        assert_eq!(dt.transactions, 12);

        let mut plu = vec![0u8; 166];
        let key = PluKey::new(1, 1).unwrap();
        plu[42..50].copy_from_slice(&key.to_xs_bytes().unwrap());
        plu[50..54].copy_from_slice(&34u32.to_be_bytes());
        plu[54..62].copy_from_slice(&130_480i64.to_be_bytes());
        plu[62..66].copy_from_slice(&18_640u32.to_be_bytes());
        let pt = decode_plu_totals(0, &plu).unwrap();
        assert_eq!(pt.key, key);
        assert_eq!(pt.transactions, 34);
        assert_eq!(pt.counter_sales, MoneyCents(130_480));
        assert_eq!(pt.counter_quantity, 18_640);
    }
}
