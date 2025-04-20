use drawer::{drawer::Drawer, rect_data::RectData};
use graphic::{bitmap::{Bitmap, ScalingMode}, lfb::DEFAULT_CHAR_HEIGHT};

use crate::{utils::scale_rect_to_window, WindowManager};

use super::component::{Casts, Component, ComponentStyling, Hideable, Resizable};

pub struct BitmapGraphic {
    pub id: Option<usize>,
    pub is_dirty: bool,
    rel_rect_data: RectData,
    abs_rect_data: RectData,
    orig_rect_data: RectData,
    drawn_rect_data: RectData,
    orig_bitmap: Bitmap,
    bitmap: Bitmap,
    scaling_mode: ScalingMode,
    scale_factor: f64,
    is_hidden: bool,
    styling: ComponentStyling,
}

impl BitmapGraphic {
    pub fn new(
        rel_rect_data: RectData,
        abs_rect_data: RectData,
        orig_rect_data: RectData,
        bitmap: Bitmap,
        scaling_mode: ScalingMode,
        styling: Option<ComponentStyling>,
    ) -> Self {
        Self {
            id: None,
            is_dirty: true,
            rel_rect_data,
            abs_rect_data,
            drawn_rect_data: abs_rect_data.clone(),
            orig_rect_data,
            scaling_mode,
            bitmap: bitmap.scale(abs_rect_data.width, abs_rect_data.height, scaling_mode.clone()),
            orig_bitmap: bitmap,
            scale_factor: 1.0,
            is_hidden: false,
            styling: styling.unwrap_or_default(),
        }
    }
}

impl Component for BitmapGraphic {
    fn draw(&mut self, focus_id: Option<usize>) {   
        if !self.is_dirty {
            return;
        }

        if self.is_hidden {
            self.is_dirty = false;
            return;
        }

        let styling = &self.styling;
        let is_focused = focus_id == self.id;

        let bg_color = if is_focused {
            styling.focused_background_color
        } else {
            styling.background_color
        };

        let border_color = if is_focused {
            styling.focused_border_color
        } else {
            styling.border_color
        };

        let text_color = {
            styling.text_color
        };

        Drawer::draw_bitmap(self.abs_rect_data.top_left, &self.bitmap);

        self.drawn_rect_data = self.abs_rect_data;

        if is_focused {
            // wegen eines bugs, dass unten und rechts noch eine border bei Fokusverlust zurück bleibt, wird border abgezogen
            Drawer::draw_rectangle(self.abs_rect_data.sub_border(), styling.focused_border_color);
        }

        self.is_dirty = false;
    }

    fn rescale_after_split(&mut self, old_window: RectData, new_window: RectData) {
        let styling: &ComponentStyling = &self.styling;

        self.abs_rect_data.top_left = self
            .abs_rect_data
            .top_left
            .move_to_new_rect(&old_window, &new_window);

        let min_dim = (DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_HEIGHT);

        let aspect_ratio = self.orig_rect_data.width as f64 / self.orig_rect_data.height as f64;

        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_window,
            min_dim,
            (self.orig_rect_data.width * self.scale_factor as u32, self.orig_rect_data.height * self.scale_factor as u32),
            styling.maintain_aspect_ratio,
            aspect_ratio,
        );

        self.bitmap = self.orig_bitmap.scale(self.abs_rect_data.width, self.abs_rect_data.height, self.scaling_mode);
        self.mark_dirty();
    }

    fn rescale_after_move(&mut self, new_rect_data: RectData) {
        let styling: &ComponentStyling = &self.styling;

        let min_dim = (DEFAULT_CHAR_HEIGHT, DEFAULT_CHAR_HEIGHT);
        let aspect_ratio = self.orig_rect_data.width as f64 / self.orig_rect_data.height as f64;
        
        self.abs_rect_data = scale_rect_to_window(
            self.rel_rect_data,
            new_rect_data,
            min_dim,
            (self.orig_rect_data.width * self.scale_factor as u32, self.orig_rect_data.height * self.scale_factor as u32),
            styling.maintain_aspect_ratio,
            aspect_ratio,
        );

        self.bitmap = self.orig_bitmap.scale(self.abs_rect_data.width, self.abs_rect_data.height, self.scaling_mode);
        self.mark_dirty();
    }

    fn get_abs_rect_data(&self) -> RectData {
        self.abs_rect_data
    }

    fn get_id(&self) -> Option<usize> {
        self.id
    }

    fn set_id(&mut self, id: usize) {
        self.id = Some(id);
    }

    fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    fn mark_dirty(&mut self) {
        self.is_dirty = true;
    }

    fn get_drawn_rect_data(&self) -> RectData {
        self.drawn_rect_data
    }
}

impl Casts for BitmapGraphic {
    fn as_hideable(&self) -> Option<&dyn Hideable> {
        Some(self)
    }

    fn as_hideable_mut(&mut self) -> Option<&mut dyn Hideable> {
        Some(self)
    }

    fn as_resizable(&self) -> Option<&dyn Resizable> {
        Some(self)
    }

    fn as_resizable_mut(&mut self) -> Option<&mut dyn Resizable> {
        Some(self)
    }
}

impl Hideable for BitmapGraphic {
    fn is_hidden(&self) -> bool {
        self.is_hidden
    }

    fn hide(&mut self) {
        self.is_hidden = true;
        self.mark_dirty();
    }

    fn show(&mut self) {
        self.is_hidden = false;
        self.mark_dirty();
    }
}

impl Resizable for BitmapGraphic {
    fn rescale(&mut self, scale_factor: f64) {
        self.scale_factor *= scale_factor;
        
        self.abs_rect_data.width = (f64::from(self.abs_rect_data.width) * scale_factor) as u32;
        self.abs_rect_data.height = (f64::from(self.abs_rect_data.height) * scale_factor) as u32;

        self.rel_rect_data.width = (f64::from(self.rel_rect_data.width) * scale_factor) as u32;
        self.rel_rect_data.height = (f64::from(self.rel_rect_data.height) * scale_factor) as u32;


        self.bitmap = self.orig_bitmap.scale(self.abs_rect_data.width, self.abs_rect_data.height, self.scaling_mode);
        self.mark_dirty();
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.scale_factor = 1.0;

        let scale_factor_x = width as f64 / self.abs_rect_data.width as f64;
        let scale_factor_y = height as f64 / self.abs_rect_data.height as f64;

        self.abs_rect_data.width = width;
        self.abs_rect_data.height = height;

        self.orig_rect_data.width = width;
        self.orig_rect_data.height = height;

        self.rel_rect_data.width = self.rel_rect_data.width * scale_factor_x as u32;
        self.rel_rect_data.height = self.rel_rect_data.height * scale_factor_y as u32;

        self.bitmap = self.orig_bitmap.scale(self.abs_rect_data.width, self.abs_rect_data.height, self.scaling_mode);
        self.mark_dirty();
    }
}