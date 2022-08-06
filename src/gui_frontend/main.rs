use std::path::Path;
use std::thread;

use eframe::{egui, Frame};
use eframe::egui::{Align, Align2, Context, Id, Layout, RichText, Ui, Window};
use nfd2::Response;

use texture_stacker::ConfigFile;

fn main() {
    let mut window = MainWindow::new();
    window.init();

    // Show the window.
    let options = eframe::NativeOptions::default();
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
enum ProcessingState {
    None,
    Processing,
    Completed,
}

struct MainWindow {
    config: texture_stacker::Config,
    processing_state: ProcessingState,
    process_thread: Option<thread::JoinHandle<Result<(), String>>>,
    //process_log: String,
    is_showing_error: bool,
    error_message: String,
}

impl MainWindow {
    pub fn new() -> Self {
        Self {
            config: Default::default(),

            processing_state: ProcessingState::None,
            process_thread: None,
            //process_log: String::new(),

            is_showing_error: false,
            error_message: String::new(),
        }
    }

    pub fn init(&mut self) {
        match texture_stacker::read_config_file() {
            Ok(config_file) => {
                self.config = config_file.into();
                println!("Loaded config file.");
            }
            Err(_) => {}
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

        // Remove empty suffixes and duplicates.
        config.suffixes.retain(|suffix| !suffix.is_empty());
        config.suffixes = remove_duplicates(config.suffixes);

        self.process_thread = Some(thread::spawn(move || {
            match texture_stacker::run(&config) {
                Ok(_) => Ok(()),
                Err(err) => Err(err.to_string()),
            }
        }));
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

        ui.add_space(12.0);
        if ui.button(RichText::new("Start").size(24.0)).clicked() {
            if self.validate_input_fields() {
                self.start_processing();
                self.processing_state = ProcessingState::Processing;
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
        if self.processing_state == ProcessingState::Processing {
            // See if the thread has finished.
            if self.process_thread.as_ref().unwrap().is_finished() {
                self.processing_state = ProcessingState::Completed;

                // Get the result from the thread handle.
                let handle = self.process_thread.take().unwrap();
                let result = handle.join().expect("Failed to join worker thread");

                match result {
                    Ok(_) => {}
                    Err(err) => self.display_error(&err),
                }
            }
        }
    }

    fn draw_processing_window(&mut self, ctx: &Context) {
        let is_processing = self.processing_state == ProcessingState::Processing;
        let title = if is_processing { "Processing" } else { "Completed" };
        Window::new(title)
            .id(Id::new("processing_window")) // required because the title changes
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                if !is_processing {
                    ui.with_layout(Layout::default().with_cross_align(Align::Center), |ui| {
                        if ui.button("Ok").clicked() {
                            self.processing_state = ProcessingState::None;
                        }
                    });
                }
            });
    }
}

impl eframe::App for MainWindow {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let is_dialog_showing = self.is_showing_error || self.processing_state != ProcessingState::None;
            ui.add_enabled_ui(!is_dialog_showing, |ui| {
                self.draw_main_window_content(ui)
            });

            // Dialogs
            if self.processing_state != ProcessingState::None {
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
