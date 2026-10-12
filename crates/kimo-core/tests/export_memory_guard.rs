//! Showcase finding: a 1080p export died silently on the 2 GB box. The guard's hard ceiling sat above physical RAM (budget*1.25+512 MB), so the
//! kernel OOM killer fired before kimo could refuse. The ceiling must stay under physical RAM, and export workers must scale with the memory budget.
use kimo_core::guard;
use std::sync::atomic::Ordering::Relaxed;
fn ram() -> usize { std::fs::read_to_string("/proc/meminfo").unwrap().lines().find(|l| l.starts_with("MemTotal")).and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse::<usize>().ok())).unwrap() * 1024 }
#[test] fn hard_ceiling_is_below_physical_ram_and_threads_follow_budget() {
    std::env::set_var("KIMO_MEM_MB", "1200"); guard::init();
    let hard = guard::HARD.load(Relaxed); assert!(hard <= ram() / 100 * 85, "hard ceiling {hard} must be <= 85% of RAM {}", ram());
    let old = 1200usize * (1 << 20); let old_hard = old + old / 4 + (512 << 20); assert!(old_hard > old, "sanity");
    assert_eq!(guard::export_threads(1920, 1080, 8), 3, "1200 MB budget / (33 MB x 12) = 3 workers at 1080p");
    assert_eq!(guard::export_threads(1280, 720, 8), 7, "720p needs less per frame");
    assert_eq!(guard::export_threads(3840, 2160, 8), 1, "4K at this budget must fall back to one worker");
    assert_eq!(guard::export_threads(1920, 1080, 2), 2, "never more workers than cpus");
}
