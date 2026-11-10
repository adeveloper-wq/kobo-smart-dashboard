#![allow(dead_code, unused_imports)]

mod backend;
mod config;
mod display;
mod frontlight;
mod i18n;
mod input;
mod models;
mod paths;
mod power;
mod ui;

use backend::{BackendClient, BackendSleepInfo, EntityState};
use clap::Parser;
use config::AppConfig;
use display::{create_framebuffer, Canvas, Refresh};
use input::{InputEvent, TouchDriver, TouchMapping};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use ui::layout::UiAction;
use ui::ScreenRenderer;

/// Log timestamps in local time (matching launch.sh)
struct LocalTime;

impl tracing_subscriber::fmt::time::FormatTime for LocalTime {
    fn format_time(&self, w: &mut tracing_subscriber::fmt::format::Writer<'_>) -> std::fmt::Result {
        write!(w, "{}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6f%:z"))
    }
}

#[derive(Parser, Debug)]
#[command(
    name = "kobo-ha-dashboard",
    version,
    about = "Dashboard for Kobo e-readers powered by Python backend"
)]
struct Args {
    /// Path to YAML configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Force hardware device mode using Linux framebuffer
    #[arg(short, long)]
    device: bool,

    /// Override orientation ("portrait" or "landscape")
    #[arg(long)]
    orientation: Option<String>,

    /// URL of the Python backend (e.g. "http://192.168.1.50:8000")
    #[arg(long)]
    backend: Option<String>,

    /// Render a single frame and exit
    #[arg(long)]
    once: bool,
}

/// Ignore taps arriving faster than this (e-ink ghost/bounce protection)
const TAP_DEBOUNCE: Duration = Duration::from_millis(350);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "kobo_ha_dashboard=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_ansi(false).with_timer(LocalTime))
        .init();

    let args = Args::parse();

    // Check candidate locations for config.yaml
    let requested = args.config.clone().unwrap_or_else(|| PathBuf::from("config.yaml"));
    let config_path = if requested.exists() {
        requested
    } else if Path::new("/mnt/onboard/.kobo-ha/config.yaml").exists() {
        PathBuf::from("/mnt/onboard/.kobo-ha/config.yaml")
    } else if Path::new("config.example.yaml").exists() {
        PathBuf::from("config.example.yaml")
    } else {
        requested
    };

    let mut config = AppConfig::load_or_default(&config_path);

    if let Some(orient) = args.orientation {
        config.device.orientation = orient;
    }

    if let Some(b_url) = args.backend {
        config.backend.url = b_url;
        config.backend.enabled = true;
    }

    // Set dashboard language
    let config_dir = config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    i18n::set_override_dir(Some(&config_dir.join("lang")));
    i18n::set_language(&config.theme.language);

    let backend_client = BackendClient::new(&config.backend.url);

    let is_linux_device = cfg!(target_os = "linux") && Path::new(&config.device.fb_device).exists();
    let run_as_device = args.device || is_linux_device;

    if !run_as_device {
        tracing::info!("Starting in console/test mode. Backend URL: {}", backend_client.url());
        match backend_client.fetch_dashboard().await {
            Ok(payload) => {
                let states = BackendClient::apply_to_app(&payload, &mut config);
                println!();
                println!("  Kobo Dashboard: Backend connection successful!");
                println!("  Backend: {}", backend_client.url());
                println!("  Status: {}", payload.status);
                println!("  Cards: {}", payload.cards.len());
                println!("  States generated: {}", states.len());
                if let Some(sleep) = &payload.sleep {
                    println!("  Sleep policy (from backend): enabled={}, wake in {}s", sleep.enabled, sleep.sleep_wake_secs);
                }
                println!();
            }
            Err(e) => {
                println!();
                println!("  Warning: Backend unreachable at {}: {}", backend_client.url(), e);
                println!();
            }
        }
        return Ok(());
    }

    #[cfg(unix)]
    return run_device(config, backend_client, args.once).await;
    #[cfg(not(unix))]
    return Err("Device mode is only available on Linux / Kobo".into());
}

#[cfg(unix)]
async fn fetch_and_update(
    backend: &BackendClient,
    config: &mut AppConfig,
    renderer: &mut ScreenRenderer,
) -> (HashMap<String, EntityState>, Option<BackendSleepInfo>) {
    match backend.fetch_dashboard().await {
        Ok(payload) => {
            if let Some(s) = &payload.sleep {
                renderer.sleep_until = s.sleep_note.clone();
            }
            renderer.last_update = Some(chrono::Utc::now());
            let states = BackendClient::apply_to_app(&payload, config);
            (states, payload.sleep)
        }
        Err(e) => {
            tracing::warn!("Backend fetch failed: {}", e);
            (HashMap::new(), None)
        }
    }
}

#[cfg(unix)]
async fn run_device(
    mut config: AppConfig,
    backend: BackendClient,
    once: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Starting on Kobo e-reader device");
    let probed = display::framebuffer::probe_resolution(&config.device.fb_device);
    let (width, height) = config.device.resolve_dimensions(probed);
    tracing::info!("Canvas {}x{} (framebuffer {:?}, model '{}')", width, height, probed, config.device.model);
    let canvas_rotation = config.device.canvas_rotation();
    let (portrait_w, portrait_h) = if canvas_rotation.swaps_axes() { (height, width) } else { (width, height) };

    let mut fb = create_framebuffer(
        &config.device.fb_device,
        width,
        height,
        canvas_rotation,
        config.device.fb_rotation(),
        config.device.color_panel,
    );
    let mut canvas = Canvas::for_screen(width, height);
    let mut renderer = ScreenRenderer::new(&config);

    if once {
        let (states, _) = fetch_and_update(&backend, &mut config, &mut renderer).await;
        renderer.render(&mut canvas, &config, 0, &states, true);
        fb.flush(&canvas, Refresh::Full)?;
        tracing::info!("Single frame rendered and flushed to framebuffer");
        std::mem::forget(fb);
        return Ok(());
    }

    let (mut touch_driver, _touch_tx) = TouchDriver::new(
        &config.device.touch_device,
        TouchMapping {
            portrait_w,
            portrait_h,
            swap_xy: config.device.swap_touch_xy,
            invert_x: config.device.invert_touch_x,
            invert_y: config.device.invert_touch_y,
            canvas_rotation,
        },
    );

    let (wake_tx, mut wake_rx) = tokio::sync::mpsc::channel::<()>(8);

    tokio::spawn(async {
        loop {
            power::log_battery();
            tokio::time::sleep(Duration::from_secs(600)).await;
        }
    });

    let mut sigterm = signal(tokio::signal::unix::SignalKind::terminate());
    let mut sigint = signal(tokio::signal::unix::SignalKind::interrupt());
    let mut sighup = signal(tokio::signal::unix::SignalKind::hangup());

    let mut partials: u32 = 0;
    let mut last_tap = Instant::now() - TAP_DEBOUNCE;
    let mut flush = |fb: &mut Box<dyn display::FramebufferDriver>, canvas: &Canvas, force_full: bool| {
        let cycles = config.device.full_refresh_cycles;
        let refresh = if force_full {
            Refresh::Full
        } else if cycles > 0 && partials >= cycles {
            Refresh::FullIfChanged
        } else {
            Refresh::Partial
        };
        match fb.flush(canvas, refresh) {
            Ok(changed) if refresh == Refresh::Full || (changed && refresh == Refresh::FullIfChanged) => partials = 0,
            Ok(true) => partials += 1,
            Ok(false) => {}
            Err(e) => tracing::error!("Framebuffer flush failed: {}", e),
        }
    };

    let mut light = frontlight::Frontlight::detect();
    let light_timeout = Duration::from_secs(config.device.frontlight_timeout_secs as u64);
    let mut light_dimmed = false;
    let mut last_activity = Instant::now();
    renderer.frontlight = light.as_ref().map(|l| l.is_on());

    let wifi_script = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("wifi.sh")))
        .unwrap_or_else(|| PathBuf::from("/mnt/onboard/.kobo-ha/wifi.sh"));

    // Deep sleep is 100% defined and scheduled by the Python backend!
    let (mut current_states, initial_sleep) = fetch_and_update(&backend, &mut config, &mut renderer).await;

    let mut sleep_backend_enabled = initial_sleep.as_ref().map(|s| s.enabled).unwrap_or(true);
    let mut sleep_after = Duration::from_secs(initial_sleep.as_ref().map(|s| s.sleep_after_secs as u64).unwrap_or(30));
    let mut sleep_wake = Duration::from_secs(initial_sleep.as_ref().map(|s| s.sleep_wake_secs as u64).unwrap_or(1800));

    let sleep_hw_supported = power::sleep_unsupported_reason(&wifi_script).is_none();
    if !sleep_hw_supported {
        tracing::warn!("Sleep mode: unavailable on hardware ({})", power::sleep_unsupported_reason(&wifi_script).unwrap_or(""));
    }

    let mut apply_backend_sleep = |s: &BackendSleepInfo| {
        sleep_backend_enabled = s.enabled;
        sleep_after = Duration::from_secs(s.sleep_after_secs.max(5) as u64);
        sleep_wake = Duration::from_secs(s.sleep_wake_secs.max(120) as u64);
        tracing::info!("Backend sleep settings: enabled={}, after={:?}, wake_interval={:?}", sleep_backend_enabled, sleep_after, sleep_wake);
    };

    if let Some(s) = &initial_sleep {
        apply_backend_sleep(s);
    }

    let mut ignore_power_until = Instant::now();
    let mut sleep_failures = 0u32;
    let mut sleep_requested = false;
    let mut last_power_press = Instant::now() - Duration::from_secs(10);

    renderer.render(&mut canvas, &config, 0, &current_states, true);
    flush(&mut fb, &canvas, true);

    let mut clock = tokio::time::interval(Duration::from_secs(60));
    clock.tick().await;

    loop {
        let light_deadline = match &light {
            Some(l) if l.is_on() && !light_timeout.is_zero() => Some(tokio::time::Instant::from_std(last_activity + light_timeout)),
            _ => None,
        };
        let sleep_deadline = if sleep_requested {
            Some(tokio::time::Instant::now())
        } else if sleep_backend_enabled && sleep_hw_supported && sleep_failures < 3 {
            Some(tokio::time::Instant::from_std(last_activity + sleep_after))
        } else {
            None
        };

        tokio::select! {
            _ = async {
                match sleep_deadline {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                sleep_requested = false;
                if power::is_plugged() {
                    tracing::info!("Sleep postponed: device is plugged in");
                    last_activity = Instant::now();
                    continue;
                }
                tracing::info!("Entering backend-scheduled sleep mode (waking in {:?})", sleep_wake);
                let light_was_on = light.as_ref().map(|l| l.is_on()).unwrap_or(false) || light_dimmed;
                let mut woke_by_button = false;
                let mut wifi_down = false;
                if let Some(l) = light.as_mut() {
                    l.set_on(false);
                    renderer.frontlight = Some(false);
                }
                loop {
                    let next = ui::widgets::header::now_local(&config.theme)
                        + chrono::Duration::from_std(sleep_wake).unwrap_or_else(|_| chrono::Duration::minutes(15));
                    let next_time_str = ui::widgets::header::format_time(&next, &config.theme);
                    let sleep_note_text = format!("{} {}", i18n::t("sleep.next"), next_time_str);
                    renderer.sleep_until = Some(sleep_note_text);
                    renderer.render(&mut canvas, &config, 0, &current_states, false);
                    flush(&mut fb, &canvas, true);

                    let wait_until = tokio::time::Instant::now() + Duration::from_secs(7);
                    let mut cancelled = false;
                    while !cancelled {
                        tokio::select! {
                            _ = tokio::time::sleep_until(wait_until) => break,
                            Some(ev) = touch_driver.recv() => cancelled = wants_awake(&ev, last_power_press, ignore_power_until),
                        }
                    }
                    if cancelled {
                        tracing::info!("Sleep cancelled by a tap or the power button");
                        woke_by_button = true;
                        break;
                    }

                    if power::is_plugged() {
                        tracing::info!("Plugged in: staying awake");
                        break;
                    }
                    let script = wifi_script.clone();
                    if !tokio::task::spawn_blocking(move || power::wifi(&script, false)).await.unwrap_or(false) {
                        tracing::warn!("Could not switch Wi-Fi off; not suspending");
                        let script = wifi_script.clone();
                        let _ = tokio::task::spawn_blocking(move || power::wifi(&script, true)).await;
                        break;
                    }
                    let mut cancelled = false;
                    while let Some(ev) = touch_driver.try_recv() {
                        cancelled |= wants_awake(&ev, last_power_press, ignore_power_until);
                    }
                    if cancelled {
                        tracing::info!("Sleep cancelled while Wi-Fi going down");
                        woke_by_button = true;
                        wifi_down = true;
                        break;
                    }
                    if let Err(e) = power::set_alarm(sleep_wake.as_secs()) {
                        tracing::warn!("Could not set wakeup alarm ({}); not suspending", e);
                        let script = wifi_script.clone();
                        let _ = tokio::task::spawn_blocking(move || power::wifi(&script, true)).await;
                        break;
                    }
                    let alarm_at = chrono::Utc::now() + chrono::Duration::from_std(sleep_wake).unwrap_or_default();
                    let mut suspended = Err(String::new());
                    for attempt in 1..=3 {
                        suspended = tokio::task::spawn_blocking(power::suspend).await.unwrap_or_else(|e| Err(e.to_string()));
                        let Err(e) = &suspended else { break };
                        tracing::warn!("Suspend attempt {} failed: {}", attempt, e);
                        if attempt < 3 {
                            tokio::time::sleep(Duration::from_secs(3)).await;
                        }
                    }
                    if suspended.is_err() && sleep_failures == 0 {
                        power::log_suspend_diagnostics();
                    }
                    power::clear_alarm();
                    sleep_failures = if suspended.is_ok() { 0 } else { sleep_failures + 1 };
                    let woke_early = chrono::Utc::now() < alarm_at - chrono::Duration::seconds(20);
                    if woke_early {
                        ignore_power_until = Instant::now() + Duration::from_secs(5);
                    }
                    match &suspended {
                        Ok(()) if woke_early => tracing::info!("Woke up: power button"),
                        Ok(()) => tracing::info!("Woke up: timer"),
                        Err(_) => tracing::warn!("The kernel refused to suspend"),
                    }
                    if suspended.is_ok() {
                        power::log_wakeup_reason();
                    }
                    power::sync();
                    woke_by_button = suspended.is_ok() && woke_early;
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                    touch_driver.drain();
                    if suspended.is_err() || woke_early {
                        wifi_down = true;
                        break;
                    }

                    let script = wifi_script.clone();
                    let wifi_ok = tokio::task::spawn_blocking(move || power::wifi(&script, true)).await.unwrap_or(false);
                    if wifi_ok {
                        let (new_states, new_sleep) = tokio::time::timeout(
                            Duration::from_secs(20),
                            fetch_and_update(&backend, &mut config, &mut renderer)
                        ).await.unwrap_or((current_states.clone(), None));
                        current_states = new_states;
                        if let Some(s) = &new_sleep {
                            apply_backend_sleep(s);
                        }
                    }
                    power::log_battery();
                }
                renderer.sleep_until = None;
                light_dimmed = false;
                if light_was_on {
                    if woke_by_button {
                        if let Some(l) = light.as_mut() {
                            l.set_on(true);
                            renderer.frontlight = Some(true);
                        }
                    } else {
                        light_dimmed = true;
                    }
                }
                if sleep_failures >= 3 {
                    tracing::warn!("Sleep mode: 3 failed attempts in a row, staying awake");
                }
                last_activity = Instant::now();
                renderer.render(&mut canvas, &config, 0, &current_states, !wifi_down);
                flush(&mut fb, &canvas, true);
                tracing::info!("Left sleep mode");
                power::sync();
                if wifi_down {
                    let script = wifi_script.clone();
                    let tx = wake_tx.clone();
                    tokio::spawn(async move {
                        if tokio::task::spawn_blocking(move || power::wifi(&script, true)).await.unwrap_or(false) {
                            let _ = tx.send(()).await;
                        }
                    });
                }
            }

            _ = async {
                match light_deadline {
                    Some(at) => tokio::time::sleep_until(at).await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                if let Some(l) = light.as_mut() {
                    tracing::info!("No taps for {:?}, switching frontlight off", light_timeout);
                    l.set_on(false);
                    light_dimmed = true;
                    renderer.frontlight = Some(false);
                    renderer.render(&mut canvas, &config, 0, &current_states, true);
                    flush(&mut fb, &canvas, false);
                }
            }

            event = touch_driver.recv() => {
                let Some(event) = event else { continue };
                let (x, y) = match event {
                    InputEvent::PowerButton if Instant::now() < ignore_power_until => continue,
                    InputEvent::PowerButton if last_power_press.elapsed() < Duration::from_secs(2) => continue,
                    InputEvent::PowerButton => {
                        last_power_press = Instant::now();
                        if sleep_backend_enabled && sleep_hw_supported && !power::is_plugged() {
                            tracing::info!("Power button pressed: going to sleep");
                            sleep_requested = true;
                            continue;
                        }
                        if let Some(l) = light.as_mut() {
                            let on = !l.is_on();
                            tracing::info!("Power button pressed: light {}", if on { "on" } else { "off" });
                            l.set_on(on);
                            light_dimmed = !on;
                            renderer.frontlight = Some(on);
                            last_activity = Instant::now();
                            renderer.render(&mut canvas, &config, 0, &current_states, true);
                            flush(&mut fb, &canvas, false);
                        }
                        continue;
                    }
                    InputEvent::Plugged => {
                        tracing::info!("USB cable plugged in: returning to Nickel");
                        break;
                    }
                    InputEvent::PowerSourceChanged => {
                        renderer.render(&mut canvas, &config, 0, &current_states, true);
                        flush(&mut fb, &canvas, false);
                        continue;
                    }
                    InputEvent::Tap { x, y } => (x, y),
                };
                if last_tap.elapsed() < TAP_DEBOUNCE {
                    continue;
                }
                last_tap = Instant::now();
                last_activity = Instant::now();
                tracing::debug!("Tap at ({:.0}, {:.0})", x, y);

                if light_dimmed {
                    light_dimmed = false;
                    if let Some(l) = light.as_mut() {
                        l.set_on(true);
                        renderer.frontlight = Some(true);
                        renderer.render(&mut canvas, &config, 0, &current_states, true);
                        flush(&mut fb, &canvas, false);
                        continue;
                    }
                }

                let Some(action) = renderer.handle_touch(x, y) else { continue };
                match action {
                    UiAction::ExitToReader => {
                        tracing::info!("Exit tapped");
                        break;
                    }
                    UiAction::Refresh => {
                        let (new_states, new_sleep) = fetch_and_update(&backend, &mut config, &mut renderer).await;
                        current_states = new_states;
                        if let Some(s) = &new_sleep {
                            apply_backend_sleep(s);
                        }
                        renderer.render(&mut canvas, &config, 0, &current_states, true);
                        flush(&mut fb, &canvas, true);
                    }
                    UiAction::ToggleFrontlight => {
                        if let Some(l) = light.as_mut() {
                            l.set_on(!l.is_on());
                            renderer.frontlight = Some(l.is_on());
                            renderer.render(&mut canvas, &config, 0, &current_states, true);
                            flush(&mut fb, &canvas, false);
                        }
                    }
                    _ => {}
                }
            }

            _ = wake_rx.recv() => {
                let (new_states, new_sleep) = fetch_and_update(&backend, &mut config, &mut renderer).await;
                current_states = new_states;
                if let Some(s) = &new_sleep {
                    apply_backend_sleep(s);
                }
                renderer.render(&mut canvas, &config, 0, &current_states, true);
                flush(&mut fb, &canvas, false);
            }

            _ = clock.tick() => {
                renderer.render(&mut canvas, &config, 0, &current_states, true);
                flush(&mut fb, &canvas, false);
            }

            _ = recv_signal(&mut sigterm) => { tracing::info!("SIGTERM received"); break; }
            _ = recv_signal(&mut sigint) => { tracing::info!("SIGINT received"); break; }
            _ = recv_signal(&mut sighup) => { tracing::info!("SIGHUP received"); break; }
        }
    }

    input::discard_nickel_power_presses();
    if let Some(l) = light.as_mut() {
        l.restore();
    }
    fb.restore();
    drop(fb);
    tracing::info!("Dashboard exited cleanly");
    std::process::exit(0);
}

fn wants_awake(event: &InputEvent, last_power_press: Instant, ignore_power_until: Instant) -> bool {
    match event {
        InputEvent::PowerButton => last_power_press.elapsed() >= Duration::from_secs(2) && Instant::now() >= ignore_power_until,
        InputEvent::Tap { .. } | InputEvent::Plugged => true,
        InputEvent::PowerSourceChanged => false,
    }
}

#[cfg(unix)]
fn signal(kind: tokio::signal::unix::SignalKind) -> Option<tokio::signal::unix::Signal> {
    tokio::signal::unix::signal(kind).ok()
}

#[cfg(unix)]
async fn recv_signal(sig: &mut Option<tokio::signal::unix::Signal>) {
    match sig {
        Some(s) => {
            s.recv().await;
        }
        None => std::future::pending::<()>().await,
    }
}
