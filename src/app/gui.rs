use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;
use rfd::FileDialog;

use crate::app::execute::{ExecutionError, ExecutionSummary, execute_plan};
use crate::config::{FfmpegPreset, MediaJuicerConfig, ProcessingMode};
use crate::image_processing::SystemImageBackend;
use crate::planning::build_processing_plan;
use crate::progress::ProgressSnapshot;
use crate::timestamps::FileSystemTimestampProvider;
use crate::video_processing::{StdFileSizeProvider, SystemFfmpegExecutor};

pub struct MediaJuicerApp {
    config: MediaJuicerConfig,
    state: AppState,
}

enum AppState {
    Idle,
    Processing {
        cancel_token: Arc<AtomicBool>,
        progress: Arc<Mutex<Option<ProgressSnapshot>>>,
        // We use a Mutex-wrapped Option to take the handle out when finished
        thread_handle: Mutex<Option<thread::JoinHandle<Result<ExecutionSummary, ExecutionError>>>>,
    },
    Finished(Result<ExecutionSummary, ExecutionError>),
}

impl MediaJuicerApp {
    pub fn new(config: MediaJuicerConfig) -> Self {
        Self {
            config,
            state: AppState::Idle,
        }
    }

    fn start_processing(&mut self) {
        let config = self.config.clone();
        let cancel_token = Arc::new(AtomicBool::new(false));
        let progress = Arc::new(Mutex::new(None));

        let cancel_token_thread = Arc::clone(&cancel_token);
        let progress_thread = Arc::clone(&progress);

        let handle = thread::spawn(move || {
            let source_path = Path::new(&config.folder_path);
            if !(source_path.exists() && source_path.is_dir()) {
                return Err(ExecutionError::ReportIo(format!(
                    "Source folder does not exist or is not a directory: {}",
                    source_path.display()
                )));
            }

            let plan = match build_processing_plan(source_path, config.mode, config.only.as_deref())
            {
                Ok(plan) => plan,
                Err(err) => return Err(ExecutionError::ReportIo(err.to_string())),
            };

            let image_backend = SystemImageBackend;
            let ffmpeg_executor = SystemFfmpegExecutor;
            let size_provider = StdFileSizeProvider;
            let timestamps = FileSystemTimestampProvider;
            let mut sink = std::io::sink();

            let mut callback = |snapshot: ProgressSnapshot| {
                let mut p = progress_thread.lock().unwrap();
                *p = Some(snapshot);
            };

            execute_plan(
                &plan,
                &config,
                &image_backend,
                &ffmpeg_executor,
                &size_provider,
                &timestamps,
                &mut sink,
                Some(cancel_token_thread),
                Some(&mut callback),
            )
        });

        self.state = AppState::Processing {
            cancel_token,
            progress,
            thread_handle: Mutex::new(Some(handle)),
        };
    }
}

impl eframe::App for MediaJuicerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Media Juicer");
            ui.add_space(10.0);

            // Path Selection
            ui.horizontal(|ui| {
                ui.label("Source Folder:");
                ui.text_edit_singleline(&mut self.config.folder_path);
                if let Some(path) = ui
                    .button("Browse...")
                    .clicked()
                    .then(|| FileDialog::new().pick_folder())
                    .flatten()
                {
                    self.config.folder_path = path.display().to_string();
                }
            });

            ui.add_space(10.0);

            // Templates
            ui.horizontal(|ui| {
                ui.label("Templates:");
                egui::ComboBox::from_id_source("templates")
                    .selected_text("Select a template...")
                    .show_ui(ui, |ui| {
                        if ui.button("1_very high quality").clicked() {
                            self.config.crf = 26;
                            self.config.ffmpeg_speed = FfmpegPreset::Veryslow;
                        }
                        if ui.button("2_good quality").clicked() {
                            self.config.crf = 28;
                            self.config.ffmpeg_speed = FfmpegPreset::Slow;
                        }
                        if ui.button("3_medium quality").clicked() {
                            self.config.crf = 32;
                            self.config.ffmpeg_speed = FfmpegPreset::Slow;
                        }
                        if ui.button("4_aggressive_portable").clicked() {
                            self.config.crf = 34;
                            self.config.video_max_pixels = 1080;
                        }
                    });
            });

            ui.add_space(10.0);

            // Options
            ui.collapsing("Processing Options", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Mode:");
                    ui.selectable_value(&mut self.config.mode, ProcessingMode::All, "All");
                    ui.selectable_value(&mut self.config.mode, ProcessingMode::Videos, "Videos");
                    ui.selectable_value(&mut self.config.mode, ProcessingMode::Images, "Images");
                    ui.selectable_value(
                        &mut self.config.mode,
                        ProcessingMode::FixDates,
                        "Fix Dates",
                    );
                });

                ui.checkbox(&mut self.config.replace, "Replace input files");
                ui.checkbox(&mut self.config.ignore_timestamps, "Ignore timestamps");

                ui.horizontal(|ui| {
                    ui.label("Only (filter):");
                    let mut only_val = self.config.only.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut only_val).changed() {
                        self.config.only = if only_val.is_empty() {
                            None
                        } else {
                            Some(only_val)
                        };
                    }
                });
            });

            ui.collapsing("Video Options", |ui| {
                ui.horizontal(|ui| {
                    ui.label("CRF (0-51):");
                    ui.add(egui::Slider::new(&mut self.config.crf, 0..=51));
                });

                ui.horizontal(|ui| {
                    ui.label("Speed:");
                    egui::ComboBox::from_label("")
                        .selected_text(format!("{:?}", self.config.ffmpeg_speed))
                        .show_ui(ui, |ui| {
                            let variants = [
                                FfmpegPreset::Ultrafast,
                                FfmpegPreset::Superfast,
                                FfmpegPreset::Veryfast,
                                FfmpegPreset::Faster,
                                FfmpegPreset::Fast,
                                FfmpegPreset::Medium,
                                FfmpegPreset::Slow,
                                FfmpegPreset::Slower,
                                FfmpegPreset::Veryslow,
                                FfmpegPreset::Placebo,
                            ];
                            for v in variants {
                                ui.selectable_value(
                                    &mut self.config.ffmpeg_speed,
                                    v,
                                    format!("{:?}", v),
                                );
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Max Pixels (0 = no resize):");
                    ui.add(
                        egui::DragValue::new(&mut self.config.video_max_pixels)
                            .speed(10)
                            .range(0..=8192),
                    );
                });
            });

            ui.collapsing("Image Options", |ui| {
                ui.horizontal(|ui| {
                    ui.label("WebP Quality (0-100):");
                    ui.add(egui::Slider::new(&mut self.config.webpq, 0..=100));
                });

                ui.horizontal(|ui| {
                    ui.label("Max Pixels:");
                    ui.add(
                        egui::DragValue::new(&mut self.config.image_max_pixels)
                            .speed(10)
                            .range(0..=8192),
                    );
                });
            });

            ui.add_space(20.0);

            // Control & Progress
            let mut next_state = None;
            match &self.state {
                AppState::Idle => {
                    if ui.button("Start Processing").clicked() {
                        self.start_processing();
                    }
                }
                AppState::Processing {
                    cancel_token,
                    progress,
                    thread_handle,
                } => {
                    if ui.button("Cancel").clicked() {
                        cancel_token.store(true, Ordering::Relaxed);
                    }

                    let current_progress = {
                        let p = progress.lock().unwrap();
                        p.clone()
                    };

                    if let Some(snapshot) = current_progress {
                        let total = snapshot.total_files;
                        let processed = snapshot.processed_files;
                        let ratio = if total > 0 {
                            processed as f32 / total as f32
                        } else {
                            0.0
                        };

                        ui.add(
                            egui::ProgressBar::new(ratio)
                                .text(format!("Processing {}/{}", processed, total)),
                        );
                    } else {
                        ui.label("Initializing...");
                    }

                    // Check if thread is finished
                    let mut handle_opt = thread_handle.lock().unwrap();
                    if handle_opt.as_ref().is_some_and(|h| h.is_finished()) {
                        let result = handle_opt.take().unwrap().join().unwrap();
                        next_state = Some(AppState::Finished(result));
                    }
                    ctx.request_repaint_after(std::time::Duration::from_millis(100));
                }
                AppState::Finished(result) => {
                    match result {
                        Ok(summary) => {
                            ui.label(format!(
                                "Successfully processed {} files.",
                                summary.progress.processed_files
                            ));
                        }
                        Err(ExecutionError::Cancelled(summary)) => {
                            ui.label(format!(
                                "Cancelled. Processed {} files before stopping.",
                                summary.progress.processed_files
                            ));
                        }
                        Err(err) => {
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
                        }
                    }
                    if ui.button("Back").clicked() {
                        next_state = Some(AppState::Idle);
                    }
                }
            }

            if let Some(new_state) = next_state {
                self.state = new_state;
            }
        });
    }
}
