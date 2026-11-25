use pc_keyboard::layouts::{AnyLayout, De105Key, Us104Key};

/// Keyboard layouts available to the input decoder.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(usize)]
pub enum Layout {
    /// German QWERTZ (105 keys).
    #[default]
    Deutsch = 0,
    /// US English QWERTY (104 keys).
    English = 1,
    /// US QWERTY base mapping for a Pinyin input method.
    ///
    /// The caller receives Latin letters and handles Pinyin composition.
    ChinesePinyin = 2,
}

impl Layout {
    pub const fn from_id(id: usize) -> Option<Self> {
        match id {
            0 => Some(Self::Deutsch),
            1 => Some(Self::English),
            2 => Some(Self::ChinesePinyin),
            _ => None,
        }
    }

    /// Short label for system status displays.
    pub const fn status_label(self) -> &'static str {
        match self {
            Self::Deutsch => "KB DE",
            Self::English => "KB EN",
            Self::ChinesePinyin => "KB CN",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Deutsch => Self::English,
            Self::English => Self::ChinesePinyin,
            Self::ChinesePinyin => Self::Deutsch,
        }
    }

    pub(crate) const fn mapping(self) -> AnyLayout {
        match self {
            Self::English | Self::ChinesePinyin => AnyLayout::Us104Key(Us104Key),
            Self::Deutsch => AnyLayout::De105Key(De105Key),
        }
    }
}
