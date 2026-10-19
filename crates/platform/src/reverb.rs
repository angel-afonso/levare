//! A shared stereo room: four damped feedback delays and two diffusers per side.
//! Delay storage is allocated at preparation; processing/reset never allocates.
struct Delay {
    data: Vec<f32>,
    cursor: usize,
    filtered: f32,
}
impl Delay {
    fn new(samples: usize) -> Self {
        Self {
            data: vec![0.; samples.max(1)],
            cursor: 0,
            filtered: 0.,
        }
    }
    fn clear(&mut self) {
        self.data.fill(0.);
        self.cursor = 0;
        self.filtered = 0.;
    }
    fn advance(&mut self) {
        self.cursor += 1;
        if self.cursor == self.data.len() {
            self.cursor = 0;
        }
    }
    fn comb(&mut self, input: f32, feedback: f32) -> f32 {
        let out = self.data[self.cursor];
        self.filtered = out * 0.75 + self.filtered * 0.25;
        // Avoid denormal CPU spikes when a tail becomes inaudible.
        if self.filtered.abs() < 1e-20 {
            self.filtered = 0.;
        }
        self.data[self.cursor] = input + self.filtered * feedback;
        self.advance();
        out
    }
    fn diffuse(&mut self, input: f32) -> f32 {
        let delayed = self.data[self.cursor];
        let out = delayed - input * 0.5;
        let next = input + out * 0.5;
        self.data[self.cursor] = if next.abs() < 1e-20 { 0. } else { next };
        self.advance();
        out
    }
}
struct Side {
    combs: [Delay; 4],
    diffusers: [Delay; 2],
}
impl Side {
    fn new(rate: u32, offset: usize) -> Self {
        let scaled =
            |length| ((length + offset) as f64 * f64::from(rate) / 48_000.).round() as usize;
        Self {
            combs: [1499, 1601, 1747, 1867].map(|n| Delay::new(scaled(n))),
            diffusers: [223, 547].map(|n| Delay::new(scaled(n))),
        }
    }
    fn clear(&mut self) {
        for delay in self.combs.iter_mut().chain(self.diffusers.iter_mut()) {
            delay.clear();
        }
    }
    fn sample(&mut self, input: f32, feedback: f32) -> f32 {
        let mut out = self
            .combs
            .iter_mut()
            .map(|d| d.comb(input, feedback))
            .sum::<f32>()
            * 0.25;
        for diffuser in &mut self.diffusers {
            out = diffuser.diffuse(out);
        }
        out
    }
}
pub(crate) struct Reverb {
    left: Side,
    right: Side,
    wet: f32,
    smoothing: f32,
    bypassed: bool,
}
impl Reverb {
    pub(crate) fn new(rate: u32) -> Self {
        Self {
            left: Side::new(rate, 0),
            right: Side::new(rate, 79),
            wet: 0.14,
            smoothing: 1. - (-1. / (rate as f32 * 0.02)).exp(),
            bypassed: false,
        }
    }
    pub(crate) fn clear(&mut self) {
        self.left.clear();
        self.right.clear();
    }
    pub(crate) fn process(&mut self, left: &mut [f32], right: &mut [f32], settings: u32) {
        let amount = (settings & 255) as f32 / 100.;
        if amount == 0. {
            if !self.bypassed {
                self.clear();
            }
            self.bypassed = true;
            self.wet = 0.;
            return;
        }
        self.bypassed = false;
        let feedback = 0.72 + ((settings >> 8) & 255) as f32 / 100. * 0.20;
        for (l, r) in left.iter_mut().zip(right) {
            self.wet += (amount - self.wet) * self.smoothing;
            let input = (*l + *r) * 0.5;
            *l += self.left.sample(input, feedback) * self.wet;
            *r += self.right.sample(input, feedback) * self.wet;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn room_has_a_stereo_decaying_tail_and_bypass_and_reset_remove_it() {
        for rate in [16_000, 48_000, 192_000] {
            let mut room = Reverb::new(rate);
            let mut left = vec![0.; rate as usize * 4];
            let mut right = left.clone();
            left[0] = 1.;
            right[0] = 1.;
            assert_no_alloc::assert_no_alloc(|| {
                room.process(&mut left, &mut right, 30 | (40 << 8))
            });
            assert_eq!(left[0], 1.);
            assert_ne!(left, right);
            assert!(
                left[rate as usize / 40..rate as usize]
                    .iter()
                    .any(|x| x.abs() > 0.001)
            );
            assert!(left.iter().all(|x| x.is_finite() && x.abs() <= 1.));
            assert!(left[rate as usize * 3..].iter().all(|x| x.abs() < 0.0001));
            let mut silence = [0.; 256];
            let mut other = silence;
            assert_no_alloc::assert_no_alloc(|| {
                room.clear();
                room.process(&mut silence, &mut other, 30 | (40 << 8));
            });
            assert_eq!(silence, [0.; 256]);
            let mut dry = [0.2; 256];
            room.process(&mut dry, &mut other, 0);
            assert_eq!(dry, [0.2; 256]);
        }
    }

    #[test]
    fn larger_room_extends_the_tail_and_amount_scales_the_return() {
        fn tail(amount: u32, size: u32) -> f64 {
            let mut room = Reverb::new(48_000);
            let mut left = vec![0.; 96_000];
            let mut right = left.clone();
            left[0] = 1.;
            right[0] = 1.;
            room.process(&mut left, &mut right, amount | (size << 8));
            left[48_000..].iter().map(|s| f64::from(*s).powi(2)).sum()
        }
        let small = tail(20, 0);
        let large = tail(20, 100);
        assert!(large > small * 10.);
        let quiet = tail(10, 40);
        let loud = tail(40, 40);
        assert!(loud > quiet * 14. && loud < quiet * 18.);
    }
}
