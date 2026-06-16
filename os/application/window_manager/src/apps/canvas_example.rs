use alloc::collections::vec_deque::VecDeque;
use alloc::{boxed::Box, rc::Rc, vec};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::color::Color;
use graphic::lfb::DEFAULT_CHAR_WIDTH;
use graphic::{
    bitmap::{Bitmap, ScalingMode},
    lfb::DEFAULT_CHAR_HEIGHT,
};
use spin::rwlock::RwLock;
use terminal::DecodedKey;

use crate::{api::Command, WindowManager};

use super::runnable::Runnable;

// Julius Drodofsky
pub struct CanvasApp;

fn render_input(key: DecodedKey, canvas: &Rc<RwLock<Bitmap>>, location: &Rc<RwLock<Vertex>>) {
    let mut position: Vertex;
    {
        position = *location.read();
    }

    let key = match key {
        DecodedKey::RawKey(s) => {
            return;
        }
        DecodedKey::Unicode(u) => u,
    };
    if key == '\n' {
        position.y += DEFAULT_CHAR_HEIGHT;
        position.x = 0;
        location.write().x = position.x;
        location.write().y = position.y;
        return;
    }
    if canvas.read().width - position.x < DEFAULT_CHAR_WIDTH {
        position.x = 0;
        position.y += DEFAULT_CHAR_HEIGHT;
    }
    position.x += canvas.write().draw_char_scaled(
        position.x,
        position.y,
        1,
        1,
        Color::new(255, 255, 255, 255),
        Color::new(0, 0, 0, 50),
        key,
    );
    location.write().x = position.x;
    location.write().y = position.y;
}

impl Runnable for CanvasApp {
    fn run() {
        //initialise values for component
        let bitmap_red = Bitmap {
            width: 200,
            height: 100,
            data: vec![
                Color { red: 255, green: 0, blue: 0, alpha: 255 }; // 10x10 rote Pixel
                20000 // 10 * 10
            ],
        };
        let deque = VecDeque::<DecodedKey>::new();
        let mut position_v = Vertex::zero();
        position_v.y = DEFAULT_CHAR_HEIGHT;
        let handle = concurrent::thread::current()
            .expect("Failed to get thread")
            .id();
        let api = WindowManager::get_api();
        let canvas = Rc::new(RwLock::new(bitmap_red));
        let position = Rc::new(RwLock::new(position_v));
        let input = Rc::new(RwLock::<VecDeque<DecodedKey>>::new(deque));
        //create component

        //use component
        let mut x = 0;
        x = canvas.write().draw_char_scaled(
            x,
            0,
            1,
            1,
            Color::new(255, 255, 255, 255),
            Color::new(0, 0, 0, 50),
            'R',
        );
        canvas.write().draw_char_scaled(
            x,
            0,
            1,
            1,
            Color::new(255, 255, 255, 255),
            Color::new(0, 0, 0, 50),
            'o',
        );
        canvas.write().draw_char_scaled(
            x * 2,
            0,
            1,
            1,
            Color::new(255, 255, 255, 255),
            Color::new(0, 0, 0, 50),
            't',
        );
        canvas.write().draw_line(
            0,
            DEFAULT_CHAR_HEIGHT,
            x * 3,
            DEFAULT_CHAR_HEIGHT,
            Color::new(255, 255, 255, 255),
        );
        let _component = api
            .execute(
                handle,
                None,
                Command::CreateCanvas {
                    styling: None,
                    log_rect_data: RectData {
                        top_left: Vertex::new(50, 80),
                        width: 200,
                        height: 100,
                    },
                    buffer: Rc::clone(&canvas),
                    input: Some(Box::new(move |c: DecodedKey| {
                        render_input(c, &canvas, &position);
                    })),
                    scaling_mode: ScalingMode::NearestNeighbor,
                },
            )
            .unwrap();
    }
}
