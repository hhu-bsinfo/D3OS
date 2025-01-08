use alloc::vec;
use drawer::{rect_data::RectData, vertex::Vertex};
use graphic::{bitmap::{Bitmap, ScalingMode}, color::Color};

use crate::{ api::Command, components::component::{ComponentStylingBuilder}, WindowManager};

use super::runnable::Runnable;

pub struct BitmapApp;

impl Runnable for BitmapApp {
    fn run() {
        let handle = concurrent::thread::current().id();
        let api = WindowManager::get_api();

        let bitmap_red = Bitmap {
            width: 10,
            height: 10,
            data: vec![
                Color { red: 255, green: 0, blue: 0, alpha: 255 }; // 10x10 rote Pixel
                100 // 10 * 10
            ],
        };

        let checkerboard_bitmap = Bitmap {
            width: 10,
            height: 10,
            data: (0..100) // 10 * 10 Pixel
                .map(|i| {
                    if (i / 10 + i % 10) % 2 == 0 {
                        Color { red: 255, green: 255, blue: 255, alpha: 255 } // Weiß
                    } else {
                        Color { red: 0, green: 0, blue: 0, alpha: 255 }       // Schwarz
                    }
                })
                .collect(),
        };

        let transparent_bitmap = Bitmap {
            width: 5,
            height: 5,
            data: vec![
                Color { red: 255, green: 0, blue: 0, alpha: 255 },   // Opaque Rot
                Color { red: 0, green: 255, blue: 0, alpha: 192 },  // Weniger transparent Grün
                Color { red: 0, green: 0, blue: 255, alpha: 128 },  // Halbtransparent Blau
                Color { red: 255, green: 255, blue: 0, alpha: 64 }, // Sehr transparent Gelb
                Color { red: 0, green: 0, blue: 0, alpha: 0 },      // Vollständig Transparent
        
                Color { red: 255, green: 0, blue: 0, alpha: 255 },
                Color { red: 0, green: 255, blue: 0, alpha: 192 },
                Color { red: 0, green: 0, blue: 255, alpha: 128 },
                Color { red: 255, green: 255, blue: 0, alpha: 64 },
                Color { red: 0, green: 0, blue: 0, alpha: 0 },
        
                Color { red: 255, green: 0, blue: 0, alpha: 255 },
                Color { red: 0, green: 255, blue: 0, alpha: 192 },
                Color { red: 0, green: 0, blue: 255, alpha: 128 },
                Color { red: 255, green: 255, blue: 0, alpha: 64 },
                Color { red: 0, green: 0, blue: 0, alpha: 0 },
        
                Color { red: 255, green: 0, blue: 0, alpha: 255 },
                Color { red: 0, green: 255, blue: 0, alpha: 192 },
                Color { red: 0, green: 0, blue: 255, alpha: 128 },
                Color { red: 255, green: 255, blue: 0, alpha: 64 },
                Color { red: 0, green: 0, blue: 0, alpha: 0 },
        
                Color { red: 255, green: 0, blue: 0, alpha: 255 },
                Color { red: 0, green: 255, blue: 0, alpha: 192 },
                Color { red: 0, green: 0, blue: 255, alpha: 128 },
                Color { red: 255, green: 255, blue: 0, alpha: 64 },
                Color { red: 0, green: 0, blue: 0, alpha: 0 },
            ],
        };

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
        
        
        let _bitmap_red_graphic = api.execute(
            handle,
            Command::CreateBitmapGraphic {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 50),
                    width: 50,
                    height: 50,
                },
                bitmap: &bitmap_red,
                scaling_mode: ScalingMode::NearestNeighbor,
                styling: None,
            },
        );

        let _checkerboard_bitmap_graphic = api.execute(
            handle,
            Command::CreateBitmapGraphic {
                log_rect_data: RectData {
                    top_left: Vertex::new(150, 50),
                    width: 50,
                    height: 50,
                },
                bitmap: &checkerboard_bitmap,
                scaling_mode: ScalingMode::NearestNeighbor,
                styling: None,
            },
        );

        let _transparent_bitmap_graphic = api.execute(
            handle,
            Command::CreateBitmapGraphic {
                log_rect_data: RectData {
                    top_left: Vertex::new(50, 150),
                    width: 50,
                    height: 50,
                },
                bitmap: &transparent_bitmap,
                scaling_mode: ScalingMode::NearestNeighbor,
                styling: Some(ComponentStylingBuilder::new()
                    .maintain_aspect_ratio(true)
                    .build()
                ),
            },
        );

        let _gradient_bitmap_graphic = api.execute(
            handle,
            Command::CreateBitmapGraphic {
                log_rect_data: RectData {
                    top_left: Vertex::new(150, 150),
                    width: 200,
                    height: 100,
                },
                bitmap: &gradient_bitmap,
                scaling_mode: ScalingMode::Bilinear,
                styling: Some(ComponentStylingBuilder::new()
                    .maintain_aspect_ratio(true)
                    .build()
                ),
            },
        );
    }
}