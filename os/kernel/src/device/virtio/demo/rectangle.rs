/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: rectangle.rs                                               ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Animated rectangle demo rendered on the Virtio GPU framebuffer.         ║
   ║                                                                         ║
   ║ Responsibilities:                                                       ║
   ║   - Spawn 20+ moving colored rectangles                                 ║
   ║   - Handle bouncing and direction reversal at screen edges              ║
   ║   - Log real-time FPS and render time statistics                        ║
   ║                                                                         ║
   ║ Author: Nikita E., Univ. Duesseldorf, 2025                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec::Vec;
use core::slice;
use log::info;
use super::renderer::Graphics;
use crate::syscall::sys_concurrent::sys_thread_sleep;
use crate::syscall::sys_time::sys_get_system_time;

use super::super::hal::HalImpl;
use virtio::transport::pci::PciTransport;
use virtio::device::gpu::VirtIOGpu;

use spin::Mutex;
use x86_64::instructions::interrupts;

use core::sync::atomic::Ordering;
use super::super::GPU_CONFIG_PENDING;

struct MovingRect {
    /// X coordinate of the top-left corner (floating point for smooth movement).
    x: f32,
    /// Y coordinate of the top-left corner.
    y: f32,
    /// Width of the rectangle in pixels.
    width: usize,
    /// Height of the rectangle in pixels.
    height: usize,
    /// Horizontal velocity in pixels per second.
    vx: f32,
    /// Vertical velocity in pixels per second.
    vy: f32,
    /// RGBA color of the rectangle.
    color: [u8; 4],
}

/// Starts the rectangle animation demo on the Virtio GPU framebuffer.
pub fn rectangle_demo(gpu_mutex: &Mutex<VirtIOGpu<HalImpl, PciTransport>>) {
    
    // 1. Initial Setup: Retrieve resolution and framebuffer pointer safely.
    // We make these variables mutable so we can update them on resize events.
    let (mut fb_ptr, mut fb_len, mut screen_width, mut screen_height) = interrupts::without_interrupts(|| {
        let mut gpu = gpu_mutex.lock();
        let (w, h) = gpu.resolution().unwrap();
        let buf = gpu.setup_framebuffer().unwrap();
        (buf.as_mut_ptr(), buf.len(), w as usize, h as usize)
    });


    // Rectangle dimensions
    const RECT_W: usize = 50;
    const RECT_H: usize = 30;

    // Predefined array of 20 distinct RGBA colors.
    let colors: [[u8; 4]; 20] = [
        [255,   0,   0, 255], [  0, 255,   0, 255], [  0,   0, 255, 255], [255, 255,   0, 255],
        [255,   0, 255, 255], [  0, 255, 255, 255], [255, 165,   0, 255], [128,   0, 128, 255],
        [255, 192, 203, 255], [  0, 128, 128, 255], [  0,   0, 128, 255], [128,   0,   0, 255],
        [128, 128,   0, 255], [192, 192, 192, 255], [128, 128, 128, 255], [165,  42,  42, 255],
        [255, 215,   0, 255], [250, 128, 114, 255], [ 64, 224, 208, 255], [238, 130, 238, 255],
    ];

    // Create a vector of MovingRect instances
    let mut rects: Vec<MovingRect> = {
        let mut vec = Vec::with_capacity(500);
        for i in 0..500 {
            let px = ((i * 37) % (screen_width - RECT_W)) as f32;
            let py = ((i * 53) % (screen_height - RECT_H)) as f32;
            let base_vx = ((i % 5) as f32 + 1.0) * 100.0;
            let base_vy = (((i / 5) as f32 + 1.0) * 120.0).min(400.0);
            let vx = if i % 2 == 0 { base_vx } else { -base_vx };
            let vy = if i % 3 == 0 { base_vy } else { -base_vy };
            let color = colors[i % colors.len()];

            vec.push(MovingRect { x: px, y: py, width: RECT_W, height: RECT_H, vx, vy, color });
        }
        vec
    };

    // Frame timing config for ~60 FPS
    let frame_time_ms = 16.67;
    let delta_time = frame_time_ms / 1000.0;

    // Performance tracking
    const MAX_SAMPLES: usize = 1000;
    let mut frame_times_ms: [isize; MAX_SAMPLES] = [0; MAX_SAMPLES];
    let mut sample_index = 0;
    let mut samples_collected: usize = 0;
    let mut last_log_time = sys_get_system_time();

    loop {
        // Handle Resizing
        if GPU_CONFIG_PENDING.swap(false, Ordering::Acquire) {
            info!("Resize event detected! Updating framebuffer...");
            
            // Re-acquire hardware resources safely
            interrupts::without_interrupts(|| {
                let mut gpu = gpu_mutex.lock();
                
                // Get new resolution
                let (w, h) = gpu.resolution().unwrap(); 
                screen_width = w as usize;
                screen_height = h as usize;
                
                
                // Get new framebuffer pointer (may change after resize)
                let buf = gpu.setup_framebuffer().unwrap(); 
                fb_ptr = buf.as_mut_ptr(); 
                fb_len = buf.len();
         
            });
            info!("New resolution: {}x{}", screen_width, screen_height);
        }

        // Create Graphics wrapper for the current frame
        // We do this every loop iteration because 'fb_ptr' might have changed.
        // This is cheap as it creates a temporary slice wrapper.
        let stride = screen_width * 4;
        let fb_slice = unsafe { slice::from_raw_parts_mut(fb_ptr, fb_len) };
        let mut gfx = Graphics::new(fb_slice, screen_width, screen_height, stride);


        let frame_start = sys_get_system_time();

        // Clear screen
        gfx.clear_screen([0, 0, 0, 255]);

        // Update rectangles
        for rect in rects.iter_mut() {
            rect.x += rect.vx * delta_time;
            rect.y += rect.vy * delta_time;

            // Bounce logic
            // Note: If window shrank, rect might be outside. This logic automatically pushes it back in.
            if rect.x <= 0.0 {
                rect.x = 0.0;
                rect.vx = rect.vx.abs();
            } else if rect.x + rect.width as f32 >= screen_width as f32 {
                rect.x = (screen_width.saturating_sub(rect.width)) as f32; // Safe sub
                rect.vx = -rect.vx.abs();
            }

            if rect.y <= 0.0 {
                rect.y = 0.0;
                rect.vy = rect.vy.abs();
            } else if rect.y + rect.height as f32 >= screen_height as f32 {
                rect.y = (screen_height.saturating_sub(rect.height)) as f32; // Safe sub
                rect.vy = -rect.vy.abs();
            }

            gfx.fill_rect(rect.x as usize, rect.y as usize, rect.width, rect.height, rect.color);
        }

        let render_start = sys_get_system_time();

        // {
        //     let mut gpu = gpu_mutex.lock();
        //     gpu.flush().expect("GPU flush failed");
        // }

        // Flush to GPU (Protected against interrupt storms)
        {
            interrupts::without_interrupts(|| {
                let mut gpu = gpu_mutex.lock();
                gpu.flush().expect("GPU flush failed");
            });
        }

        let render_end = sys_get_system_time();
        let render_time = render_end - render_start;

        // Save render time for performance metrics
        frame_times_ms[sample_index] = render_time;
        sample_index = (sample_index + 1) % MAX_SAMPLES;
        samples_collected = samples_collected.saturating_add(1).min(MAX_SAMPLES);

        // Every 30 seconds, log performance stats (FPS, ⌀, min, max, 95th percentile)
        let now = sys_get_system_time();
        if now - last_log_time >= 30_000 {
            let mut sorted = frame_times_ms[..samples_collected].to_vec();
            sorted.sort_unstable();

            let avg_render_time = sorted.iter().sum::<isize>() as f64 / samples_collected as f64;
            let fps = 1000.0 / avg_render_time;
            let min_render = *sorted.first().unwrap_or(&0);
            let max_render = *sorted.last().unwrap_or(&0);
            let p95_render = sorted[(samples_collected * 95 / 100).min(samples_collected - 1)];

            info!(
                "[Perf] FPS: {:.2}, ⌀: {:.2} ms, min: {} ms, max: {} ms, 95%: {} ms",
                fps, avg_render_time, min_render, max_render, p95_render
            );

            last_log_time = now;
        }

        // Sleep to maintain stable frame rate (16.67 ms)
        let frame_end = sys_get_system_time();
        let elapsed = frame_end - frame_start;
        if elapsed < frame_time_ms as isize {
            sys_thread_sleep((frame_time_ms as isize - elapsed) as usize);
        }
    }
}