#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod backend;
mod editor;
mod lifecycle;
mod platform;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use eframe::egui;
use oxide_core::config::{Config, Paths};

fn main() {
    env_logger::init();
    if let Err(error) = run() {
        eprintln!("Oxide: {error:#}");
        if !std::env::args().any(|argument| argument == "--smoke-ui") {
            rfd::MessageDialog::new()
                .set_title("Oxide could not start")
                .set_level(rfd::MessageLevel::Error)
                .set_description(format!("{error:#}"))
                .show();
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let mut root = None;
    let mut background = false;
    let mut no_worker = false;
    let mut normal = false;
    let mut smoke = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                root = Some(PathBuf::from(
                    args.next().context("--data-dir requires a path")?,
                ))
            }
            "--background" => background = true,
            "--normal" => normal = true,
            "--no-worker" => no_worker = true,
            "--smoke-ui" => smoke = true,
            "--help" => {
                println!(
                    "oxide-app [--data-dir PATH] [--background] [--normal] [--no-worker]\n--smoke-ui requires an explicit disposable --data-dir and exits after rendering."
                );
                return Ok(());
            }
            _ => bail!("Unknown argument: {arg}"),
        }
    }
    if smoke && root.is_none() {
        bail!("--smoke-ui requires a disposable --data-dir");
    }
    let custom_root = root.is_some();
    let paths = Paths::discover(root)?;
    let Some(resident) = lifecycle::Resident::claim(&paths, normal)? else {
        return Ok(());
    };
    let (mut config, config_error, onboarding) = match Config::load(&paths.config) {
        Ok(Some(config)) => (config, None, false),
        Ok(None) => {
            let mut config = Config::default();
            if custom_root {
                config.capture_directory = paths.data.join("captures");
            }
            config.save(&paths.config)?;
            (config, None, true)
        }
        Err(error) => (Config::default(), Some(format!("{error:#}")), false),
    };
    if normal && config_error.is_none() {
        config.recording_exclusion_enabled = false;
        config.save(&paths.config)?;
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Oxide")
            .with_inner_size([1120.0, 760.0])
            .with_min_inner_size([420.0, 360.0])
            .with_visible(false)
            .with_icon(icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Oxide",
        options,
        Box::new(move |creation| {
            Ok(Box::new(app::App::new(
                creation,
                paths,
                config,
                config_error,
                resident,
                app::Launch {
                    background,
                    no_worker,
                    smoke,
                    onboarding,
                },
            )?))
        }),
    )
    .map_err(|error| anyhow::anyhow!("Native application failed: {error}"))
}

fn icon() -> egui::IconData {
    let mut rgba = vec![0_u8; 32 * 32 * 4];
    for y in 0_i32..32 {
        for x in 0_i32..32 {
            let distance = (x - 16) * (x - 16) + (y - 16) * (y - 16);
            if (70..=170).contains(&distance) {
                let index = ((y * 32 + x) * 4) as usize;
                rgba[index..index + 4].copy_from_slice(&[190, 110, 76, 255]);
            }
        }
    }
    egui::IconData {
        rgba,
        width: 32,
        height: 32,
    }
}
