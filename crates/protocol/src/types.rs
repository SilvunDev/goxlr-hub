//! The names the protocol gives to the parts of the device.

/// Declares a field-less enum with an `ALL` list in declaration order.
macro_rules! listed_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident $(= $value:expr)?),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(rename_all = "camelCase")
        )]
        pub enum $name { $($variant $(= $value)?),+ }

        impl $name {
            pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];
        }
    };
}

listed_enum! {
    /// An audio channel with a volume. The order is the device's numbering.
    Channel { Mic, LineIn, Console, System, Game, Chat, Sample, Music, Headphones, MicMonitor, LineOut }
}

impl Channel {
    pub const COUNT: usize = Self::ALL.len();

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(usize::from(index)).copied()
    }
}

listed_enum! {
    /// One of the four motorised faders, left to right.
    Fader { A, B, C, D }
}

impl Fader {
    pub const COUNT: usize = Self::ALL.len();

    pub fn index(self) -> u8 {
        self as u8
    }

    pub fn from_index(index: u8) -> Option<Self> {
        Self::ALL.get(usize::from(index)).copied()
    }
}

listed_enum! {
    /// A physical button. The value is its bit in the status word.
    Button {
        EffectSelect1 = 0,
        EffectSelect5 = 1,
        SamplerSelectA = 2,
        SamplerTopLeft = 3,
        Fader1Mute = 4,
        EffectSelect2 = 5,
        EffectSelect6 = 6,
        SamplerSelectB = 7,
        SamplerTopRight = 8,
        Fader2Mute = 9,
        EffectSelect3 = 10,
        EffectRobot = 11,
        SamplerSelectC = 12,
        SamplerBottomRight = 13,
        Fader3Mute = 14,
        EffectSelect4 = 15,
        EffectHardTune = 16,
        SamplerBottomLeft = 17,
        SamplerClear = 18,
        Fader4Mute = 19,
        EffectMegaphone = 20,
        EffectFx = 21,
        Bleep = 22,
        MicMute = 23,
    }
}

impl Button {
    pub fn bit(self) -> u8 {
        self as u8
    }
}

/// The buttons held down at one moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ButtonSet(u32);

impl ButtonSet {
    const MASK: u32 = (1 << Button::ALL.len()) - 1;

    /// Bits that match no button are dropped.
    pub fn from_bits(bits: u32) -> Self {
        Self(bits & Self::MASK)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    pub fn contains(self, button: Button) -> bool {
        self.0 & (1 << button.bit()) != 0
    }

    pub fn insert(&mut self, button: Button) {
        self.0 |= 1 << button.bit();
    }

    pub fn remove(&mut self, button: Button) {
        self.0 &= !(1 << button.bit());
    }

    pub fn iter(self) -> impl Iterator<Item = Button> {
        Button::ALL
            .into_iter()
            .filter(move |button| self.contains(*button))
    }
}

/// How a button is lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonLight {
    Lit = 0x01,
    Dimmed = 0x02,
}

/// The light of every button, sent as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonLights([ButtonLight; Self::LEN]);

impl Default for ButtonLights {
    fn default() -> Self {
        Self([ButtonLight::Dimmed; Self::LEN])
    }
}

impl ButtonLights {
    /// One byte per button, at the position of its bit in the status word.
    pub const LEN: usize = Button::ALL.len();

    pub fn get(&self, button: Button) -> ButtonLight {
        self.0[usize::from(button.bit())]
    }

    pub fn set(&mut self, button: Button, light: ButtonLight) {
        self.0[usize::from(button.bit())] = light;
    }

    pub fn bytes(&self) -> [u8; Self::LEN] {
        self.0.map(|light| light as u8)
    }

    /// `None` when the size is wrong or a byte is no light this app sends.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let mut lights = Self::default();
        if bytes.len() != Self::LEN {
            return None;
        }
        for (light, byte) in lights.0.iter_mut().zip(bytes) {
            *light = match byte {
                0x01 => ButtonLight::Lit,
                0x02 => ButtonLight::Dimmed,
                _ => return None,
            };
        }
        Some(lights)
    }
}

/// Stereo side of a routing row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub const BOTH: [Self; 2] = [Self::Left, Self::Right];
}

listed_enum! {
    /// A source of the routing table.
    RoutingInput { Mic, Chat, Music, Game, Console, LineIn, System, Samples }
}

impl RoutingInput {
    pub const COUNT: usize = Self::ALL.len();

    /// The device numbers each side of an input separately.
    pub fn id(self, side: Side) -> u8 {
        let left = match self {
            Self::Mic => 0x02,
            Self::LineIn => 0x04,
            Self::Console => 0x06,
            Self::System => 0x08,
            Self::Game => 0x0a,
            Self::Chat => 0x0c,
            Self::Music => 0x0e,
            Self::Samples => 0x10,
        };
        match side {
            Side::Left => left,
            Side::Right => left + 1,
        }
    }

    pub fn from_id(id: u8) -> Option<(Self, Side)> {
        Self::ALL.into_iter().find_map(|input| {
            Side::BOTH
                .into_iter()
                .find(|side| input.id(*side) == id)
                .map(|side| (input, side))
        })
    }
}

listed_enum! {
    /// A destination of the routing table.
    RoutingOutput { Headphones, BroadcastMix, ChatMic, Sampler, LineOut }
}

impl RoutingOutput {
    /// Where this output sits in the body of a routing command.
    pub fn position(self, side: Side) -> usize {
        let left = match self {
            Self::Headphones => 1,
            Self::BroadcastMix => 5,
            Self::ChatMic => 9,
            Self::Sampler => 13,
            Self::LineOut => 17,
        };
        match side {
            Side::Left => left,
            Side::Right => left + 2,
        }
    }
}

/// The outputs one input is sent to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutputSet(u8);

impl OutputSet {
    pub fn of(outputs: &[RoutingOutput]) -> Self {
        let mut set = Self::default();
        for output in outputs {
            set.insert(*output);
        }
        set
    }

    pub fn contains(self, output: RoutingOutput) -> bool {
        self.0 & (1 << output as u8) != 0
    }

    pub fn insert(&mut self, output: RoutingOutput) {
        self.0 |= 1 << output as u8;
    }

    pub fn remove(&mut self, output: RoutingOutput) {
        self.0 &= !(1 << output as u8);
    }

    /// The same set, with one output put in or taken out.
    pub fn with(mut self, output: RoutingOutput, on: bool) -> Self {
        if on {
            self.insert(output);
        } else {
            self.remove(output);
        }
        self
    }

    pub fn iter(self) -> impl Iterator<Item = RoutingOutput> {
        RoutingOutput::ALL
            .into_iter()
            .filter(move |output| self.contains(*output))
    }
}

listed_enum! {
    /// How the microphone is plugged in. Only a condenser gets phantom power.
    MicType { Dynamic, Condenser, Jack }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channels_are_numbered_like_the_device() {
        assert_eq!(Channel::Mic.index(), 0);
        assert_eq!(Channel::Chat.index(), 5);
        assert_eq!(Channel::LineOut.index(), 10);
        assert_eq!(Channel::from_index(7), Some(Channel::Music));
        assert_eq!(Channel::from_index(11), None);
        assert_eq!(Fader::from_index(3), Some(Fader::D));
        assert_eq!(Fader::from_index(4), None);
    }

    #[test]
    fn every_button_has_its_own_bit() {
        let mut seen = 0u32;
        for button in Button::ALL {
            assert_eq!(seen & (1 << button.bit()), 0, "{button:?}");
            seen |= 1 << button.bit();
        }
        assert_eq!(seen, 0x00ff_ffff);
        assert_eq!(Button::Fader1Mute.bit(), 4);
        assert_eq!(Button::Fader4Mute.bit(), 19);
        assert_eq!(Button::MicMute.bit(), 23);
    }

    #[test]
    fn button_sets_ignore_bits_without_a_button() {
        let set = ButtonSet::from_bits(0xff00_0010);
        assert_eq!(set.bits(), 0x10);
        assert_eq!(set.iter().collect::<Vec<_>>(), [Button::Fader1Mute]);

        let mut set = ButtonSet::default();
        set.insert(Button::Bleep);
        assert!(set.contains(Button::Bleep));
        set.remove(Button::Bleep);
        assert_eq!(set, ButtonSet::default());
    }

    #[test]
    fn button_lights_sit_at_the_bit_of_their_button() {
        let mut lights = ButtonLights::default();
        assert_eq!(lights.bytes(), [0x02; 24]);
        lights.set(Button::Fader2Mute, ButtonLight::Lit);
        lights.set(Button::MicMute, ButtonLight::Lit);
        let bytes = lights.bytes();
        assert_eq!((bytes[9], bytes[23], bytes[4]), (0x01, 0x01, 0x02));
        assert_eq!(lights.get(Button::Fader2Mute), ButtonLight::Lit);
        assert_eq!(ButtonLights::from_bytes(&bytes), Some(lights));
        assert_eq!(ButtonLights::from_bytes(&bytes[..23]), None);
        assert_eq!(ButtonLights::from_bytes(&[0x03; 24]), None);
    }

    #[test]
    fn routing_inputs_have_one_id_per_side() {
        assert_eq!(RoutingInput::Mic.id(Side::Left), 0x02);
        assert_eq!(RoutingInput::Mic.id(Side::Right), 0x03);
        assert_eq!(RoutingInput::Samples.id(Side::Right), 0x11);
        for input in RoutingInput::ALL {
            for side in Side::BOTH {
                assert_eq!(RoutingInput::from_id(input.id(side)), Some((input, side)));
            }
        }
        assert_eq!(RoutingInput::from_id(0x01), None);
        assert_eq!(RoutingInput::from_id(0x12), None);
    }

    #[test]
    fn an_output_leaves_a_set_without_taking_the_others() {
        let mut set = OutputSet::of(&[RoutingOutput::Headphones, RoutingOutput::LineOut]);
        set.remove(RoutingOutput::Headphones);
        set.remove(RoutingOutput::Sampler);
        assert_eq!(set, OutputSet::of(&[RoutingOutput::LineOut]));

        let set = set.with(RoutingOutput::ChatMic, true);
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [RoutingOutput::ChatMic, RoutingOutput::LineOut]
        );
        assert_eq!(set.with(RoutingOutput::ChatMic, true), set);
        let set = set
            .with(RoutingOutput::ChatMic, false)
            .with(RoutingOutput::LineOut, false);
        assert_eq!(set, OutputSet::default());
    }

    #[test]
    fn routing_outputs_sit_at_their_positions() {
        assert_eq!(RoutingOutput::Headphones.position(Side::Left), 1);
        assert_eq!(RoutingOutput::Headphones.position(Side::Right), 3);
        assert_eq!(RoutingOutput::LineOut.position(Side::Right), 19);
    }
}
