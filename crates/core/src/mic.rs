//! The microphone: how it is plugged in, and the processing the GoXLR applies
//! to it. The device cannot be asked for any of it, so the app keeps it and
//! sends it.

use goxlr_hub_device::{Device, DeviceError};
use goxlr_hub_protocol::{
    COMPRESSOR_ATTACK_MS, COMPRESSOR_RATIOS, COMPRESSOR_RELEASE_MS, EffectKey, EqBand,
    GATE_TIMES_MS, MicParamKey, MicType, eq_frequency_value, gate_attenuation_db,
};
use serde::{Deserialize, Serialize};

/// The preamp gives 0 to 72 dB.
pub const MAX_GAIN_DB: u8 = 72;

/// The gain of a microphone type that was never set.
const STARTING_GAIN_DB: u8 = 30;

/// What the device is told once, the way GoXLR Utility does: the gate mode,
/// the gate switched on, and the compressor selected.
const FIXED_EFFECTS: [(EffectKey, i32); 3] = [
    (EffectKey::GateMode, 2),
    (EffectKey::GateEnabled, 1),
    (EffectKey::MicCompSelect, 1),
];

/// The frequencies the bands of the equaliser can be moved between, in
/// hertz: four low bands, three middle ones, three high ones.
const fn band_range(band: usize) -> (f32, f32) {
    match band {
        0..=3 => (30.0, 300.0),
        4..=6 => (300.0, 2000.0),
        _ => (2000.0, 18000.0),
    }
}

const STARTING_FREQUENCIES: [f32; EqBand::COUNT] = [
    31.5, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// The equaliser turns a band up or down by 9 dB at most.
const MAX_EQ_GAIN_DB: i8 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GateSetting {
    /// -59 to 0 dB: the gate opens above it.
    Threshold,
    /// 0 to 100 %: how far the closed gate turns the microphone down.
    Attenuation,
    /// A rank in `GATE_TIMES_MS`.
    Attack,
    /// A rank in `GATE_TIMES_MS`.
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompressorSetting {
    /// -40 to 0 dB: the compressor works above it.
    Threshold,
    /// A rank in `COMPRESSOR_RATIOS`.
    Ratio,
    /// A rank in `COMPRESSOR_ATTACK_MS`.
    Attack,
    /// A rank in `COMPRESSOR_RELEASE_MS`.
    Release,
    /// -6 to 24 dB, added after the compression.
    MakeupGain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Gate {
    pub threshold: i8,
    pub attenuation: u8,
    pub attack: u8,
    pub release: u8,
}

impl Gate {
    fn with(mut self, setting: GateSetting, value: i32) -> Self {
        let last_time = GATE_TIMES_MS.len() - 1;
        match setting {
            GateSetting::Threshold => self.threshold = clamp(value, -59, 0),
            GateSetting::Attenuation => self.attenuation = clamp(value, 0, 100),
            GateSetting::Attack => self.attack = rank(value, last_time),
            GateSetting::Release => self.release = rank(value, last_time),
        }
        self
    }

    /// The full-size GoXLR wants the gate as a parameter and as an effect.
    fn send(self, device: &mut dyn Device, setting: GateSetting) -> Result<(), DeviceError> {
        let (effect, param, value) = match setting {
            GateSetting::Threshold => (
                EffectKey::GateThreshold,
                MicParamKey::GateThreshold,
                self.threshold,
            ),
            GateSetting::Attenuation => (
                EffectKey::GateAttenuation,
                MicParamKey::GateAttenuation,
                gate_attenuation_db(self.attenuation),
            ),
            GateSetting::Attack => (
                EffectKey::GateAttack,
                MicParamKey::GateAttack,
                self.attack.cast_signed(),
            ),
            GateSetting::Release => (
                EffectKey::GateRelease,
                MicParamKey::GateRelease,
                self.release.cast_signed(),
            ),
        };
        device.set_mic_param(param, f32::from(value))?;
        device.set_effect(effect, i32::from(value))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Compressor {
    pub threshold: i8,
    pub ratio: u8,
    pub attack: u8,
    pub release: u8,
    pub makeup_gain: i8,
}

impl Compressor {
    fn with(mut self, setting: CompressorSetting, value: i32) -> Self {
        match setting {
            CompressorSetting::Threshold => self.threshold = clamp(value, -40, 0),
            CompressorSetting::Ratio => self.ratio = rank(value, COMPRESSOR_RATIOS.len() - 1),
            CompressorSetting::Attack => self.attack = rank(value, COMPRESSOR_ATTACK_MS.len() - 1),
            CompressorSetting::Release => {
                self.release = rank(value, COMPRESSOR_RELEASE_MS.len() - 1);
            }
            CompressorSetting::MakeupGain => self.makeup_gain = clamp(value, -6, 24),
        }
        self
    }

    /// The full-size GoXLR wants the compressor as a parameter and as an
    /// effect. The parameter takes the ratio itself, the effect its rank.
    fn send(self, device: &mut dyn Device, setting: CompressorSetting) -> Result<(), DeviceError> {
        let (effect, param, value) = match setting {
            CompressorSetting::Threshold => (
                EffectKey::CompressorThreshold,
                MicParamKey::CompressorThreshold,
                self.threshold,
            ),
            CompressorSetting::Ratio => {
                let ratio = COMPRESSOR_RATIOS[usize::from(self.ratio)];
                device.set_mic_param(MicParamKey::CompressorRatio, ratio)?;
                return device.set_effect(EffectKey::CompressorRatio, i32::from(self.ratio));
            }
            CompressorSetting::Attack => (
                EffectKey::CompressorAttack,
                MicParamKey::CompressorAttack,
                self.attack.cast_signed(),
            ),
            CompressorSetting::Release => (
                EffectKey::CompressorRelease,
                MicParamKey::CompressorRelease,
                self.release.cast_signed(),
            ),
            CompressorSetting::MakeupGain => (
                EffectKey::CompressorMakeupGain,
                MicParamKey::CompressorMakeupGain,
                self.makeup_gain,
            ),
        };
        device.set_mic_param(param, f32::from(value))?;
        device.set_effect(effect, i32::from(value))
    }
}

/// One band of the equaliser: where it sits and how much it turns up or
/// down.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EqPoint {
    /// In hertz.
    pub frequency: f32,
    /// -9 to 9 dB.
    pub gain: i8,
}

fn clamp<T: TryFrom<i32>>(value: i32, min: i32, max: i32) -> T {
    // The bounds always fit the type the value is kept in.
    T::try_from(value.clamp(min, max)).unwrap_or_else(|_| unreachable!())
}

fn rank(value: i32, last: usize) -> u8 {
    let last = u8::try_from(last).unwrap_or(u8::MAX);
    clamp(value, 0, i32::from(last))
}

#[derive(Debug, Clone, PartialEq)]
pub struct MicState {
    /// `None` until it is chosen: a wrong guess could silence the
    /// microphone, or send phantom power to one that must not get any.
    pub mic_type: Option<MicType>,
    /// The gain of each type, in `MicType::ALL` order, 0 to 72 dB.
    pub gains: [u8; MicType::ALL.len()],
    pub gate: Gate,
    pub compressor: Compressor,
    /// In `EqBand::ALL` order.
    pub equalizer: [EqPoint; EqBand::COUNT],
    /// 0 to 100.
    pub de_esser: u8,
}

impl Default for MicState {
    /// The microphone of the virtual device: a dynamic one.
    fn default() -> Self {
        Self {
            mic_type: Some(MicType::Dynamic),
            ..Self::unknown()
        }
    }
}

impl MicState {
    /// The microphone of a real device the app meets: nobody said yet how
    /// it is plugged in. The processing starts neutral.
    pub fn unknown() -> Self {
        Self {
            mic_type: None,
            gains: [STARTING_GAIN_DB; MicType::ALL.len()],
            gate: Gate {
                threshold: -30,
                attenuation: 100,
                attack: 0,
                release: 19,
            },
            compressor: Compressor {
                threshold: 0,
                ratio: 9,
                attack: 1,
                release: 9,
                makeup_gain: 0,
            },
            equalizer: STARTING_FREQUENCIES.map(|frequency| EqPoint { frequency, gain: 0 }),
            de_esser: 0,
        }
    }

    /// The gain of the microphone type in use, when one was chosen.
    pub fn gain(&self) -> Option<u8> {
        self.mic_type.map(|mic_type| self.gains[mic_type as usize])
    }

    /// How far a band can be moved: within its part of the spectrum, and
    /// never past its neighbours.
    pub fn frequency_bounds(&self, band: EqBand) -> (f32, f32) {
        let at = band as usize;
        let (mut min, mut max) = band_range(at);
        if let Some(lower) = at.checked_sub(1) {
            min = min.max(self.equalizer[lower].frequency);
        }
        if let Some(higher) = self.equalizer.get(at + 1) {
            max = max.min(higher.frequency);
        }
        (min, max)
    }

    /// Brings a device to this state. The type and the gain are only sent
    /// once they were chosen.
    pub fn send_all(&self, device: &mut dyn Device) -> Result<(), DeviceError> {
        if let (Some(mic_type), Some(gain)) = (self.mic_type, self.gain()) {
            device.set_mic_gain(mic_type, u16::from(gain))?;
        }
        for (key, value) in FIXED_EFFECTS {
            device.set_effect(key, value)?;
        }
        for setting in [
            GateSetting::Threshold,
            GateSetting::Attenuation,
            GateSetting::Attack,
            GateSetting::Release,
        ] {
            self.gate.send(device, setting)?;
        }
        for setting in [
            CompressorSetting::Threshold,
            CompressorSetting::Ratio,
            CompressorSetting::Attack,
            CompressorSetting::Release,
            CompressorSetting::MakeupGain,
        ] {
            self.compressor.send(device, setting)?;
        }
        for (band, point) in EqBand::ALL.into_iter().zip(self.equalizer) {
            send_frequency(device, band, point.frequency)?;
            device.set_effect(EffectKey::EqGain(band), i32::from(point.gain))?;
        }
        device.set_effect(EffectKey::DeEsser, i32::from(self.de_esser))
    }

    /// Chooses how the microphone is plugged in. The gain of that type goes
    /// with it.
    pub fn set_type(
        &mut self,
        device: &mut dyn Device,
        mic_type: MicType,
    ) -> Result<(), DeviceError> {
        device.set_mic_gain(mic_type, u16::from(self.gains[mic_type as usize]))?;
        self.mic_type = Some(mic_type);
        Ok(())
    }

    /// Sets the gain of the microphone type in use. Nothing happens while
    /// no type was chosen.
    pub fn set_gain(&mut self, device: &mut dyn Device, gain: u8) -> Result<(), DeviceError> {
        let Some(mic_type) = self.mic_type else {
            return Ok(());
        };
        let gain = gain.min(MAX_GAIN_DB);
        device.set_mic_gain(mic_type, u16::from(gain))?;
        self.gains[mic_type as usize] = gain;
        Ok(())
    }

    pub fn set_gate(
        &mut self,
        device: &mut dyn Device,
        setting: GateSetting,
        value: i32,
    ) -> Result<(), DeviceError> {
        let gate = self.gate.with(setting, value);
        gate.send(device, setting)?;
        self.gate = gate;
        Ok(())
    }

    pub fn set_compressor(
        &mut self,
        device: &mut dyn Device,
        setting: CompressorSetting,
        value: i32,
    ) -> Result<(), DeviceError> {
        let compressor = self.compressor.with(setting, value);
        compressor.send(device, setting)?;
        self.compressor = compressor;
        Ok(())
    }

    /// Moves a band of the equaliser. A frequency that is no number leaves
    /// the band where it is.
    pub fn set_eq_band(
        &mut self,
        device: &mut dyn Device,
        band: EqBand,
        frequency: f32,
        gain: i8,
    ) -> Result<(), DeviceError> {
        let current = self.equalizer[band as usize];
        let (min, max) = self.frequency_bounds(band);
        let point = EqPoint {
            frequency: if frequency.is_finite() {
                frequency.clamp(min, max)
            } else {
                current.frequency
            },
            gain: gain.clamp(-MAX_EQ_GAIN_DB, MAX_EQ_GAIN_DB),
        };
        if point.frequency != current.frequency {
            send_frequency(device, band, point.frequency)?;
        }
        if point.gain != current.gain {
            device.set_effect(EffectKey::EqGain(band), i32::from(point.gain))?;
        }
        self.equalizer[band as usize] = point;
        Ok(())
    }

    pub fn set_de_esser(&mut self, device: &mut dyn Device, amount: u8) -> Result<(), DeviceError> {
        let amount = amount.min(100);
        device.set_effect(EffectKey::DeEsser, i32::from(amount))?;
        self.de_esser = amount;
        Ok(())
    }

    pub(crate) fn view(&self) -> MicView {
        MicView {
            mic_type: self.mic_type,
            gain: self.gain(),
            gate: self.gate,
            compressor: self.compressor,
            equalizer: EqBand::ALL
                .into_iter()
                .zip(self.equalizer)
                .map(|(band, point)| {
                    let (min_frequency, max_frequency) = self.frequency_bounds(band);
                    EqBandView {
                        band,
                        frequency: point.frequency,
                        gain: point.gain,
                        min_frequency,
                        max_frequency,
                    }
                })
                .collect(),
            de_esser: self.de_esser,
        }
    }
}

fn send_frequency(
    device: &mut dyn Device,
    band: EqBand,
    frequency: f32,
) -> Result<(), DeviceError> {
    device.set_effect(EffectKey::EqFrequency(band), eq_frequency_value(frequency))
}

/// The microphone as the interface receives it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MicView {
    /// Nothing until it is chosen.
    pub mic_type: Option<MicType>,
    /// 0 to 72 dB, or nothing while no type is chosen.
    pub gain: Option<u8>,
    pub gate: Gate,
    pub compressor: Compressor,
    /// One entry per band, lowest first.
    pub equalizer: Vec<EqBandView>,
    pub de_esser: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBandView {
    pub band: EqBand,
    pub frequency: f32,
    pub gain: i8,
    /// How far the band can be moved, in hertz.
    pub min_frequency: f32,
    pub max_frequency: f32,
}

#[cfg(test)]
mod tests {
    use goxlr_hub_device::{VirtualHandle, open_virtual};

    use super::*;

    fn device() -> (Box<dyn Device>, VirtualHandle) {
        let (device, hands) = open_virtual().unwrap();
        (Box::new(device), hands)
    }

    #[test]
    fn a_microphone_nobody_described_is_sent_its_processing_and_no_gain() {
        let (mut device, hands) = device();
        MicState::unknown().send_all(device.as_mut()).unwrap();

        let state = hands.state();
        assert!(!state.mic_gain_set);
        assert_eq!(state.effects.len(), 33);
        assert_eq!(state.mic_params.len(), 9);
        assert_eq!(state.effects[&EffectKey::GateMode], 2);
        assert_eq!(state.effects[&EffectKey::GateEnabled], 1);
        assert_eq!(state.effects[&EffectKey::MicCompSelect], 1);
        assert_eq!(state.effects[&EffectKey::GateThreshold], -30);
        assert_eq!(state.effects[&EffectKey::GateAttenuation], -61);
        assert_eq!(state.effects[&EffectKey::GateRelease], 19);
        assert_eq!(state.mic_params[&MicParamKey::GateAttenuation], -61.0);
        assert_eq!(state.effects[&EffectKey::CompressorRatio], 9);
        assert_eq!(state.mic_params[&MicParamKey::CompressorRatio], 4.0);
        assert_eq!(state.effects[&EffectKey::EqFrequency(EqBand::Hz31)], 16);
        assert_eq!(state.effects[&EffectKey::EqFrequency(EqBand::Khz16)], 231);
        assert_eq!(state.effects[&EffectKey::EqGain(EqBand::Khz1)], 0);
        assert_eq!(state.effects[&EffectKey::DeEsser], 0);
    }

    #[test]
    fn a_described_microphone_is_sent_its_type_and_gain() {
        let (mut device, hands) = device();
        MicState::default().send_all(device.as_mut()).unwrap();
        let state = hands.state();
        assert_eq!((state.mic_type, state.mic_gain), (MicType::Dynamic, 30));
    }

    #[test]
    fn each_microphone_type_keeps_its_own_gain() {
        let (mut device, hands) = device();
        let mut mic = MicState::unknown();

        // No type yet: the gain has nowhere to go.
        mic.set_gain(device.as_mut(), 50).unwrap();
        assert!(!hands.state().mic_gain_set);
        assert_eq!(mic, MicState::unknown());
        assert_eq!(mic.gain(), None);

        mic.set_type(device.as_mut(), MicType::Dynamic).unwrap();
        mic.set_gain(device.as_mut(), 50).unwrap();
        assert_eq!(hands.state().mic_gain, 50);
        mic.set_type(device.as_mut(), MicType::Condenser).unwrap();
        let state = hands.state();
        assert_eq!((state.mic_type, state.mic_gain), (MicType::Condenser, 30));
        mic.set_gain(device.as_mut(), 200).unwrap();
        assert_eq!(hands.state().mic_gain, 72);

        mic.set_type(device.as_mut(), MicType::Dynamic).unwrap();
        assert_eq!(hands.state().mic_gain, 50);
        assert_eq!(mic.gain(), Some(50));
        assert_eq!(mic.gains, [50, 72, 30]);
    }

    #[test]
    fn the_gate_goes_both_ways_and_stays_within_bounds() {
        let (mut device, hands) = device();
        let mut mic = MicState::unknown();

        mic.set_gate(device.as_mut(), GateSetting::Threshold, -45)
            .unwrap();
        mic.set_gate(device.as_mut(), GateSetting::Attenuation, 50)
            .unwrap();
        mic.set_gate(device.as_mut(), GateSetting::Attack, 3)
            .unwrap();
        mic.set_gate(device.as_mut(), GateSetting::Release, 1000)
            .unwrap();
        let state = hands.state();
        assert_eq!(state.effects[&EffectKey::GateThreshold], -45);
        assert_eq!(state.mic_params[&MicParamKey::GateThreshold], -45.0);
        assert_eq!(state.effects[&EffectKey::GateAttenuation], -18);
        assert_eq!(state.mic_params[&MicParamKey::GateAttenuation], -18.0);
        assert_eq!(state.effects[&EffectKey::GateAttack], 3);
        assert_eq!(state.mic_params[&MicParamKey::GateAttack], 3.0);
        assert_eq!(state.effects[&EffectKey::GateRelease], 45);
        assert_eq!(
            mic.gate,
            Gate {
                threshold: -45,
                attenuation: 50,
                attack: 3,
                release: 45
            }
        );
        // Only what was set was sent.
        assert_eq!(state.effects.len(), 4);

        mic.set_gate(device.as_mut(), GateSetting::Threshold, i32::MIN)
            .unwrap();
        mic.set_gate(device.as_mut(), GateSetting::Attenuation, 101)
            .unwrap();
        mic.set_gate(device.as_mut(), GateSetting::Attack, -1)
            .unwrap();
        assert_eq!(
            (mic.gate.threshold, mic.gate.attenuation, mic.gate.attack),
            (-59, 100, 0)
        );
        mic.set_gate(device.as_mut(), GateSetting::Threshold, 5)
            .unwrap();
        assert_eq!(hands.state().effects[&EffectKey::GateThreshold], 0);
    }

    #[test]
    fn the_compressor_sends_the_ratio_as_a_rank_and_as_a_number() {
        let (mut device, hands) = device();
        let mut mic = MicState::unknown();

        mic.set_compressor(device.as_mut(), CompressorSetting::Ratio, 7)
            .unwrap();
        mic.set_compressor(device.as_mut(), CompressorSetting::Threshold, -20)
            .unwrap();
        mic.set_compressor(device.as_mut(), CompressorSetting::Attack, 19)
            .unwrap();
        mic.set_compressor(device.as_mut(), CompressorSetting::Release, 20)
            .unwrap();
        mic.set_compressor(device.as_mut(), CompressorSetting::MakeupGain, 30)
            .unwrap();
        let state = hands.state();
        assert_eq!(state.effects[&EffectKey::CompressorRatio], 7);
        assert_eq!(state.mic_params[&MicParamKey::CompressorRatio], 2.5);
        assert_eq!(state.effects[&EffectKey::CompressorThreshold], -20);
        assert_eq!(state.mic_params[&MicParamKey::CompressorThreshold], -20.0);
        assert_eq!(state.effects[&EffectKey::CompressorAttack], 19);
        assert_eq!(state.effects[&EffectKey::CompressorRelease], 19);
        assert_eq!(state.effects[&EffectKey::CompressorMakeupGain], 24);
        assert_eq!(state.mic_params[&MicParamKey::CompressorMakeupGain], 24.0);

        mic.set_compressor(device.as_mut(), CompressorSetting::Ratio, 99)
            .unwrap();
        mic.set_compressor(device.as_mut(), CompressorSetting::Threshold, -99)
            .unwrap();
        mic.set_compressor(device.as_mut(), CompressorSetting::MakeupGain, -99)
            .unwrap();
        assert_eq!(
            mic.compressor,
            Compressor {
                threshold: -40,
                ratio: 14,
                attack: 19,
                release: 19,
                makeup_gain: -6
            }
        );
        assert_eq!(
            hands.state().mic_params[&MicParamKey::CompressorRatio],
            64.0
        );
    }

    #[test]
    fn an_equaliser_band_moves_within_its_range_and_its_neighbours() {
        let (mut device, hands) = device();
        let mut mic = MicState::unknown();

        mic.set_eq_band(device.as_mut(), EqBand::Khz1, 1500.0, 6)
            .unwrap();
        let state = hands.state();
        assert_eq!(state.effects[&EffectKey::EqFrequency(EqBand::Khz1)], 149);
        assert_eq!(state.effects[&EffectKey::EqGain(EqBand::Khz1)], 6);

        // Only what changed is sent.
        mic.set_eq_band(device.as_mut(), EqBand::Hz63, 63.0, -9)
            .unwrap();
        let state = hands.state();
        assert_eq!(state.effects.len(), 3);
        assert_eq!(state.effects[&EffectKey::EqGain(EqBand::Hz63)], -9);

        // 500 Hz stops at the band above, which now sits at 1500 Hz.
        assert_eq!(mic.frequency_bounds(EqBand::Hz500), (300.0, 1500.0));
        mic.set_eq_band(device.as_mut(), EqBand::Hz500, 1900.0, 0)
            .unwrap();
        assert_eq!(mic.equalizer[4].frequency, 1500.0);
        // 2 kHz stops at the end of the middle bands, and at the band below.
        assert_eq!(mic.frequency_bounds(EqBand::Khz2), (1500.0, 2000.0));
        mic.set_eq_band(device.as_mut(), EqBand::Khz2, 20.0, 0)
            .unwrap();
        assert_eq!(mic.equalizer[6].frequency, 1500.0);
        // The ends only have one neighbour.
        assert_eq!(mic.frequency_bounds(EqBand::Hz31), (30.0, 63.0));
        assert_eq!(mic.frequency_bounds(EqBand::Khz16), (8000.0, 18000.0));

        mic.set_eq_band(device.as_mut(), EqBand::Khz16, f32::INFINITY, 100)
            .unwrap();
        assert_eq!(
            mic.equalizer[9],
            EqPoint {
                frequency: 16000.0,
                gain: 9
            }
        );
        mic.set_eq_band(device.as_mut(), EqBand::Khz16, f32::NAN, -100)
            .unwrap();
        assert_eq!(
            mic.equalizer[9],
            EqPoint {
                frequency: 16000.0,
                gain: -9
            }
        );
        assert_eq!(hands.state().effects[&EffectKey::EqGain(EqBand::Khz16)], -9);
    }

    #[test]
    fn the_de_esser_stops_at_a_hundred() {
        let (mut device, hands) = device();
        let mut mic = MicState::unknown();
        mic.set_de_esser(device.as_mut(), 35).unwrap();
        assert_eq!(hands.state().effects[&EffectKey::DeEsser], 35);
        mic.set_de_esser(device.as_mut(), 250).unwrap();
        assert_eq!(hands.state().effects[&EffectKey::DeEsser], 100);
        assert_eq!(mic.de_esser, 100);
    }
}
