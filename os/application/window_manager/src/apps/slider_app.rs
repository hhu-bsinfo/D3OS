use alloc::{boxed::Box, rc::Rc, string::{String, ToString}};
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::{Bitmap, ScalingMode}, color::Color};
use spin::RwLock;
use crate::{api::Command, components::component::ComponentStylingBuilder, signal::{ComponentRef, Signal, Stateful}, WindowManager};

use super::runnable::Runnable;

pub struct SliderApp;

impl Runnable for SliderApp {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let initial_value = 1;
        let last_value: Stateful::<i32> = Signal::new(initial_value);

        let label = Signal::new(initial_value.to_string());
        let label_slider = Rc::clone(&label);

        let bitmap: Rc<RwLock<Option<ComponentRef>>> = Rc::new(RwLock::new(None));

        let gradient_bitmap = Bitmap {
            width: 20,
            height: 10,
            data: (0..200) // 20 * 10 Pixel
                .map(|i| {
                    let intensity = ((i % 20) as f32 / 19.0 * 255.0) as u8;
                    Color { red: intensity, green: intensity, blue: 255, alpha: 255 } // Blau zu Weiß
                })
                .collect(),
        };

        let _label_value = api.execute(
            handle,
            Command::CreateLabel {
                log_pos: Vertex::new(50, 100),
                text: label,
                on_loop_iter: None,
                font_size: Some(2),
                styling: None,
            },
        ).unwrap();

        let bitmap_slider = Rc::clone(&bitmap);
        let _slider = api.execute(
            handle,
            Command::CreateSlider {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 150),
                    width: 200,
                    height: 50,
                },
                on_change: Some(Box::new(move |value| {
                    label_slider.set(value.to_string());
                    
                    if let Some(bitmap) = bitmap_slider.read().as_ref() {
                        if let Some(resizeable) = bitmap.write().as_resizable_mut() {
                            let factor = value as f64 / last_value.get() as f64;
                            resizeable.rescale(factor);
                            last_value.set(value);
                        }
                    }
                })),
                value: initial_value,
                min: 1,
                max: 3,
                steps: 1,
                styling: None,
            }
        );

        let bitmap_init = Rc::clone(&bitmap);
        *bitmap_init.write() = Some(api.execute(
            handle,
            Command::CreateBitmapGraphic {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 350),
                    width: 50,
                    height: 25,
                },
                bitmap: &gradient_bitmap,
                scaling_mode: ScalingMode::Bilinear,
                styling: Some(ComponentStylingBuilder::new()
                    .maintain_aspect_ratio(true)
                    .build()
                ),
            },
        ).unwrap());
    }
}
