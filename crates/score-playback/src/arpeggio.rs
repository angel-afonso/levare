use score_core::Arpeggio;

/// One bounded attack sequence, shared by the full plan and entry audition.
/// Roll by 30 ms per interval, at most 120 ms overall and half the note's gate.
pub(super) fn attacks<const N: usize>(
    mut keys: [Option<u8>; N],
    mark: Option<Arpeggio>,
    gate: u64,
    rate: u32,
) -> ([Option<u8>; N], [u64; N]) {
    let mut delays = [0; N];
    if let Some(mark) = mark {
        keys.sort_unstable_by_key(|key| key.unwrap_or(128));
        // Enharmonic spellings of the same sounding key are one attack.
        let mut unique = [None; N];
        let mut count = 0;
        for key in keys.into_iter().flatten() {
            if count == 0 || unique[count - 1] != Some(key) {
                unique[count] = Some(key);
                count += 1;
            }
        }
        keys = unique;
        if mark == Arpeggio::Down {
            keys[..count].reverse();
        }
        if count > 1 {
            let intervals = (count - 1) as u64;
            let spread = (u64::from(rate) * 30 / 1000 * intervals)
                .min(u64::from(rate) * 120 / 1000)
                .min(gate / 2);
            for (index, delay) in delays[..count].iter_mut().enumerate() {
                *delay = spread * index as u64 / intervals;
            }
        }
    }
    (keys, delays)
}
