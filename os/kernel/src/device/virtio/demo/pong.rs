/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: pong.rs                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Interactive Pong demo rendered on the Virtio GPU framebuffer.           ║
   ║                                                                         ║
   ║ Responsibilities:                                                       ║
   ║   - Simulate Pong with basic AI and keyboard input                      ║
   ║   - Render game objects to the framebuffer                              ║
   ║   - Measure frame rendering performance                                 ║
   ║                                                                         ║
   ║ Author: Nikita E., Univ. Duesseldorf, 2025                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::slice;
use log::info;
use pc_keyboard::KeyState;
use stream::RawInputStream;
use super::renderer::Graphics;
use crate::syscall::sys_concurrent::sys_thread_sleep;
use crate::keyboard;
use crate::syscall::sys_time::sys_get_system_time;

use super::super::hal::HalImpl;
use virtio::transport::pci::PciTransport;
use virtio::device::gpu::VirtIOGpu;

use spin::Mutex;
use x86_64::instructions::interrupts;

/// Player paddle state.
pub struct Paddle {
    /// Top-left corner X position in pixels.
    pub x: usize,
    /// Top-left corner Y position in pixels.
    pub y: usize,
    /// Paddle width in pixels.
    pub width: usize,
    /// Paddle height in pixels.
    pub height: usize,
}

/// Ball state and velocity.
pub struct Ball {
    /// Center X coordinate in pixels (float for smooth movement).
    pub x: f32,
    /// Center Y coordinate in pixels.
    pub y: f32,
    /// Size (width and height) in pixels.
    pub size: usize,
    /// Horizontal velocity in pixels per second.
    pub vx: f32,
    /// Vertical velocity in pixels per second.
    pub vy: f32,
}

// PC scancodes for movement keys
const SCANCODE_W: u8 = 0x11;
const SCANCODE_S: u8 = 0x1F;
const SCANCODE_UP: u8 = 0x48;
const SCANCODE_DOWN: u8 = 0x50;

/// Holds the last keyboard scancode read. Unsafe global for simplicity.
static mut LAST_SCANCODE: Option<u8> = None;

/// Poll the keyboard controller and update `LAST_SCANCODE` if a key was pressed.
fn poll_keyboard() {
    if let Some(kb) = keyboard() {
        if let Some(event) = kb.read_event_nb() {
            if event.state != KeyState::Up {
                unsafe { LAST_SCANCODE = Some(event.code as u8); }
            }
        }
    }
}

/// Check if the specified scancode was the last key pressed.
fn is_key_pressed(scancode: u8) -> bool {
    unsafe { LAST_SCANCODE == Some(scancode) }
}

/// Handle user input for the left paddle (W/S keys).
fn handle_player_input(paddle: &mut Paddle, height: usize) {
    if is_key_pressed(SCANCODE_W) {
        paddle.y = paddle.y.saturating_sub(10);
    }
    if is_key_pressed(SCANCODE_S) {
        paddle.y = (paddle.y + 10).min(height - paddle.height);
    }
}

/// Update the game state: paddle positions, ball movement, collisions, and scoring.
fn update_game(
    paddle1: &mut Paddle,
    paddle2: &mut Paddle,
    ball: &mut Ball,
    width: usize,
    height: usize,
    delta_time: f32,
    score1: &mut usize,
    score2: &mut usize,
) {
    // Player paddle movement
    handle_player_input(paddle1, height);

    // Update ball position by velocity
    ball.x += ball.vx * delta_time;
    ball.y += ball.vy * delta_time;

    let ball_x = ball.x as usize;
    let ball_y = ball.y as usize;

    // Simple AI: move second paddle towards ball
    let ai_speed = 400.0;
    let target_center = paddle2.y as f32 + (paddle2.height as f32 / 2.0);
    if target_center < ball.y {
        paddle2.y = ((paddle2.y as f32) + ai_speed * delta_time)
            .min((height - paddle2.height) as f32) as usize;
    } else {
        paddle2.y = ((paddle2.y as f32) - ai_speed * delta_time)
            .max(0.0) as usize;
    }

    // Bounce off top/bottom edges
    if ball_y == 0 || ball_y + ball.size >= height {
        ball.vy = -ball.vy;
    }

    // Bounce off paddles
    if ball_x <= paddle1.x + paddle1.width
        && ball_y + ball.size >= paddle1.y
        && ball_y <= paddle1.y + paddle1.height
    {
        ball.vx = ball.vx.abs();
    }
    if ball_x + ball.size >= paddle2.x
        && ball_y + ball.size >= paddle2.y
        && ball_y <= paddle2.y + paddle2.height
    {
        ball.vx = -ball.vx.abs();
    }

    // Out of bounds: score and reset ball
    if ball_x == 0 {
        *score2 += 1;
        ball.x = (width / 2) as f32;
        ball.y = (height / 2) as f32;
        ball.vx = 800.0;
        ball.vy = 600.0;
    } else if ball_x + ball.size >= width {
        *score1 += 1;
        ball.x = (width / 2) as f32;
        ball.y = (height / 2) as f32;
        ball.vx = -800.0;
        ball.vy = 600.0;
    }
}

/// Render paddles, ball, center line, and scores to the framebuffer.
fn render_game(
    gfx: &mut Graphics,
    paddle1: &Paddle,
    paddle2: &Paddle,
    ball: &Ball,
    score1: usize,
    score2: usize,
) {
    // Clear screen to black
    gfx.clear_screen([0, 0, 0, 0xFF]);

    // Draw dashed center line
    for y in (0..gfx.height).step_by(20) {
        gfx.fill_rect(gfx.width / 2 - 1, y, 2, 10, [0xFF, 0xFF, 0xFF, 0xFF]);
    }

    // Draw ball and paddles as white rectangles
    gfx.fill_rect(ball.x as usize, ball.y as usize, ball.size, ball.size, [0xFF, 0xFF, 0xFF, 0xFF]);
    gfx.fill_rect(paddle1.x, paddle1.y, paddle1.width, paddle1.height, [0xFF, 0xFF, 0xFF, 0xFF]);
    gfx.fill_rect(paddle2.x, paddle2.y, paddle2.width, paddle2.height, [0xFF, 0xFF, 0xFF, 0xFF]);

    // Draw scores using 7-segment digits
    draw_digit(gfx, score1 % 10, gfx.width / 4 - 8, 10);
    draw_digit(gfx, score2 % 10, gfx.width * 3 / 4 - 8, 10);
}

/// Draw a single digit (0–9) at the given position using a 7-segment style.
fn draw_digit(gfx: &mut Graphics, digit: usize, x: usize, y: usize) {
    // Segment enable table for digits 0–9
    let segments = [
        [1, 1, 1, 0, 1, 1, 1], // 0
        [0, 0, 1, 0, 0, 1, 0], // 1
        [1, 0, 1, 1, 1, 0, 1], // 2
        [1, 0, 1, 1, 0, 1, 1], // 3
        [0, 1, 1, 1, 0, 1, 0], // 4
        [1, 1, 0, 1, 0, 1, 1], // 5
        [1, 1, 0, 1, 1, 1, 1], // 6
        [1, 0, 1, 0, 0, 1, 0], // 7
        [1, 1, 1, 1, 1, 1, 1], // 8
        [1, 1, 1, 1, 0, 1, 1], // 9
    ];
    let seg = segments[digit.min(9)];
    let white = [0xFF, 0xFF, 0xFF, 0xFF];

    // Each segment: (x_off, y_off, width, height)
    let segment_defs = [
        (2, 0, 12, 2),   // Top
        (0, 2, 2, 12),   // Top-left
        (14, 2, 2, 12),  // Top-right
        (2, 14, 12, 2),  // Middle
        (0, 16, 2, 12),  // Bottom-left
        (14, 16, 2, 12), // Bottom-right
        (2, 28, 12, 2),  // Bottom
    ];

    // Draw enabled segments
    for (i, &on) in seg.iter().enumerate() {
        if on == 1 {
            let (dx, dy, w, h) = segment_defs[i];
            gfx.fill_rect(x + dx, y + dy, w, h, white);
        }
    }
}

/// Entry point for the Pong demo. Blocks indefinitely running the game loop.
pub fn pong_demo(gpu_mutex: &Mutex<VirtIOGpu<HalImpl, PciTransport>>) {
    // Initialize framebuffer and resolution
    let (fb_ptr, fb_len, width, height) = {
        let mut gpu = gpu_mutex.lock();
        let (w, h) = gpu.resolution().unwrap();
        let buf = gpu.setup_framebuffer().unwrap();
        (buf.as_mut_ptr(), buf.len(), w as usize, h as usize)
    }; // Lock fällt hier
    let stride = width * 4; // 4 bytes per pixel (RGBA)
    let fb_slice = unsafe { slice::from_raw_parts_mut(fb_ptr, fb_len) };
    let mut gfx_fb = Graphics::new(fb_slice, width, height, stride);

    // Initialize game entities
    let mut paddle1 = Paddle { x: 20, y: height / 2 - 30, width: 10, height: 60 };
    let mut paddle2 = Paddle { x: width - 30, y: height / 2 - 30, width: 10, height: 60 };
    let mut ball = Ball {
        x: (width / 2) as f32,
        y: (height / 2) as f32,
        size: 10,
        vx: 800.0,
        vy: 600.0,
    };
    let mut score1 = 0;
    let mut score2 = 0;

    // Timing constants for ~60 FPS
    let frame_time_ms = 16.666;
    let delta_time = frame_time_ms / 1000.0;

    // Performance counters
    let mut frame_count = 0;
    let mut total_render_time_ms = 0isize;
    let mut last_log_time = sys_get_system_time();


    loop {
        let frame_start = sys_get_system_time();

        // Input and game update phase
        poll_keyboard(); // check for key press (W/S)
        update_game(&mut paddle1, &mut paddle2, &mut ball, width, height, delta_time, &mut score1, &mut score2);

        // Rendering phase
        let render_start = sys_get_system_time();
        render_game(&mut gfx_fb, &paddle1, &paddle2, &ball, score1, score2);

        {
            interrupts::without_interrupts(|| {
                let mut gpu = gpu_mutex.lock();
                gpu.flush().expect("GPU flush failed");
            });
        }
        let render_end = sys_get_system_time();

        // Stats update
        frame_count += 1;
        total_render_time_ms += render_end - render_start;

        let now = sys_get_system_time();
        if now - last_log_time >= 30_000 {
            // Log average FPS and render time every 30 seconds
            let elapsed_secs = (now - last_log_time) as f64 / 1000.0;
            let avg_fps = frame_count as f64 / elapsed_secs;
            let avg_render_ms = total_render_time_ms as f64 / frame_count as f64;
            info!(
                "[Performance] FPS: {:.2}, Durchschnittliche Renderzeit: {:.2} ms",
                avg_fps, avg_render_ms
            );
            frame_count = 0;
            total_render_time_ms = 0;
            last_log_time = now;
        }

        // Frame rate limiter: sleep if we finished too early
        let frame_end = sys_get_system_time();
        let elapsed = frame_end - frame_start;
        if elapsed < frame_time_ms as isize {
            sys_thread_sleep((frame_time_ms as isize - elapsed) as usize);
        }
    }
}

