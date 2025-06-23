use crate::apps::text_editor::font::Font;
use crate::apps::text_editor::model::ViewConfig;
use graphic::color::{Color, WHITE};
use graphic::lfb::{DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_WIDTH};

#[derive(Debug, Clone, Copy)]
pub struct TextEditorConfig {
    pub width: usize,
    pub height: usize,
    pub background_color: Color,
    pub markdown_view: ViewConfig,
    pub simple_view: ViewConfig,
    pub code_view: ViewConfig,
}

impl TextEditorConfig {
    pub fn new(width: usize, height: usize) -> Self {
        let bg_color = Color::new(20, 20, 20, 255);
        let normal = Font {
            scale: 1,
            fg_color: WHITE,
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let strong = Font {
            scale: 1,
            fg_color: Color::new(69, 133, 136, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let emphasis = Font {
            scale: 1,
            fg_color: Color::new(131, 165, 152, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let number = Font {
            scale: 1,
            fg_color: Color::new(250, 189, 47, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let comment = Font {
            scale: 1,
            fg_color: Color::new(131, 165, 152, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let keyword = Font {
            scale: 1,
            fg_color: Color::new(142, 192, 124, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let string = Font {
            scale: 1,
            fg_color: Color::new(211, 134, 155, 255),
            bg_color: bg_color,
            char_width: DEFAULT_CHAR_WIDTH,
            char_height: DEFAULT_CHAR_HEIGHT,
        };
        let markdown_view = ViewConfig::Markdown {
            normal: normal,
            emphasis: emphasis,
            strong: strong,
        };

        let simple_view = ViewConfig::Simple {
            font_scale: normal.scale,
            fg_color: normal.fg_color,
            bg_color: normal.bg_color,
        };
        let code_view = ViewConfig::Code {
            normal: normal,
            keyword: keyword,
            string: string,
            number: number,
            comment: comment,
        };
        TextEditorConfig {
            width: width,
            height: height,
            background_color: bg_color,
            markdown_view: markdown_view,
            simple_view: simple_view,
            code_view: code_view,
        }
    }
}
