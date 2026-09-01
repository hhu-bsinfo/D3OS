/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: renderer.rs                                                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Basic software rendering for Virtio GPU backbuffer.                     ║
   ║                                                                         ║
   ║ Responsibilities:                                                       ║
   ║   - Fill rectangles with RGBA colors                                    ║
   ║   - Clear framebuffer to a solid color                                  ║
   ║   - Allocate pixel backbuffers for off-screen rendering                 ║
   ║                                                                         ║
   ║ Assumes 4 bytes per pixel (RGBA8 format).                               ║
   ║                                                                         ║
   ║ Author: Nikita E., Univ. Duesseldorf, 2025                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::boxed::Box;
use alloc::vec;

/// A 2D graphics context for rendering into an RGBA8 framebuffer.
///
/// Provides basic drawing methods on a linear pixel buffer (typically mapped to GPU memory).
pub struct Graphics<'a> {
    /// The raw pixel buffer in RGBA format (4 bytes per pixel).
    pub buf: &'a mut [u8],
    /// Width of the framebuffer in pixels.
    pub width: usize,
    /// Height of the framebuffer in pixels.
    pub height: usize,
    /// Number of bytes per scanline (usually width * 4).
    pub stride: usize,
}


impl<'a> Graphics<'a> {
    /// Create a new `Graphics` context.
    ///
    /// # Parameters
    /// - `buf`: mutable reference to the RGBA8 pixel data.
    /// - `width`: pixel width of the buffer.
    /// - `height`: pixel height of the buffer.
    /// - `stride`: number of bytes per row (e.g., `width * 4` for packed RGBA).
    ///
    /// # Returns
    /// A `Graphics` instance for drawing.
    pub fn new(buf: &'a mut [u8], width: usize, height: usize, stride: usize) -> Self {
        Self { buf, width, height, stride }
    }

    /// Fills a rectangle region on the framebuffer with a solid RGBA color.
    ///
    /// Pixels outside the framebuffer boundaries are clamped safely.
    /// This operation is software-only and iterates row by row.
    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: [u8; 4]) {
        // Iterate rows within [y, y+h) clamped to height
        for yy in y..(y + h).min(self.height) {
            // Compute start index of the row: yy * stride + x * bytes_per_pixel
            let row_start = yy * self.stride + x * 4;
            // Limit width to not exceed frame width
            let max_cols = w.min(self.width.saturating_sub(x));
            for col in 0..max_cols {
                // Each pixel is 4 bytes (RGBA)
                let base = row_start + col * 4;
                self.buf[base..base + 4].copy_from_slice(&color);
            }
        }
    }

    /// Clears the entire framebuffer to a solid RGBA color.
    ///
    /// Optimized using `chunks_exact_mut` to overwrite every pixel.
    pub fn clear_screen(&mut self, color: [u8; 4]) {
        for chunk in self.buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&color);
        }
    }

    /// Allocates a boxed backbuffer for off-screen rendering or double-buffering.
    ///
    /// Returns a zero-initialized pixel buffer sized to `stride * height` bytes.
    pub fn create_backbuffer(width: usize, height: usize, stride: usize) -> Box<[u8]> {
        let size = stride.checked_mul(height).expect("Backbuffer size overflow");
        vec![0u8; size].into_boxed_slice()
    }
}
