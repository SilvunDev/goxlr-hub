//! The microphone processing of the full-size GoXLR: the keys its settings
//! are sent under, and the scales the device expects.
//!
//! The gate and the compressor are set twice, as an effect and as a
//! microphone parameter. The equaliser and the de-esser are effects only.

/// One of the ten bands of the equaliser, named after where it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "camelCase")
)]
pub enum EqBand {
    Hz31,
    Hz63,
    Hz125,
    Hz250,
    Hz500,
    Khz1,
    Khz2,
    Khz4,
    Khz8,
    Khz16,
}

impl EqBand {
    pub const ALL: [Self; 10] = [
        Self::Hz31,
        Self::Hz63,
        Self::Hz125,
        Self::Hz250,
        Self::Hz500,
        Self::Khz1,
        Self::Khz2,
        Self::Khz4,
        Self::Khz8,
        Self::Khz16,
    ];
    pub const COUNT: usize = Self::ALL.len();

    /// The effect that carries the frequency of the band; its gain is the
    /// next one.
    fn frequency_id(self) -> u32 {
        match self {
            Self::Hz31 => 0x0126,
            Self::Hz63 => 0x00f8,
            Self::Hz125 => 0x0113,
            Self::Hz250 => 0x0129,
            Self::Hz500 => 0x0116,
            Self::Khz1 => 0x011d,
            Self::Khz2 => 0x012c,
            Self::Khz4 => 0x0120,
            Self::Khz8 => 0x0109,
            Self::Khz16 => 0x012f,
        }
    }
}

/// A setting of the microphone processing sent as an effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EffectKey {
    GateMode,
    GateThreshold,
    GateEnabled,
    GateAttenuation,
    GateAttack,
    GateRelease,
    MicCompSelect,
    CompressorThreshold,
    CompressorRatio,
    CompressorAttack,
    CompressorRelease,
    CompressorMakeupGain,
    DeEsser,
    EqFrequency(EqBand),
    EqGain(EqBand),
}

impl EffectKey {
    const PLAIN: [Self; 13] = [
        Self::GateMode,
        Self::GateThreshold,
        Self::GateEnabled,
        Self::GateAttenuation,
        Self::GateAttack,
        Self::GateRelease,
        Self::MicCompSelect,
        Self::CompressorThreshold,
        Self::CompressorRatio,
        Self::CompressorAttack,
        Self::CompressorRelease,
        Self::CompressorMakeupGain,
        Self::DeEsser,
    ];

    pub fn all() -> impl Iterator<Item = Self> {
        Self::PLAIN
            .into_iter()
            .chain(EqBand::ALL.into_iter().map(Self::EqFrequency))
            .chain(EqBand::ALL.into_iter().map(Self::EqGain))
    }

    pub fn id(self) -> u32 {
        match self {
            Self::GateMode => 0x0010,
            Self::GateThreshold => 0x0011,
            Self::GateEnabled => 0x0014,
            Self::GateAttenuation => 0x0015,
            Self::GateAttack => 0x0016,
            Self::GateRelease => 0x0017,
            Self::MicCompSelect => 0x014b,
            Self::CompressorThreshold => 0x013d,
            Self::CompressorRatio => 0x013c,
            Self::CompressorAttack => 0x013e,
            Self::CompressorRelease => 0x013f,
            Self::CompressorMakeupGain => 0x0140,
            Self::DeEsser => 0x000b,
            Self::EqFrequency(band) => band.frequency_id(),
            Self::EqGain(band) => band.frequency_id() + 1,
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::all().find(|key| key.id() == id)
    }
}

/// A setting of the gate or of the compressor sent as a microphone
/// parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MicParamKey {
    GateThreshold,
    GateAttack,
    GateRelease,
    GateAttenuation,
    CompressorThreshold,
    CompressorRatio,
    CompressorAttack,
    CompressorRelease,
    CompressorMakeupGain,
}

impl MicParamKey {
    pub const ALL: [Self; 9] = [
        Self::GateThreshold,
        Self::GateAttack,
        Self::GateRelease,
        Self::GateAttenuation,
        Self::CompressorThreshold,
        Self::CompressorRatio,
        Self::CompressorAttack,
        Self::CompressorRelease,
        Self::CompressorMakeupGain,
    ];

    pub fn id(self) -> u32 {
        match self {
            Self::GateThreshold => 0x0003_0200,
            Self::GateAttack => 0x0003_0400,
            Self::GateRelease => 0x0003_0600,
            Self::GateAttenuation => 0x0003_0900,
            Self::CompressorThreshold => 0x0006_0200,
            Self::CompressorRatio => 0x0006_0300,
            Self::CompressorAttack => 0x0006_0400,
            Self::CompressorRelease => 0x0006_0600,
            Self::CompressorMakeupGain => 0x0006_0700,
        }
    }

    pub fn from_id(id: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.id() == id)
    }
}

/// The attack and release times of the gate, in milliseconds. The device is
/// sent the rank of a time, not the time.
pub const GATE_TIMES_MS: [u16; 46] = [
    10, 20, 30, 40, 50, 60, 70, 80, 90, 100, 110, 120, 130, 140, 150, 160, 170, 180, 190, 200, 250,
    300, 350, 400, 450, 500, 550, 600, 650, 700, 750, 800, 850, 900, 950, 1000, 1100, 1200, 1300,
    1400, 1500, 1600, 1700, 1800, 1900, 2000,
];

/// The ratios of the compressor. The effect is sent the rank of a ratio,
/// the microphone parameter the ratio itself.
pub const COMPRESSOR_RATIOS: [f32; 15] = [
    1.0, 1.1, 1.2, 1.4, 1.6, 1.8, 2.0, 2.5, 3.2, 4.0, 5.6, 8.0, 16.0, 32.0, 64.0,
];

/// The attack times of the compressor, in milliseconds, sent by rank.
pub const COMPRESSOR_ATTACK_MS: [u16; 20] = [
    0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 14, 16, 18, 20, 23, 26, 30, 35, 40,
];

/// The release times of the compressor, in milliseconds, sent by rank.
pub const COMPRESSOR_RELEASE_MS: [u16; 20] = [
    0, 15, 25, 35, 45, 55, 65, 75, 85, 100, 115, 140, 170, 230, 340, 680, 1000, 1500, 2000, 3000,
];

/// How far the closed gate turns the microphone down, in decibels.
const GATE_ATTENUATION_DB: [i8; 26] = [
    -6, -7, -8, -9, -10, -11, -12, -13, -14, -15, -16, -17, -18, -19, -20, -21, -22, -23, -24, -25,
    -26, -27, -28, -30, -32, -61,
];

/// The attenuation of the gate is set as a percentage; the device takes
/// decibels, along a scale that is not even.
pub fn gate_attenuation_db(percent: u8) -> i8 {
    let rank = if percent >= 100 {
        GATE_ATTENUATION_DB.len() - 1
    } else {
        usize::from(percent) * 24 / 100
    };
    GATE_ATTENUATION_DB[rank]
}

/// The device counts a frequency in twenty-fourths of an octave above 20 Hz.
pub fn eq_frequency_value(hertz: f32) -> i32 {
    if !hertz.is_finite() || hertz <= 20.0 {
        return 0;
    }
    (24.0 * (hertz / 20.0).log2()).round() as i32
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_key_has_its_own_id_and_reads_back() {
        let effects: Vec<EffectKey> = EffectKey::all().collect();
        assert_eq!(effects.len(), 33);
        let ids: BTreeSet<u32> = effects.iter().map(|key| key.id()).collect();
        assert_eq!(ids.len(), effects.len());
        for key in effects {
            assert_eq!(EffectKey::from_id(key.id()), Some(key));
        }
        // The microphone input mute and the voice effects are not handled here.
        assert_eq!(EffectKey::from_id(0x0158), None);
        assert_eq!(EffectKey::from_id(0x0076), None);

        for key in MicParamKey::ALL {
            assert_eq!(MicParamKey::from_id(key.id()), Some(key));
        }
        assert_eq!(MicParamKey::from_id(0), None);
    }

    #[test]
    fn equaliser_bands_are_numbered_like_the_device() {
        assert_eq!(EffectKey::EqFrequency(EqBand::Hz31).id(), 0x0126);
        assert_eq!(EffectKey::EqGain(EqBand::Hz31).id(), 0x0127);
        assert_eq!(EffectKey::EqFrequency(EqBand::Hz63).id(), 0x00f8);
        assert_eq!(EffectKey::EqGain(EqBand::Khz8).id(), 0x010a);
        assert_eq!(EffectKey::EqFrequency(EqBand::Khz16).id(), 0x012f);
    }

    #[test]
    fn gate_attenuation_follows_the_uneven_scale_of_the_device() {
        assert_eq!(gate_attenuation_db(0), -6);
        assert_eq!(gate_attenuation_db(50), -18);
        assert_eq!(gate_attenuation_db(96), -30);
        assert_eq!(gate_attenuation_db(99), -30);
        assert_eq!(gate_attenuation_db(100), -61);
        assert_eq!(gate_attenuation_db(255), -61);
    }

    #[test]
    fn frequencies_are_counted_in_twenty_fourths_of_an_octave() {
        assert_eq!(eq_frequency_value(20.0), 0);
        assert_eq!(eq_frequency_value(31.5), 16);
        assert_eq!(eq_frequency_value(1000.0), 135);
        assert_eq!(eq_frequency_value(16000.0), 231);
        assert_eq!(eq_frequency_value(5.0), 0);
        assert_eq!(eq_frequency_value(f32::NAN), 0);
    }

    #[test]
    fn times_and_ratios_only_grow() {
        assert!(GATE_TIMES_MS.is_sorted());
        assert!(COMPRESSOR_ATTACK_MS.is_sorted());
        assert!(COMPRESSOR_RELEASE_MS.is_sorted());
        assert!(COMPRESSOR_RATIOS.is_sorted());
        assert_eq!((GATE_TIMES_MS[0], GATE_TIMES_MS[45]), (10, 2000));
        assert_eq!(GATE_TIMES_MS[19], 200);
        assert_eq!(COMPRESSOR_RATIOS[9], 4.0);
    }
}
