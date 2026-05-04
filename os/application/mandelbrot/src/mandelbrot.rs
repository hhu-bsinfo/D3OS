#![no_std]
extern crate alloc;

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use concurrent::{core::current_core_id, thread};
use graphic::lfb::{FramebufferInfo, map_framebuffer};
use terminal::{print, println};
use ::time::systime;
use spin::Once;
#[allow(unused_imports)]
use runtime::*;

// ---- Mandelbrot Parameter ----
const WIDTH: u32 = 800;
const HEIGHT: u32 = 600;
const MAX_ITER: u32 = 1000;

// Complex Layer (classic Output)
const RE_MIN: f64 = -2.2;
const RE_MAX: f64 = 1.0;
const IM_MIN: f64 = -1.2;
const IM_MAX: f64 = 1.2;

// work-load per job, can adjust overhead
const ROWS_PER_JOB_DEFAULT: u32 = 16;

// number of workers (threads)
const WORKERS_DEFAULT: usize = 8;

// global job-queue
static NEXT_JOB: AtomicUsize = AtomicUsize::new(0);
static DONE_WORKERS: AtomicUsize = AtomicUsize::new(0);
static FB: Once<FramebufferInfo> = Once::new();
static CID_OUTPUT: AtomicBool = AtomicBool::new(false);
static ACTIVE_ROWS_PER_JOB: AtomicUsize = AtomicUsize::new(ROWS_PER_JOB_DEFAULT as usize);
static TOTAL_JOBS: AtomicUsize =
    AtomicUsize::new(((HEIGHT + ROWS_PER_JOB_DEFAULT - 1) / ROWS_PER_JOB_DEFAULT) as usize);

fn mandelbrot(c_re: f64, c_im: f64, max_iter: u32) -> u32 {
    let (mut z_re, mut z_im) = (0.0f64, 0.0f64);
    for i in 0..max_iter {
        let z_re2 = z_re * z_re - z_im * z_im + c_re;
        let z_im2 = 2.0 * z_re * z_im + c_im;
        z_re = z_re2;
        z_im = z_im2;
        if z_re * z_re + z_im * z_im > 4.0 {
            return i;
        }
    }
    max_iter
}

// returns u32 in the format 0x00RRGGBB
fn color(it: u32, max_iter: u32) -> u32 {
    if it >= max_iter {
        return 0x00000000; // black
    }
    let t = it as f64 / max_iter as f64;
    let r = (9.0  * (1.0 - t) * t * t * t * 255.0) as u32;
    let g = (15.0 * (1.0 - t) * (1.0 - t) * t * t * 255.0) as u32;
    let b = (8.5  * (1.0 - t) * (1.0 - t) * (1.0 - t) * t * 255.0) as u32;
    (r << 16) | (g << 8) | b
}

unsafe fn draw_row_block(fb: &FramebufferInfo, y0: u32, y1: u32) {
    let fb_base = fb.addr as *mut u8;

    // image at center (optional)
    let x_off = if fb.width > WIDTH { (fb.width - WIDTH) / 2 } else { 0 };
    let y_off = if fb.height > HEIGHT { (fb.height - HEIGHT) / 2 } else { 0 };

    for y in y0..y1 {
        let c_im = IM_MAX - (y as f64) * (IM_MAX - IM_MIN) / (HEIGHT as f64);

        // Pointer at start (centered)
        let row_ptr_u32 = unsafe {
            fb_base.add(((y_off + y) * fb.pitch + x_off * 4) as usize) as *mut u32 };

        for x in 0..WIDTH {
            let c_re = RE_MIN + (x as f64) * (RE_MAX - RE_MIN) / (WIDTH as f64);
            let it = mandelbrot(c_re, c_im, MAX_ITER);
            let px = color(it, MAX_ITER);
            unsafe { row_ptr_u32.add(x as usize).write(px) };
        }
    }
}

fn worker_thread() {
    let fb = FB.get().unwrap();
    let rows_per_job = ACTIVE_ROWS_PER_JOB.load(Ordering::Acquire) as u32;

    if CID_OUTPUT.load(Ordering::Acquire) {
        print!("{} ", current_core_id().unwrap());
    }

    loop {
        let job = NEXT_JOB.fetch_add(1, Ordering::AcqRel);
        if job >= TOTAL_JOBS.load(Ordering::Acquire) {
            break;
        }
        let y0 = (job as u32) * rows_per_job;
        let y1 = core::cmp::min(y0 + rows_per_job, HEIGHT);

        unsafe { draw_row_block(fb, y0, y1) };
    }
    //println!("Mandelbrot worker finished on core {}", current_core_id().unwrap());

    DONE_WORKERS.fetch_add(1, Ordering::Release);
}

#[unsafe(no_mangle)]
pub fn main() {

    let mut worker_cnt = WORKERS_DEFAULT;

    for arg in env::args().skip(1) {
        if let Some(v) = arg.strip_prefix("--workers=") {
            worker_cnt = v.parse().unwrap();
        } else if let Some(v) = arg.strip_prefix("--rows_per_job=") {
            ACTIVE_ROWS_PER_JOB.store(v.parse().unwrap(), Ordering::Release);
        } else if arg.contains("--log-cid") {
            CID_OUTPUT.store(true, Ordering::Release);
        } else {
            println!("Unknown argument: {}", arg);
            println!("Usage: mandelbrot [--workers=<n>] [--rows_per_job=<n>] [--log-cid]");
            return;
        }
    }
    let rows_per_job = ACTIVE_ROWS_PER_JOB.load(Ordering::Acquire) as u32;

    if rows_per_job < 1 || rows_per_job > HEIGHT {
        println!("rows_per_job must be greater than 0 and less than height ({})", HEIGHT);
        return;
    }
    if worker_cnt < 1 {
        println!("There must be at least one worker thread");
        return;
    }

    let cid_output = CID_OUTPUT.load(Ordering::Acquire);
    let total_jobs = ((HEIGHT + rows_per_job - 1) / rows_per_job) as usize;
    TOTAL_JOBS.store(total_jobs, Ordering::Release);

    let fb = map_framebuffer().expect("map_framebuffer failed");

    println!("Mandelbrot multicore demo starting...");

    FB.call_once(|| fb);

    NEXT_JOB.store(0, Ordering::Release);
    DONE_WORKERS.store(0, Ordering::Release);

    if cid_output {print!("Mandelbrot workers starting on cores: ")};

    let time = systime();   // note start time for later

    let mut last = thread::create(worker_thread).unwrap();
    for _ in 1..worker_cnt {
        last = thread::create(worker_thread).unwrap();
    }
    last.join();    // join last since we can only join one thread at a time

    // busy-wait until all workers finished
    while DONE_WORKERS.load(Ordering::Acquire) < worker_cnt {
        thread::sleep(100);
    }

    println!("\nMandelbrot done in {}s. (workers: {}, rows_per_job: {}, total_jobs: {})",
             (systime()-time).as_seconds_f32(), worker_cnt, rows_per_job, total_jobs);

}
