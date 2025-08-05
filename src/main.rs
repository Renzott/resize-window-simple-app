use eframe::egui;
use egui::{Image, vec2};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM},
        UI::WindowsAndMessaging::{
            EnumWindows, GetWindowTextW, IsWindowVisible, SWP_NOMOVE, SWP_NOZORDER, SetWindowPos,
        },
    },
    core::BOOL,
};

use egui::{FontData, FontDefinitions, FontFamily};
use std::os::windows::ffi::OsStringExt;
use std::{ffi::OsString, io::Cursor};
use zip::ZipArchive;

#[derive(Debug, PartialEq, EnumIter)]
enum WindowResolution {
    Preset320,
    Preset480,
    Preset720,
    Preset1080,
    Preset1440,
}

impl WindowResolution {
    fn value(&self) -> &'static str {
        match self {
            WindowResolution::Preset320 => "320x240",
            WindowResolution::Preset480 => "480x320",
            WindowResolution::Preset720 => "1280x720",
            WindowResolution::Preset1080 => "1920x1080",
            WindowResolution::Preset1440 => "2560x1440",
        }
    }
}

const MAX_TITLE_LENGTH: usize = 512;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 380.0])
            .with_min_inner_size([400.0, 380.0]),
        ..Default::default()
    };

    eframe::run_native(
        "My egui App",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            register_fonts(&cc.egui_ctx);
            Ok(Box::new(MyApp::new()))
        }),
    )
}

fn register_fonts(ctx: &egui::Context) {
    let zip_bytes = std::fs::read("src/fonts/zipfonts.zip").expect("Zipfonts not found");

    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader).expect("ZIP invalid");

    let mut fonts = FontDefinitions::default();

    fn load_font(
        archive: &mut ZipArchive<Cursor<Vec<u8>>>,
        name_in_zip: &str,
        alias: &str,
        fonts: &mut FontDefinitions,
    ) {
        let mut file = archive
            .by_name(name_in_zip)
            .expect(&format!("Not found {}", name_in_zip));
        let mut buf = Vec::new();
        std::io::copy(&mut file, &mut buf).expect("Cannot read file");
        fonts
            .font_data
            .insert(alias.to_string(), FontData::from_owned(buf).into());
    }

    load_font(
        &mut archive,
        "NotoSans-Regular.ttf",
        "noto_sans",
        &mut fonts,
    );
    load_font(&mut archive, "Symbola.ttf", "symbola", &mut fonts);
    load_font(
        &mut archive,
        "NotoSansJP-Regular.ttf",
        "noto_jp",
        &mut fonts,
    );

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .splice(
            0..,
            vec![
                "noto_sans".to_string(),
                "symbola".to_string(),
                "noto_jp".to_string(),
            ],
        );

    ctx.set_fonts(fonts);
}

fn list_visible_windows() -> Vec<(HWND, String)> {
    let mut list: Vec<(HWND, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut list as *mut _ as isize),
        );
    }
    list
}

struct MyApp {
    list_programs: Vec<(HWND, String)>,
    selected_program: Option<HWND>,
    resolution: WindowResolution,
    last_refresh: std::time::Instant,
}

impl MyApp {
    fn new() -> Self {
        Self {
            list_programs: list_visible_windows(),
            selected_program: None,
            resolution: WindowResolution::Preset720,
            last_refresh: std::time::Instant::now(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Resize App");

            let now = std::time::Instant::now();
            if now.duration_since(self.last_refresh).as_secs_f32() > 5.0 {
                self.list_programs = list_visible_windows();
                self.last_refresh = now;

                if let Some(hwnd) = self.selected_program {
                    if let Some((_i, _)) = self
                        .list_programs
                        .iter()
                        .enumerate()
                        .find(|(_, (h, _))| *h == hwnd)
                    {
                        self.selected_program = Some(hwnd);
                    } else {
                        self.selected_program = None;
                    }
                }
            }

            egui::ComboBox::from_label("Choose a program")
                .selected_text(
                    self.selected_program
                        .and_then(|h| {
                            self.list_programs
                                .iter()
                                .find(|(hwnd, _)| *hwnd == h)
                                .map(|(_, title)| title.as_str())
                        })
                        .unwrap_or("Select a program"),
                )
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (hwnd, title) in &self.list_programs {
                        let is_selected = self.selected_program == Some(*hwnd);
                        if ui.selectable_label(is_selected, title).clicked() {
                            self.selected_program = Some(*hwnd);
                        }
                    }
                });

            if let Some(index) = self.selected_program {
                let (hwnd, title) = &self.list_programs[self
                    .list_programs
                    .iter()
                    .position(|(hwnd, _)| *hwnd == index)
                    .unwrap()];
                ui.label(format!("Selected HWND: {:?}, title: {}", hwnd.0, title));
            } else {
                ui.label("Selected program: None");
            }

            ui.separator();

            egui::ComboBox::from_label("Choose a resolution")
                .selected_text(self.resolution.value())
                .width(200.0)
                .show_ui(ui, |ui| {
                    for resolution in WindowResolution::iter() {
                        if ui
                            .selectable_label(self.resolution == resolution, resolution.value())
                            .clicked()
                        {
                            self.resolution = resolution;
                        }
                    }
                });

            ui.label(format!("Selected resolution: {}", self.resolution.value()));

            if ui
                .button("Resize")
                .on_hover_text("Resize the selected window")
                .clicked()
            {
                if self.selected_program.is_none() {
                    return;
                }
                let mut values = self.resolution.value().split('x');
                let width = values.next().unwrap().parse::<u32>().unwrap();
                let height = values.next().unwrap().parse::<u32>().unwrap();

                let (hwnd, _) = &self.list_programs[self
                    .list_programs
                    .iter()
                    .position(|(hwnd, _)| *hwnd == self.selected_program.unwrap())
                    .unwrap()];

                unsafe {
                    let _ = SetWindowPos(
                        *hwnd,
                        None,
                        0,
                        0,
                        width.try_into().unwrap(),
                        height.try_into().unwrap(),
                        SWP_NOMOVE | SWP_NOZORDER,
                    );
                }
            }

            let image = Image::new(egui::include_image!("../kirb.png"))
                .fit_to_exact_size(vec2(200.0, 200.0));

            ui.add(image);
        });
    }
}

extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        if IsWindowVisible(hwnd).as_bool() {
            if let Some(title) = get_window_text(hwnd) {
                let windows_list = &mut *(lparam.0 as *mut Vec<(HWND, String)>);
                if !windows_list.iter().any(|(_, t)| t == &title) {
                    windows_list.push((hwnd, title));
                }
            }
        }
    }
    true.into()
}

fn get_window_text(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; MAX_TITLE_LENGTH];
    let len = unsafe { GetWindowTextW(hwnd, &mut buffer) };

    if len > 0 {
        let os_str = OsString::from_wide(&buffer[..len as usize]);
        let title = os_str.to_string_lossy().to_string();

        if title.len() >= 2 {
            return Some(title);
        }
    }

    None
}
