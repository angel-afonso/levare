//! Small owned binding to sfizz 1.2.3's C interface. No native code is required
//! to build or use the app's default engine. Symbols live as long as every synth.
#[cfg(test)]
use super::manifest::vsco_instruments;
use super::{VscoPack, first_pack_with, manifest::DynamicsMode};
use crate::{AudioError, error};
use libloading::Library;
use score_core::{Articulation, Instrument, PlayingTechnique, Score};
use score_platform::sound_engine::{NoteEvent, SoundEngine};
use score_playback::{PlaybackPlan, Tempo};
use std::{
    collections::HashMap,
    ffi::{CString, c_char, c_void},
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::Arc,
};

type Synth = *mut c_void;
struct Api {
    create: unsafe extern "C" fn() -> Synth,
    free: unsafe extern "C" fn(Synth),
    load: unsafe extern "C" fn(Synth, *const c_char, *const c_char) -> bool,
    rate: unsafe extern "C" fn(Synth, f32),
    block: unsafe extern "C" fn(Synth, i32),
    voices: unsafe extern "C" fn(Synth, i32),
    volume: unsafe extern "C" fn(Synth, f32),
    on: unsafe extern "C" fn(Synth, i32, i32, i32),
    send_cc: unsafe extern "C" fn(Synth, i32, i32, i32),
    off: unsafe extern "C" fn(Synth, i32, i32, i32),
    silence: unsafe extern "C" fn(Synth),
    render: unsafe extern "C" fn(Synth, *mut *mut f32, i32, i32),
    preloaded: unsafe extern "C" fn(Synth) -> usize,
    offline: unsafe extern "C" fn(Synth),
    _library: Library,
}

impl Api {
    fn open(path: &Path) -> Result<Arc<Self>, AudioError> {
        // SAFETY: the optional pack explicitly supplies a sfizz library. Every
        // symbol is checked before use, with types from the pinned sfizz.h.
        unsafe {
            let library = Library::new(path).map_err(error)?;
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get(concat!($name, "\0").as_bytes())
                        .map_err(error)?
                };
            }
            Ok(Arc::new(Self {
                create: symbol!("sfizz_create_synth"),
                free: symbol!("sfizz_free"),
                load: symbol!("sfizz_load_string"),
                rate: symbol!("sfizz_set_sample_rate"),
                block: symbol!("sfizz_set_samples_per_block"),
                voices: symbol!("sfizz_set_num_voices"),
                volume: symbol!("sfizz_set_volume"),
                on: symbol!("sfizz_send_note_on"),
                send_cc: symbol!("sfizz_send_cc"),
                off: symbol!("sfizz_send_note_off"),
                silence: symbol!("sfizz_all_sound_off"),
                render: symbol!("sfizz_render_block"),
                preloaded: symbol!("sfizz_get_num_preloaded_samples"),
                offline: symbol!("sfizz_enable_freewheeling"),
                _library: library,
            }))
        }
    }
}

struct Voice {
    api: Arc<Api>,
    synth: NonNull<c_void>,
    range: (u8, u8),
    keys: [bool; 128],
    last_cc1: Option<u8>,
}

// SAFETY: the synth is uniquely owned. It is prepared without an audio stream,
// then moved once into that stream's callback; CT and RT calls never overlap.
unsafe impl Send for Voice {}

impl Voice {
    /// `dynamic_layers` respreads the recorded velocity layers over the written
    /// dynamics; see `spread_velocity_layers`. Loudness still follows velocity.
    fn load(
        api: &Arc<Api>,
        path: &Path,
        rate: u32,
        dynamic_layers: bool,
    ) -> Result<Self, AudioError> {
        let path_c = CString::new(path.to_string_lossy().as_bytes()).map_err(error)?;
        let text = std::fs::read_to_string(path).map_err(error)?;
        let patch = if dynamic_layers {
            spread_velocity_layers(&text)
        } else {
            text.clone()
        };
        let text_c = CString::new(patch).map_err(error)?;
        // SAFETY: all calls here run on the preparation thread, before callback
        // ownership. A Voice guard frees the handle on every error path.
        unsafe {
            let synth =
                NonNull::new((api.create)()).ok_or_else(|| error("No se pudo crear sfizz"))?;
            let voice = Self {
                api: api.clone(),
                synth,
                range: key_range(&text),
                keys: [false; 128],
                last_cc1: None,
            };
            (api.rate)(synth.as_ptr(), rate as f32);
            (api.block)(synth.as_ptr(), 64);
            (api.voices)(synth.as_ptr(), 64);
            (api.volume)(synth.as_ptr(), -4.);
            // The real path still resolves the patch's relative sample folders.
            if !(api.load)(synth.as_ptr(), path_c.as_ptr(), text_c.as_ptr())
                || (api.preloaded)(synth.as_ptr()) == 0
            {
                return Err(error(format!(
                    "No se pudieron cargar las muestras de {}",
                    path.display()
                )));
            }
            Ok(voice)
        }
    }
    fn on(&mut self, key: u8, velocity: u8) {
        self.keys[usize::from(key)] = true;
        // SAFETY: exclusive callback ownership, valid MIDI numbers, delay zero
        // inside the following block; all buffers/voices were reserved at load.
        unsafe {
            (self.api.on)(self.synth.as_ptr(), 0, i32::from(key), i32::from(velocity));
        }
    }
    fn cc1(&mut self, value: u8) {
        if !update_cc1(&mut self.last_cc1, value) {
            return;
        }
        // SAFETY: same unique callback ownership and zero-delay block as on().
        unsafe {
            (self.api.send_cc)(self.synth.as_ptr(), 0, 1, i32::from(value));
        }
    }
    fn off(&mut self, key: u8) {
        if std::mem::replace(&mut self.keys[usize::from(key)], false) {
            // SAFETY: same ownership and event ordering as on().
            unsafe {
                (self.api.off)(self.synth.as_ptr(), 0, i32::from(key), 0);
            }
        }
    }
    fn silence(&mut self, immediate: bool) {
        self.last_cc1 = None;
        if immediate {
            // SAFETY: sfizz_all_sound_off is an RT operation on the owned synth.
            unsafe {
                (self.api.silence)(self.synth.as_ptr());
            }
            self.keys.fill(false);
        } else {
            for key in 0..128 {
                self.off(key);
            }
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        debug_assert_eq!(left.len(), right.len());
        debug_assert!(left.len() <= 64);
        let mut channels = [left.as_mut_ptr(), right.as_mut_ptr()];
        // SAFETY: two distinct writable planes, each at least num_frames long.
        // No CT operation can run while the audio callback owns this object.
        unsafe {
            (self.api.render)(
                self.synth.as_ptr(),
                channels.as_mut_ptr(),
                2,
                left.len() as i32,
            );
        }
    }
}

fn update_cc1(last: &mut Option<u8>, value: u8) -> bool {
    if *last == Some(value) {
        return false;
    }
    *last = Some(value);
    true
}
impl Drop for Voice {
    fn drop(&mut self) {
        // SAFETY: unique ownership, live Api/library, audio stream stopped first.
        unsafe {
            (self.api.free)(self.synth.as_ptr());
        }
    }
}

/// VSCO splits velocity evenly (0–62/63–127, or thirds and quarters), but the
/// written dynamics only span ppp=32 to fff=124: mp…fff all reached the loud
/// layer. Map each layer boundary from the full 0–127 axis onto that span, so
/// layers divide the dynamics evenly (ppp–mp | mf–fff with two layers). Only
/// region limits change; the velocity sent, and therefore loudness, does not.
fn spread_velocity_layers(text: &str) -> String {
    const PPP: u32 = 32;
    const FFF: u32 = 124;
    let boundary = |lovel: u32| PPP + (lovel * (FFF - PPP) + 63) / 127;
    let mut patch = String::with_capacity(text.len());
    for piece in text.split_inclusive(char::is_whitespace) {
        let token = piece.trim_end();
        let spread = if let Some(value) = token
            .strip_prefix("lovel=")
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|v| (1..=127).contains(v))
        {
            Some(format!("lovel={}", boundary(value)))
        } else {
            token
                .strip_prefix("hivel=")
                .and_then(|v| v.parse::<u32>().ok())
                .filter(|v| *v < 127)
                .map(|value| format!("hivel={}", boundary(value + 1) - 1))
        };
        match spread {
            Some(token) => {
                patch.push_str(&token);
                patch.push_str(&piece[piece.trim_end().len()..]);
            }
            None => patch.push_str(piece),
        }
    }
    patch
}

/// SFZ region limits may use MIDI integers or note names (as VPO does).
/// Outside their recorded range, notes fall back to General MIDI.
fn key_range(text: &str) -> (u8, u8) {
    let mut low = 127;
    let mut high = 0;
    for line in text.lines() {
        let line = line.split_once("//").map_or(line, |(content, _)| content);
        for token in line.split_whitespace() {
            if let Some(value) = token
                .strip_prefix("lokey=")
                .and_then(parse_sfz_key)
                .or_else(|| token.strip_prefix("key=").and_then(parse_sfz_key))
            {
                low = low.min(value);
            }
            if let Some(value) = token
                .strip_prefix("hikey=")
                .and_then(parse_sfz_key)
                .or_else(|| token.strip_prefix("key=").and_then(parse_sfz_key))
            {
                high = high.max(value);
            }
        }
    }
    if low <= high { (low, high) } else { (0, 127) }
}

fn parse_sfz_key(value: &str) -> Option<u8> {
    if let Ok(number) = value.parse::<u8>() {
        return (number <= 127).then_some(number);
    }
    let mut chars = value.chars();
    let semitone = match chars.next()?.to_ascii_lowercase() {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (alteration, octave) = if let Some(rest) = rest.strip_prefix('#') {
        (1, rest)
    } else if let Some(rest) = rest.strip_prefix('b') {
        (-1, rest)
    } else {
        (0, rest)
    };
    let octave: i32 = octave.parse().ok()?;
    if !(-1..=9).contains(&octave) {
        return None;
    }
    u8::try_from((octave + 1) * 12 + semitone + alteration)
        .ok()
        .filter(|key| *key <= 127)
}

struct Lane {
    channel: u8,
    instrument: Instrument,
    dynamics: DynamicsMode,
    /// Hairpin target from controller 11, and the gain that follows it per
    /// sample: a slow crescendo arrives in 127 steps that would otherwise zipper.
    expression: f32,
    expression_gain: f32,
    /// After an immediate silence nothing sounds, so the next level is exact.
    expression_settled: bool,
    volume: f32,
    balance: (f32, f32),
    gains: (f32, f32),
    mix_initialized: bool,
    normal: Voice,
    pizz: Option<Voice>,
    short: Option<Voice>,
}

pub(crate) struct NativeBank {
    lanes: Vec<Lane>,
    left: [f32; 64],
    right: [f32; 64],
}
impl NativeBank {
    pub(crate) fn new(
        pack: &VscoPack,
        score: &Score,
        audition: Instrument,
        rate: u32,
    ) -> Result<Self, AudioError> {
        Self::new_many(std::slice::from_ref(pack), score, audition, rate)
    }

    pub(crate) fn new_many(
        packs: &[VscoPack],
        score: &Score,
        audition: Instrument,
        rate: u32,
    ) -> Result<Self, AudioError> {
        let mut apis: HashMap<PathBuf, Arc<Api>> = HashMap::new();
        let plan = PlaybackPlan::new(score, Tempo::default(), rate).map_err(error)?;
        let seats = crate::seats(score);
        let mut routes: Vec<_> = plan
            .tracks
            .iter()
            .map(|track| {
                let seat = seats
                    .iter()
                    .find(|(part, _)| *part == track.part)
                    .map(|(_, seat)| *seat)
                    .unwrap_or_default();
                (
                    track.channel,
                    score
                        .part(track.part)
                        .expect("validated track")
                        .instrument(),
                    seat,
                )
            })
            .collect();
        routes.push((15, audition, Default::default()));
        let mut seen = [false; score_playback::PLAYBACK_CHANNELS];
        let mut lanes = Vec::new();
        for (channel, instrument, seat) in routes {
            if std::mem::replace(&mut seen[usize::from(channel)], true) {
                continue;
            }
            let Some((pack, patch)) = first_pack_with(packs, instrument, seat) else {
                continue;
            };
            let library = pack
                .library
                .canonicalize()
                .unwrap_or_else(|_| pack.library.clone());
            if !apis.contains_key(&library) {
                apis.insert(library.clone(), Api::open(&library)?);
            }
            let api = &apis[&library];
            let layers = patch.spread_layers;
            lanes.push(Lane {
                channel,
                instrument,
                dynamics: patch.dynamics,
                expression: 1.,
                expression_gain: 1.,
                expression_settled: true,
                volume: 1.,
                balance: (1., 1.),
                gains: (1., 1.),
                mix_initialized: false,
                normal: Voice::load(api, &pack.root.join(&patch.sustain), rate, layers)?,
                pizz: patch
                    .pizzicato
                    .as_ref()
                    .map(|file| Voice::load(api, &pack.root.join(file), rate, layers))
                    .transpose()?,
                short: patch
                    .staccato
                    .as_ref()
                    .map(|file| pack.root.join(file))
                    .filter(|p| p.is_file())
                    .map(|path| Voice::load(api, &path, rate, layers))
                    .transpose()?,
            });
        }
        let mut bank = Self {
            lanes,
            left: [0.; 64],
            right: [0.; 64],
        };
        // Warm the native DSP outside the real-time callback.
        let mut left = [0.; 64];
        let mut right = [0.; 64];
        bank.render(&mut left, &mut right);
        Ok(bank)
    }
}
impl SoundEngine for NativeBank {
    fn offline(&mut self) {
        for lane in &mut self.lanes {
            for voice in std::iter::once(&mut lane.normal)
                .chain(lane.pizz.iter_mut())
                .chain(lane.short.iter_mut())
            {
                // SAFETY: mode change happens on the processing thread before rendering.
                unsafe {
                    (voice.api.offline)(voice.synth.as_ptr());
                }
            }
        }
    }
    fn midi(&mut self, channel: u8, status: u8, a: u8, b: u8) {
        if let Some(lane) = self.lanes.iter_mut().find(|lane| lane.channel == channel) {
            match status {
                0xb0 if a == 11 => {
                    lane.expression = (f32::from(b) / 127.).powi(2);
                    if std::mem::take(&mut lane.expression_settled) {
                        lane.expression_gain = lane.expression;
                    }
                }
                0xb0 if a == 7 => lane.volume = (f32::from(b) / 100.).powi(2),
                0xb0 if a == 10 => {
                    let pan = (f32::from(b) - 64.) / if b < 64 { 64. } else { 63. };
                    lane.balance = ((1. - pan.max(0.)).sqrt(), (1. + pan.min(0.)).sqrt());
                    if !lane.mix_initialized {
                        lane.gains = (lane.volume * lane.balance.0, lane.volume * lane.balance.1);
                        lane.mix_initialized = true;
                    }
                }
                _ => {}
            }
        }
    }
    /// Returns false when this channel/instrument/key needs the GM fallback.
    fn note_on(&mut self, channel: u8, note: NoteEvent) -> bool {
        if note.harmonic.is_some() {
            return false;
        }
        let Some(lane) = self
            .lanes
            .iter_mut()
            .find(|lane| lane.channel == channel && lane.instrument == note.instrument)
        else {
            return false;
        };
        let key = note.key;
        let covers = |voice: &Voice| (voice.range.0..=voice.range.1).contains(&key);
        // Pizzicato wins over the bowed short recording. A slurred staccato is
        // portato: the sustained recording with the plan's shorter length.
        let voice = if note.technique == PlayingTechnique::Pizzicato {
            lane.pizz.as_mut()
        } else if note.articulation == Some(Articulation::Staccato)
            && !note.legato
            && lane.short.as_ref().is_some_and(covers)
        {
            lane.short.as_mut()
        } else {
            Some(&mut lane.normal)
        };
        let Some(voice) = voice.filter(|v| covers(v)) else {
            return false;
        };
        if lane.dynamics == DynamicsMode::Cc1 {
            voice.cc1(note.level);
        }
        voice.on(key, note.level);
        true
    }
    fn note_off(&mut self, channel: u8, key: u8) {
        for lane in self.lanes.iter_mut().filter(|lane| lane.channel == channel) {
            lane.normal.off(key);
            for voice in lane.pizz.iter_mut().chain(lane.short.iter_mut()) {
                voice.off(key);
            }
        }
    }
    fn silence(&mut self, channel: Option<u8>, immediate: bool) {
        for lane in self
            .lanes
            .iter_mut()
            .filter(|lane| channel.is_none_or(|c| lane.channel == c))
        {
            lane.normal.silence(immediate);
            for voice in lane.pizz.iter_mut().chain(lane.short.iter_mut()) {
                voice.silence(immediate);
            }
            lane.expression_settled |= immediate;
        }
    }
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len();
        for lane in &mut self.lanes {
            let mut gain_left = [0.; 64];
            let mut gain_right = [0.; 64];
            for i in 0..n {
                lane.gains.0 += (lane.volume * lane.balance.0 - lane.gains.0) * 0.005;
                lane.gains.1 += (lane.volume * lane.balance.1 - lane.gains.1) * 0.005;
                lane.expression_gain += (lane.expression - lane.expression_gain) * 0.005;
                gain_left[i] = lane.gains.0 * lane.expression_gain;
                gain_right[i] = lane.gains.1 * lane.expression_gain;
            }
            for voice in std::iter::once(&mut lane.normal)
                .chain(lane.pizz.iter_mut())
                .chain(lane.short.iter_mut())
            {
                voice.render(&mut self.left[..n], &mut self.right[..n]);
                for i in 0..n {
                    left[i] += self.left[i] * gain_left[i];
                    right[i] += self.right[i] * gain_right[i];
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::InstrumentSpec;

    #[test]
    fn cc1_cache_is_per_voice_and_resets_even_after_a_zero_value() {
        let (mut normal, mut pizz) = (None, None);
        assert!(update_cc1(&mut normal, 0));
        assert!(!update_cc1(&mut normal, 0));
        assert!(
            update_cc1(&mut pizz, 0),
            "the first pizzicato attack has its own synth"
        );
        assert!(update_cc1(&mut normal, 110));
        assert!(!update_cc1(&mut pizz, 0));
        pizz = None;
        assert!(update_cc1(&mut pizz, 0), "silence resets the voice cache");
    }

    #[test]
    fn named_vpo_keys_define_the_real_range_without_comments_or_keyswitches() {
        let patch = "// lokey=c-1 hikey=g9\n<control> sw_lokey=c2 sw_hikey=c3\n<region> lokey=g3 hikey=g#3 sample=one.wav\n<region> lokey=c4 hikey=d4 sample=two.wav\n";
        assert_eq!(parse_sfz_key("g3"), Some(55));
        assert_eq!(parse_sfz_key("g#3"), Some(56));
        assert_eq!(parse_sfz_key("c32767"), None);
        assert_eq!(parse_sfz_key("c3000"), None);
        assert_eq!(parse_sfz_key("128"), None);
        assert_eq!(key_range(patch), (55, 62));
    }

    #[test]
    #[ignore = "requires locally installed Virtual Playing Orchestra and sfizz"]
    fn installed_vpo_plays_performance_pizzicato_staccato_and_cc1_per_voice() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../soundpacks/Virtual-Playing-Orchestra3/manifest.json");
        let pack = VscoPack::load(&manifest).unwrap();
        let score = Score::default();
        let mut bank = NativeBank::new(&pack, &score, Instrument::Violin, 48_000).unwrap();
        bank.offline();
        let note = NoteEvent {
            key: 69,
            level: 88,
            instrument: Instrument::Violin,
            technique: PlayingTechnique::Arco,
            harmonic: None,
            articulation: None,
            legato: false,
        };
        assert_no_alloc::assert_no_alloc(|| assert!(bank.note_on(15, note)));
        assert_eq!(bank.lanes[0].normal.last_cc1, Some(88));
        assert_no_alloc::assert_no_alloc(|| {
            assert!(bank.note_on(
                15,
                NoteEvent {
                    technique: PlayingTechnique::Pizzicato,
                    ..note
                }
            ))
        });
        assert_eq!(bank.lanes[0].pizz.as_ref().unwrap().last_cc1, Some(88));
        assert_no_alloc::assert_no_alloc(|| {
            assert!(bank.note_on(
                15,
                NoteEvent {
                    articulation: Some(Articulation::Staccato),
                    ..note
                }
            ))
        });
        assert_eq!(bank.lanes[0].short.as_ref().unwrap().last_cc1, Some(88));
        let mut left = [0.; 64];
        let mut right = left;
        let mut energy = 0.;
        assert_no_alloc::assert_no_alloc(|| {
            for _ in 0..256 {
                left.fill(0.);
                right.fill(0.);
                bank.render(&mut left, &mut right);
                energy += left
                    .iter()
                    .chain(&right)
                    .map(|sample| sample * sample)
                    .sum::<f32>();
            }
        });
        assert!(energy > 0.0001, "VPO must render actual samples");
        bank.silence(None, true);
        assert!(bank.lanes[0].normal.last_cc1.is_none());
        assert!(bank.lanes[0].pizz.as_ref().unwrap().last_cc1.is_none());
        assert!(bank.lanes[0].short.as_ref().unwrap().last_cc1.is_none());
        // Same note velocity, different CC1 than the SFZ's initial set_cc1=64.
        let mut render_level = |level, mode| {
            let mut candidate = pack.clone();
            candidate
                .spec
                .instruments
                .iter_mut()
                .find(|(instrument, _)| *instrument == Instrument::Violin)
                .unwrap()
                .1
                .main
                .first
                .dynamics = mode;
            let mut bank = NativeBank::new(&candidate, &score, Instrument::Violin, 48_000).unwrap();
            bank.offline();
            assert!(bank.note_on(15, NoteEvent { level, ..note }));
            let mut sum = 0.;
            for _ in 0..256 {
                left.fill(0.);
                right.fill(0.);
                bank.render(&mut left, &mut right);
                sum += left
                    .iter()
                    .chain(&right)
                    .map(|sample| sample * sample)
                    .sum::<f32>();
            }
            sum
        };
        let soft_cc = render_level(40, DynamicsMode::Cc1);
        let soft_velocity = render_level(40, DynamicsMode::Velocity);
        let loud_cc = render_level(110, DynamicsMode::Cc1);
        assert!(
            soft_cc > 0.0001 && loud_cc > soft_cc * 2.,
            "CC1 must shape real VPO output"
        );
        assert!(
            (soft_cc - soft_velocity).abs() > soft_cc.min(soft_velocity) * 0.05,
            "CC1 must change energy beyond key velocity alone"
        );
    }

    #[test]
    #[ignore = "requires locally installed Virtual Playing Orchestra and sfizz"]
    fn every_declared_vpo_patch_prepares_and_produces_recorded_audio() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../soundpacks/Virtual-Playing-Orchestra3/manifest.json");
        let pack = VscoPack::load(&manifest).unwrap();
        let score = Score::default();
        let patches =
            pack.spec.instruments.iter().flat_map(|(instrument, spec)| {
                spec.patches().map(move |patch| (instrument, patch))
            });
        for (instrument, patch) in patches {
            // Every declared recording set, alone, as the audition patch.
            let mut single = pack.clone();
            single.spec.instruments = vec![(*instrument, InstrumentSpec::from(patch.clone()))];
            for (kind, available) in [
                ("sustain", true),
                ("pizzicato", patch.pizzicato.is_some()),
                ("staccato", patch.staccato.is_some()),
            ] {
                if !available {
                    continue;
                }
                let mut bank = NativeBank::new(&single, &score, *instrument, 48_000).unwrap();
                bank.offline();
                let lane = bank.lanes.iter().find(|lane| lane.channel == 15).unwrap();
                let voice = match kind {
                    "pizzicato" => lane.pizz.as_ref().unwrap(),
                    "staccato" => lane.short.as_ref().unwrap(),
                    _ => &lane.normal,
                };
                let (low, high) = voice.range;
                let middle = low + (high - low) / 2;
                let mut keys = if let Some(drum) = instrument.percussion_key() {
                    vec![drum]
                } else {
                    let mut keys = vec![middle, low, high];
                    keys.extend(low..=high);
                    keys
                };
                keys.dedup();
                let mut sounded = false;
                for key in keys {
                    let note = NoteEvent {
                        key,
                        level: 88,
                        instrument: *instrument,
                        technique: if kind == "pizzicato" {
                            PlayingTechnique::Pizzicato
                        } else {
                            PlayingTechnique::Arco
                        },
                        harmonic: None,
                        articulation: (kind == "staccato").then_some(Articulation::Staccato),
                        legato: false,
                    };
                    if !bank.note_on(15, note) {
                        continue;
                    }
                    let selected = match kind {
                        "pizzicato" => bank.lanes[0].pizz.as_ref().unwrap(),
                        "staccato" => bank.lanes[0].short.as_ref().unwrap(),
                        _ => &bank.lanes[0].normal,
                    };
                    if !selected.keys[usize::from(key)] {
                        bank.silence(None, true);
                        continue;
                    }
                    let mut left = [0.; 64];
                    let mut right = left;
                    let mut energy = 0.;
                    for _ in 0..512 {
                        left.fill(0.);
                        right.fill(0.);
                        bank.render(&mut left, &mut right);
                        energy += left
                            .iter()
                            .chain(&right)
                            .map(|sample| sample * sample)
                            .sum::<f32>();
                    }
                    bank.silence(None, true);
                    if energy > 0.000001 {
                        sounded = true;
                        break;
                    }
                }
                assert!(
                    sounded,
                    "VPO {instrument:?} {kind} has no audible recording in {low}..={high}"
                );
            }
        }
    }

    #[test]
    fn velocity_layers_divide_the_written_dynamics_without_touching_other_opcodes() {
        let two = "<region> lovel=0 hivel=62 key=60 sample=a.wav\n<region> lovel=63 hivel=127\tsample=b.wav\n";
        assert_eq!(
            spread_velocity_layers(two),
            "<region> lovel=0 hivel=77 key=60 sample=a.wav\n<region> lovel=78 hivel=127\tsample=b.wav\n"
        );
        let layer = |text: &str, level: u32| {
            let patch = spread_velocity_layers(text);
            let bounds: Vec<u32> = patch
                .split_whitespace()
                .filter_map(|t| t.strip_prefix("lovel=").and_then(|v| v.parse().ok()))
                .collect();
            bounds.iter().filter(|b| **b <= level).count() - 1
        };
        // ppp, pp, p, mp, mf, f, ff, fff.
        let dynamics = score_core::Dynamic::ALL.map(|d| u32::from(d.velocity()));
        let two = "lovel=0 hivel=62 lovel=63 hivel=127";
        assert_eq!(dynamics.map(|v| layer(two, v)), [0, 0, 0, 0, 1, 1, 1, 1]);
        let three = "lovel=0 hivel=41 lovel=42 hivel=83 lovel=84 hivel=127";
        assert_eq!(dynamics.map(|v| layer(three, v)), [0, 0, 0, 1, 1, 2, 2, 2]);
        let four = "lovel=0 hivel=30 lovel=31 hivel=62 lovel=63 hivel=94 lovel=95 hivel=127";
        assert_eq!(dynamics.map(|v| layer(four, v)), [0, 0, 1, 1, 2, 2, 3, 3]);
    }

    #[test]
    #[ignore = "requires the optional local VSCO CE library and sfizz engine"]
    fn mezzo_piano_plays_the_soft_violin_recording_and_mezzo_forte_the_loud_one() {
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../soundpacks/vsco-ce/manifest.json");
        let pack = VscoPack::load(&manifest).unwrap();
        let api = Api::open(&pack.library).unwrap();
        let path = pack.root.join("SViolinVib.sfz");
        let render = |layers, velocity| {
            let mut voice = Voice::load(&api, &path, 48_000, layers).unwrap();
            voice.on(69, velocity);
            let mut audio = Vec::new();
            for _ in 0..64 {
                let (mut left, mut right) = ([0f32; 64], [0f32; 64]);
                voice.render(&mut left, &mut right);
                audio.extend_from_slice(&left);
            }
            audio
        };
        let mp = score_core::Dynamic::Mp.velocity();
        let mf = score_core::Dynamic::Mf.velocity();
        // Both recordings sound; mp switches to the soft one, mf keeps the loud one.
        assert!(render(true, mp).iter().any(|v| v.abs() > 1e-4));
        assert_ne!(render(true, mp), render(false, mp));
        assert_eq!(render(true, mf), render(false, mf));
    }

    #[test]
    #[ignore = "requires the optional local VSCO CE library and sfizz engine"]
    fn real_short_recordings_are_selected_for_every_supported_instrument_and_pizz_wins() {
        let manifest =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../soundpacks/vsco-ce/manifest.json");
        let pack = VscoPack::load(&manifest).unwrap();
        for (instrument, patch) in vsco_instruments() {
            if patch.main.first.staccato.is_none() {
                continue;
            }
            let mut score = Score::default();
            let piano = score.parts()[0].id();
            score.add_part(instrument).unwrap();
            score.remove_part(piano).unwrap();
            let mut bank = NativeBank::new(&pack, &score, instrument, 48_000).unwrap();
            let lane = &bank.lanes[0];
            let short = lane
                .short
                .as_ref()
                .expect("Original VSCO short patch must load");
            let low = short.range.0.max(lane.normal.range.0);
            let high = short.range.1.min(lane.normal.range.1);
            assert!(low <= high);
            let key = low + (high - low) / 2;
            let staccato = NoteEvent {
                key,
                level: 88,
                instrument,
                technique: PlayingTechnique::Arco,
                harmonic: None,
                articulation: Some(Articulation::Staccato),
                legato: false,
            };
            assert!(bank.note_on(0, staccato));
            assert!(bank.lanes[0].short.as_ref().unwrap().keys[usize::from(key)]);
            assert!(!bank.lanes[0].normal.keys[usize::from(key)]);
            let mut left = [0.; 64];
            let mut right = left;
            let mut energy = 0.;
            assert_no_alloc::assert_no_alloc(|| {
                for _ in 0..128 {
                    left.fill(0.);
                    right.fill(0.);
                    bank.render(&mut left, &mut right);
                    energy += left.iter().chain(&right).map(|v| v * v).sum::<f32>();
                }
            });
            assert!(energy > 0.0001, "No real short audio for {instrument:?}");
            bank.note_off(0, key);
            assert!(!bank.lanes[0].short.as_ref().unwrap().keys[usize::from(key)]);
            bank.silence(None, true);
            // Portato (a slurred staccato) keeps the sustained recording.
            assert!(bank.note_on(
                0,
                NoteEvent {
                    legato: true,
                    ..staccato
                }
            ));
            assert!(bank.lanes[0].normal.keys[usize::from(key)]);
            assert!(!bank.lanes[0].short.as_ref().unwrap().keys[usize::from(key)]);
            bank.silence(None, true);
            // Another instrument on this channel is not this lane's recording.
            assert!(!bank.note_on(
                0,
                NoteEvent {
                    instrument: Instrument::Guitar,
                    ..staccato
                }
            ));
            if bank.lanes[0].pizz.is_some() {
                assert!(bank.note_on(
                    0,
                    NoteEvent {
                        technique: PlayingTechnique::Pizzicato,
                        ..staccato
                    }
                ));
                assert!(bank.lanes[0].pizz.as_ref().unwrap().keys[usize::from(key)]);
                assert!(!bank.lanes[0].short.as_ref().unwrap().keys[usize::from(key)]);
                bank.silence(None, true);
            }
            // Partial optional packs degrade to sustained recordings and the
            // host's common short length, without loading anything in the callback.
            bank.lanes[0].short = None;
            assert!(bank.note_on(0, staccato));
            assert!(bank.lanes[0].normal.keys[usize::from(key)]);
        }
    }
}
