use crate::library::graphic::color;
use crate::library::graphic::color::Color;

pub const COLOR_TABLE_256: [Color;256] = [
    // 16 predefined colors, matching the 4-bit ANSI colors
    color::BLACK, color::RED, color::GREEN, color::YELLOW, color::BLUE, color::MAGENTA, color::CYAN, color::WHITE,
    color::BLACK.bright(), color::RED.bright(), color::GREEN.bright(), color::YELLOW.bright(), color::BLUE.bright(), color::MAGENTA.bright(), color::CYAN.bright(), color::WHITE.bright(),

    // 216 colors
    Color { red: 0, green: 0, blue: 0, alpha: 255 }, Color { red: 0, green: 0, blue: 95, alpha: 255 }, Color { red: 0, green: 0, blue: 135, alpha: 255 },
    Color { red: 0, green: 0, blue: 175, alpha: 255 }, Color { red: 0, green: 0, blue: 215, alpha: 255 }, Color { red: 0, green: 0, blue: 255, alpha: 255 },

    Color { red: 0, green: 95, blue: 0, alpha: 255 }, Color { red: 0, green: 95, blue: 95, alpha: 255 }, Color { red: 0, green: 95, blue: 135, alpha: 255 },
    Color { red: 0, green: 95, blue: 175, alpha: 255 }, Color { red: 0, green: 95, blue: 215, alpha: 255 }, Color { red: 0, green: 95, blue: 255, alpha: 255 },

    Color { red: 0, green: 135, blue: 0, alpha: 255 }, Color { red: 0, green: 135, blue: 95, alpha: 255 }, Color { red: 0, green: 135, blue: 135, alpha: 255 },
    Color { red: 0, green: 135, blue: 175, alpha: 255 }, Color { red: 0, green: 135, blue: 215, alpha: 255 }, Color { red: 0, green: 135, blue: 255, alpha: 255 },

    Color { red: 0, green: 175, blue: 0, alpha: 255 }, Color { red: 0, green: 175, blue: 95, alpha: 255 }, Color { red: 0, green: 175, blue: 135, alpha: 255 },
    Color { red: 0, green: 175, blue: 175, alpha: 255 }, Color { red: 0, green: 175, blue: 215, alpha: 255 }, Color { red: 0, green: 175, blue: 255, alpha: 255 },

    Color { red: 0, green: 215, blue: 0, alpha: 255 }, Color { red: 0, green: 215, blue: 95, alpha: 255 }, Color { red: 0, green: 215, blue: 135, alpha: 255 },
    Color { red: 0, green: 215, blue: 175, alpha: 255 }, Color { red: 0, green: 215, blue: 215, alpha: 255 }, Color { red: 0, green: 215, blue: 255, alpha: 255 },

    Color { red: 0, green: 255, blue: 0, alpha: 255 }, Color { red: 0, green: 255, blue: 95, alpha: 255 }, Color { red: 0, green: 255, blue: 135, alpha: 255 },
    Color { red: 0, green: 255, blue: 175, alpha: 255 }, Color { red: 0, green: 255, blue: 215, alpha: 255 }, Color { red: 0, green: 255, blue: 255, alpha: 255 },

    Color { red: 95, green: 0, blue: 0, alpha: 255 }, Color { red: 95, green: 0, blue: 95, alpha: 255 }, Color { red: 95, green: 0, blue: 135, alpha: 255 },
    Color { red: 95, green: 0, blue: 175, alpha: 255 }, Color { red: 95, green: 0, blue: 215, alpha: 255 }, Color { red: 95, green: 0, blue: 255, alpha: 255 },

    Color { red: 95, green: 95, blue: 0, alpha: 255 }, Color { red: 95, green: 95, blue: 95, alpha: 255 }, Color { red: 95, green: 95, blue: 135, alpha: 255 },
    Color { red: 95, green: 95, blue: 175, alpha: 255 }, Color { red: 95, green: 95, blue: 215, alpha: 255 }, Color { red: 95, green: 95, blue: 255, alpha: 255 },

    Color { red: 95, green: 135, blue: 0, alpha: 255 }, Color { red: 95, green: 135, blue: 95, alpha: 255 }, Color { red: 95, green: 135, blue: 135, alpha: 255 },
    Color { red: 95, green: 135, blue: 175, alpha: 255 }, Color { red: 95, green: 135, blue: 215, alpha: 255 }, Color { red: 95, green: 135, blue: 255, alpha: 255 },

    Color { red: 95, green: 175, blue: 0, alpha: 255 }, Color { red: 95, green: 175, blue: 95, alpha: 255 }, Color { red: 95, green: 175, blue: 135, alpha: 255 },
    Color { red: 95, green: 175, blue: 175, alpha: 255 }, Color { red: 95, green: 175, blue: 215, alpha: 255 }, Color { red: 95, green: 175, blue: 255, alpha: 255 },

    Color { red: 95, green: 215, blue: 0, alpha: 255 }, Color { red: 95, green: 215, blue: 95, alpha: 255 }, Color { red: 95, green: 215, blue: 135, alpha: 255 },
    Color { red: 95, green: 215, blue: 175, alpha: 255 }, Color { red: 95, green: 215, blue: 215, alpha: 255 }, Color { red: 95, green: 215, blue: 255, alpha: 255 },

    Color { red: 95, green: 255, blue: 0, alpha: 255 }, Color { red: 95, green: 255, blue: 95, alpha: 255 }, Color { red: 95, green: 255, blue: 135, alpha: 255 },
    Color { red: 95, green: 255, blue: 175, alpha: 255 }, Color { red: 95, green: 255, blue: 215, alpha: 255 }, Color { red: 95, green: 255, blue: 255, alpha: 255 },

    Color { red: 135, green: 0, blue: 0, alpha: 255 }, Color { red: 135, green: 0, blue: 95, alpha: 255 }, Color { red: 135, green: 0, blue: 135, alpha: 255 },
    Color { red: 135, green: 0, blue: 175, alpha: 255 }, Color { red: 135, green: 0, blue: 215, alpha: 255 }, Color { red: 135, green: 0, blue: 255, alpha: 255 },

    Color { red: 135, green: 95, blue: 0, alpha: 255 }, Color { red: 135, green: 95, blue: 95, alpha: 255 }, Color { red: 135, green: 95, blue: 135, alpha: 255 },
    Color { red: 135, green: 95, blue: 175, alpha: 255 }, Color { red: 135, green: 95, blue: 215, alpha: 255 }, Color { red: 135, green: 95, blue: 255, alpha: 255 },

    Color { red: 135, green: 135, blue: 0, alpha: 255 }, Color { red: 135, green: 135, blue: 95, alpha: 255 }, Color { red: 135, green: 135, blue: 135, alpha: 255 },
    Color { red: 135, green: 135, blue: 175, alpha: 255 }, Color { red: 135, green: 135, blue: 215, alpha: 255 }, Color { red: 135, green: 135, blue: 255, alpha: 255 },

    Color { red: 135, green: 175, blue: 0, alpha: 255 }, Color { red: 135, green: 175, blue: 95, alpha: 255 }, Color { red: 135, green: 175, blue: 135, alpha: 255 },
    Color { red: 135, green: 175, blue: 175, alpha: 255 }, Color { red: 135, green: 175, blue: 215, alpha: 255 }, Color { red: 135, green: 175, blue: 255, alpha: 255 },

    Color { red: 135, green: 215, blue: 0, alpha: 255 }, Color { red: 135, green: 215, blue: 95, alpha: 255 }, Color { red: 135, green: 215, blue: 135, alpha: 255 },
    Color { red: 135, green: 215, blue: 175, alpha: 255 }, Color { red: 135, green: 215, blue: 215, alpha: 255 }, Color { red: 135, green: 215, blue: 255, alpha: 255 },

    Color { red: 135, green: 255, blue: 0, alpha: 255 }, Color { red: 135, green: 255, blue: 95, alpha: 255 }, Color { red: 135, green: 255, blue: 135, alpha: 255 },
    Color { red: 135, green: 255, blue: 175, alpha: 255 }, Color { red: 135, green: 255, blue: 215, alpha: 255 }, Color { red: 135, green: 255, blue: 255, alpha: 255 },

    Color { red: 175, green: 0, blue: 0, alpha: 255 }, Color { red: 175, green: 0, blue: 95, alpha: 255 }, Color { red: 175, green: 0, blue: 135, alpha: 255 },
    Color { red: 175, green: 0, blue: 175, alpha: 255 }, Color { red: 175, green: 0, blue: 215, alpha: 255 }, Color { red: 175, green: 0, blue: 255, alpha: 255 },

    Color { red: 175, green: 95, blue: 0, alpha: 255 }, Color { red: 175, green: 95, blue: 95, alpha: 255 }, Color { red: 175, green: 95, blue: 135, alpha: 255 },
    Color { red: 175, green: 95, blue: 175, alpha: 255 }, Color { red: 175, green: 95, blue: 215, alpha: 255 }, Color { red: 175, green: 95, blue: 255, alpha: 255 },

    Color { red: 175, green: 135, blue: 0, alpha: 255 }, Color { red: 175, green: 135, blue: 95, alpha: 255 }, Color { red: 175, green: 135, blue: 135, alpha: 255 },
    Color { red: 175, green: 135, blue: 175, alpha: 255 }, Color { red: 175, green: 135, blue: 215, alpha: 255 }, Color { red: 175, green: 135, blue: 255, alpha: 255 },

    Color { red: 175, green: 175, blue: 0, alpha: 255 }, Color { red: 175, green: 175, blue: 95, alpha: 255 }, Color { red: 175, green: 175, blue: 135, alpha: 255 },
    Color { red: 175, green: 175, blue: 175, alpha: 255 }, Color { red: 175, green: 175, blue: 215, alpha: 255 }, Color { red: 175, green: 175, blue: 255, alpha: 255 },

    Color { red: 175, green: 215, blue: 0, alpha: 255 }, Color { red: 175, green: 215, blue: 95, alpha: 255 }, Color { red: 175, green: 215, blue: 135, alpha: 255 },
    Color { red: 175, green: 215, blue: 175, alpha: 255 }, Color { red: 175, green: 215, blue: 215, alpha: 255 }, Color { red: 175, green: 215, blue: 255, alpha: 255 },

    Color { red: 175, green: 255, blue: 0, alpha: 255 }, Color { red: 175, green: 255, blue: 95, alpha: 255 }, Color { red: 175, green: 255, blue: 135, alpha: 255 },
    Color { red: 175, green: 255, blue: 175, alpha: 255 }, Color { red: 175, green: 255, blue: 215, alpha: 255 }, Color { red: 175, green: 255, blue: 255, alpha: 255 },

    Color { red: 215, green: 0, blue: 0, alpha: 255 }, Color { red: 215, green: 0, blue: 95, alpha: 255 }, Color { red: 215, green: 0, blue: 135, alpha: 255 },
    Color { red: 215, green: 0, blue: 175, alpha: 255 }, Color { red: 215, green: 0, blue: 215, alpha: 255 }, Color { red: 215, green: 0, blue: 255, alpha: 255 },

    Color { red: 215, green: 95, blue: 0, alpha: 255 }, Color { red: 215, green: 95, blue: 95, alpha: 255 }, Color { red: 215, green: 95, blue: 135, alpha: 255 },
    Color { red: 215, green: 95, blue: 175, alpha: 255 }, Color { red: 215, green: 95, blue: 215, alpha: 255 }, Color { red: 215, green: 95, blue: 255, alpha: 255 },

    Color { red: 215, green: 135, blue: 0, alpha: 255 }, Color { red: 215, green: 135, blue: 95, alpha: 255 }, Color { red: 215, green: 135, blue: 135, alpha: 255 },
    Color { red: 215, green: 135, blue: 175, alpha: 255 }, Color { red: 215, green: 135, blue: 215, alpha: 255 }, Color { red: 215, green: 135, blue: 255, alpha: 255 },

    Color { red: 215, green: 175, blue: 0, alpha: 255 }, Color { red: 215, green: 175, blue: 95, alpha: 255 }, Color { red: 215, green: 175, blue: 135, alpha: 255 },
    Color { red: 215, green: 175, blue: 175, alpha: 255 }, Color { red: 215, green: 175, blue: 215, alpha: 255 }, Color { red: 215, green: 175, blue: 255, alpha: 255 },

    Color { red: 215, green: 215, blue: 0, alpha: 255 }, Color { red: 215, green: 215, blue: 95, alpha: 255 }, Color { red: 215, green: 215, blue: 135, alpha: 255 },
    Color { red: 215, green: 215, blue: 175, alpha: 255 }, Color { red: 215, green: 215, blue: 215, alpha: 255 }, Color { red: 215, green: 215, blue: 255, alpha: 255 },

    Color { red: 215, green: 255, blue: 0, alpha: 255 }, Color { red: 215, green: 255, blue: 95, alpha: 255 }, Color { red: 215, green: 255, blue: 135, alpha: 255 },
    Color { red: 215, green: 255, blue: 175, alpha: 255 }, Color { red: 215, green: 255, blue: 215, alpha: 255 }, Color { red: 215, green: 255, blue: 255, alpha: 255 },

    Color { red: 255, green: 0, blue: 0, alpha: 255 }, Color { red: 255, green: 0, blue: 95, alpha: 255 }, Color { red: 255, green: 0, blue: 135, alpha: 255 },
    Color { red: 255, green: 0, blue: 175, alpha: 255 }, Color { red: 255, green: 0, blue: 215, alpha: 255 }, Color { red: 255, green: 0, blue: 255, alpha: 255 },

    Color { red: 255, green: 95, blue: 0, alpha: 255 }, Color { red: 255, green: 95, blue: 95, alpha: 255 }, Color { red: 255, green: 95, blue: 135, alpha: 255 },
    Color { red: 255, green: 95, blue: 175, alpha: 255 }, Color { red: 255, green: 95, blue: 215, alpha: 255 }, Color { red: 255, green: 95, blue: 255, alpha: 255 },

    Color { red: 255, green: 135, blue: 0, alpha: 255 }, Color { red: 255, green: 135, blue: 95, alpha: 255 }, Color { red: 255, green: 135, blue: 135, alpha: 255 },
    Color { red: 255, green: 135, blue: 175, alpha: 255 }, Color { red: 255, green: 135, blue: 215, alpha: 255 }, Color { red: 255, green: 135, blue: 255, alpha: 255 },

    Color { red: 255, green: 175, blue: 0, alpha: 255 }, Color { red: 255, green: 175, blue: 95, alpha: 255 }, Color { red: 255, green: 175, blue: 135, alpha: 255 },
    Color { red: 255, green: 175, blue: 175, alpha: 255 }, Color { red: 255, green: 175, blue: 215, alpha: 255 }, Color { red: 255, green: 175, blue: 255, alpha: 255 },

    Color { red: 255, green: 215, blue: 0, alpha: 255 }, Color { red: 255, green: 215, blue: 95, alpha: 255 }, Color { red: 255, green: 215, blue: 135, alpha: 255 },
    Color { red: 255, green: 215, blue: 175, alpha: 255 }, Color { red: 255, green: 215, blue: 215, alpha: 255 }, Color { red: 255, green: 215, blue: 255, alpha: 255 },

    Color { red: 255, green: 255, blue: 0, alpha: 255 }, Color { red: 255, green: 255, blue: 95, alpha: 255 }, Color { red: 255, green: 255, blue: 135, alpha: 255 },
    Color { red: 255, green: 255, blue: 175, alpha: 255 }, Color { red: 255, green: 255, blue: 215, alpha: 255 }, Color { red: 255, green: 255, blue: 255, alpha: 255 },

    // 24 grayscale Colors
    Color { red: 8, green: 8, blue: 8, alpha: 255 }, Color { red: 18, green: 18, blue: 18, alpha: 255 }, Color { red: 28, green: 28, blue: 28, alpha: 255 },
    Color { red: 38, green: 38, blue: 38, alpha: 255 }, Color { red: 48, green: 48, blue: 48, alpha: 255 }, Color { red: 58, green: 58, blue: 58, alpha: 255 },
    Color { red: 68, green: 68, blue: 68, alpha: 255 }, Color { red: 78, green: 78, blue: 78, alpha: 255 }, Color { red: 88, green: 88, blue: 88, alpha: 255 },
    Color { red: 98, green: 98, blue: 98, alpha: 255 }, Color { red: 108, green: 108, blue: 108, alpha: 255 }, Color { red: 118, green: 118, blue: 118, alpha: 255 },
    Color { red: 128, green: 128, blue: 128, alpha: 255 }, Color { red: 138, green: 138, blue: 138, alpha: 255 }, Color { red: 148, green: 148, blue: 148, alpha: 255 },
    Color { red: 158, green: 158, blue: 158, alpha: 255 }, Color { red: 168, green: 168, blue: 168, alpha: 255 }, Color { red: 178, green: 178, blue: 178, alpha: 255 },
    Color { red: 188, green: 188, blue: 188, alpha: 255 }, Color { red: 198, green: 198, blue: 198, alpha: 255 }, Color { red: 208, green: 208, blue: 208, alpha: 255 },
    Color { red: 218, green: 218, blue: 218, alpha: 255 }, Color { red: 228, green: 228, blue: 228, alpha: 255 }, Color { red: 238, green: 238, blue: 238, alpha: 255 }
];