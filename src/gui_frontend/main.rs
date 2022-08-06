use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::{egui, Frame, NativeOptions};
use eframe::egui::{Align, Align2, Context, Direction, Id, Layout, ProgressBar, RichText, Ui, Vec2, Window};
use nfd2::Response;

use texture_stacker::{Config, ConfigFile};
use crate::egui::DroppedFile;

fn main() {
    let mut window = MainWindow::new();
    window.init();

    // Show the window.
    let options = NativeOptions {
        initial_window_size: Some(Vec2::new(460.0, 280.0)),
        resizable: false,
        ..NativeOptions::default()
    };

    eframe::run_native(
        "Texture Stacker",
        options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Box::new(window)
        }),
    )
}

#[derive(Eq, PartialEq)]
enum ProcessingStatus {
    None,
    Processing,
    Completed,
}

struct ProcessingState {
    progress: f32,
}

struct MainWindow {
    config: Config,
    processing_status: ProcessingStatus,
    process_thread: Option<thread::JoinHandle<Result<(), String>>>,
    processing_state: Arc<Mutex<ProcessingState>>,
    //process_log: String,
    is_showing_error: bool,
    error_message: String,
}

impl MainWindow {
    pub fn new() -> Self {
        Self {
            config: Default::default(),

            processing_status: ProcessingStatus::None,
            process_thread: None,
            processing_state: Arc::new(Mutex::new(ProcessingState { progress: 0.0 })),
            //process_log: String::new(),

            is_showing_error: false,
            error_message: String::new(),
        }
    }

    pub fn init(&mut self) {
        if let Ok(config_file) = texture_stacker::read_config_file() {
            self.config = config_file.into();
            println!("Loaded config file.");
        }
    }

    fn draw_suffix_list(&mut self, ui: &mut Ui) {
        let mut index_to_remove: Option<usize> = None;

        let suffix_count = self.config.suffixes.len();
        for (i, suffix) in self.config.suffixes.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(suffix);
                ui.add_enabled_ui(suffix_count > 1, |ui| {
                    if ui.button("X").clicked() {
                        index_to_remove = Some(i);
                    }
                });
            });
        }

        if ui.button("Add Suffix").clicked() {
            self.config.suffixes.push("_D".to_owned());
        }

        if let Some(index) = index_to_remove {
            self.config.suffixes.remove(index);
        }
    }

    fn display_error(&mut self, message: &str) {
        self.error_message = message.to_owned();
        self.is_showing_error = true;
    }

    fn validate_input_fields(&mut self) -> bool {
        return if !Path::new(&self.config.input_directory).is_dir() {
            self.display_error("Invalid input directory.");
            false
        } else if
        self.config.suffixes.is_empty() ||
            self.config.suffixes.iter().all(|suffix| suffix.is_empty())
        {
            self.display_error("No suffixes specified.");
            false
        } else {
            true
        };
    }

    fn start_processing(&mut self) {
        let mut config = self.config.clone();
        let progress_mutex = self.processing_state.clone();

        // Remove empty suffixes and duplicates.
        config.suffixes.retain(|suffix| !suffix.is_empty());
        config.suffixes = remove_duplicates(config.suffixes);

        self.process_thread = Some(thread::spawn(move || {
            match texture_stacker::run(&config, Some(Box::new(move |progress: f32| {
                let mut state = progress_mutex.lock().unwrap();
                state.progress = progress;
            }))) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.to_string()),
            }
        }));
    }

    /// Handles drag and drop and drawing help text on hover. Returns true if hovering, and drawing
    /// the main window content should be skipped.
    fn handle_drag_and_drop(&mut self, ui: &mut Ui) -> bool {
        let is_hovering_files: bool = !ui.ctx().input().raw.hovered_files.is_empty();
        let dropped_files: Vec<DroppedFile> = ui.ctx().input().raw.dropped_files.clone();

        if !dropped_files.is_empty() {
            if let Some(path) = &dropped_files[0].path {
                let mut path = path.clone();
                // If a file was dropped, we want take it's containing directory instead.
                if path.is_file() {
                    path.pop();
                }

                self.config.input_directory = path.to_string_lossy().to_string();
            }
        }

        if is_hovering_files {
            ui.with_layout(Layout::centered_and_justified(Direction::TopDown), |ui| {
                ui.label(RichText::new("Drop Input Here").size(42.0));
            });

            return true
        }

        false
    }

    fn draw_main_window_content(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Input Directory");
            file_picker_field(ui, &mut self.config.input_directory);
        });

        ui.horizontal(|ui| {
            ui.label("Output Texture Name");
            ui.text_edit_singleline(&mut self.config.output_texture_name);
        });

        ui.checkbox(&mut self.config.keep_mask_alpha, "Keep Mask Alpha");


        ui.label("Suffixes");
        self.draw_suffix_list(ui);

        ui.add_space(5.0);
        if ui.button("Reset Settings To Default").clicked() {
            self.config = Config::default();
        }

        ui.separator();
        if ui.button(RichText::new("Combine").size(24.0)).clicked() {
            if self.validate_input_fields() {
                self.start_processing();
                self.processing_status = ProcessingStatus::Processing;
            }
        }
    }

    fn draw_error_window(&mut self, ctx: &Context) {
        Window::new("Error")
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(&self.error_message);
                ui.with_layout(Layout::default().with_cross_align(Align::Center), |ui| {
                    if ui.button("Ok").clicked() {
                        self.is_showing_error = false;
                    }
                })
            });
    }

    fn update_processing_state(&mut self) {
        if self.processing_status == ProcessingStatus::Processing {
            // See if the thread has finished.
            if self.process_thread.as_ref().unwrap().is_finished() {
                self.processing_status = ProcessingStatus::Completed;

                // Get the result from the thread handle.
                let handle = self.process_thread.take().unwrap();
                let result = handle.join().expect("Failed to join worker thread");

                match result {
                    Ok(_) => {}
                    Err(err) => {
                        // Hide the process dialog and show error.
                        self.processing_status = ProcessingStatus::None;
                        self.display_error(&err);
                    },
                }
            }
        }
    }

    fn draw_processing_window(&mut self, ctx: &Context) {
        let is_processing = self.processing_status == ProcessingStatus::Processing;
        let title = if is_processing { "Combining..." } else { "Completed" };
        Window::new(title)
            .id(Id::new("processing_window")) // required because the title changes
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                let progress = self.processing_state.lock().unwrap().progress;

                ui.add(ProgressBar::new(progress)
                    .show_percentage()
                    .animate(self.processing_status == ProcessingStatus::Processing));

                if !is_processing {
                    ui.with_layout(Layout::default().with_cross_align(Align::Center), |ui| {
                        if ui.button("Ok").clicked() {
                            self.processing_status = ProcessingStatus::None;
                        }
                    });
                }
            });
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let is_dialog_showing = self.is_showing_error || self.processing_status != ProcessingStatus::None;

            // Handle drag & drop in main window.
            if !is_dialog_showing {
                if self.handle_drag_and_drop(ui) {
                    return;
                }
            }

            ui.add_enabled_ui(!is_dialog_showing, |ui| {
                self.draw_main_window_content(ui)
            });

            // Dialogs
            if self.processing_status != ProcessingStatus::None {
                self.update_processing_state();
                self.draw_processing_window(ctx);
            }

            if self.is_showing_error {
                self.draw_error_window(ctx);
            }
        });
    }

    fn on_exit(&mut self, _gl: &eframe::glow::Context) {
        // No point in handling errors here as the user will never be able to see them.
        match texture_stacker::write_config_file(&ConfigFile::from(self.config.clone())) {
            Ok(_) => println!("Wrote config file."),
            Err(err) => println!("Error writing config file: {}", err),
        }
    }
}


fn file_picker_field(ui: &mut Ui, string: &mut String) {
    ui.horizontal(|ui| {
        ui.text_edit_singleline(string);

        if ui.button("Browse").clicked() {
            match nfd2::open_pick_folder(None).expect("aaaaaah") {
                Response::Okay(folder_path) => {
                    string.clear();
                    string.push_str(&folder_path.to_string_lossy());
                }
                Response::OkayMultiple(_) => {}
                Response::Cancel => {}
            }
        }
    });
}

/// Consumes the vector and returns a new vector with all duplicate elements removed. Unique
/// elements will be preserved in order.
fn remove_duplicates<T>(input: Vec<T>) -> Vec<T>
    where T: PartialEq
{
    let mut output: Vec<T> = Vec::new();
    for elem in input.into_iter() {
        if !output.contains(&elem) {
            output.push(elem);
        }
    }

    output
}
