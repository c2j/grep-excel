// Validation spike: egui GUI + Chinese IME + existing SearchEngine.
// Run: cargo run --bin spike --features "gui,engine-memory"

use eframe::egui;
use egui_extras::{Column, TableBuilder};
use grep_excel::engine::{DefaultEngine, SearchEngine, SearchMode, SearchQuery};
use grep_excel::types::SearchResult;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "grep_excel GUI Spike — IME & Table Validation",
        options,
        Box::new(|cc| {
            setup_cjk_fonts(&cc.egui_ctx);
            Ok(Box::new(SpikeApp::new()))
        }),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

/// Try to load a system CJK font for Chinese character rendering.
/// Falls back to egui default (Latin-only) if no CJK font found —
/// characters will show as tofu (□), but IME test is still valid.
fn setup_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    let cjk_font_paths = [
        // macOS
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "/System/Library/Fonts/Hiragino Sans GB.ttc",
        // Linux — common CJK font locations
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/wenquanyi/wqy-microhei/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        // Windows
        "C:\\Windows\\Fonts\\msyh.ttc",
        "C:\\Windows\\Fonts\\simsun.ttc",
    ];

    for path in &cjk_font_paths {
        if let Ok(data) = std::fs::read(path) {
            fonts
                .font_data
                .insert("CJK".into(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            fonts
                .families
                .get_mut(&egui::FontFamily::Proportional)
                .unwrap()
                .insert(0, "CJK".into());
            fonts
                .families
                .get_mut(&egui::FontFamily::Monospace)
                .unwrap()
                .push("CJK".into());
            eprintln!("[spike] Loaded CJK font: {}", path);
            break;
        }
    }

    ctx.set_fonts(fonts);
}

struct SpikeApp {
    engine: DefaultEngine,
    search_text: String,
    results: Vec<SearchResult>,
    status: String,
    imported_file_name: Option<String>,
    file_loaded: bool,
}

impl SpikeApp {
    fn new() -> Self {
        Self {
            engine: DefaultEngine::new().expect("Failed to create search engine"),
            search_text: String::new(),
            results: Vec::new(),
            status: "就绪 — 点击「打开文件」加载 Excel，在搜索框输入中文测试 IME".into(),
            imported_file_name: None,
            file_loaded: false,
        }
    }

    fn import_file(&mut self, path: PathBuf) {
        let path_str = path.display().to_string();
        self.status = format!("正在导入: {}...", path_str);
        let start = std::time::Instant::now();

        match self.engine.import_excel(&path, &|_, _| {}) {
            Ok(info) => {
                let elapsed = start.elapsed();
                self.imported_file_name = Some(info.name.clone());
                self.file_loaded = true;
                self.status = format!(
                    "✅ 已导入: {} ({} sheets, {} 行, {:.1}s)",
                    info.name,
                    info.sheets.len(),
                    info.total_rows,
                    elapsed.as_secs_f64()
                );
            }
            Err(e) => {
                self.status = format!("❌ 导入失败: {}", e);
            }
        }
    }

    fn do_search(&mut self) {
        if self.search_text.trim().is_empty() {
            self.results.clear();
            self.status = "请输入搜索关键词".into();
            return;
        }

        self.status = format!("正在搜索: {}...", self.search_text);
        let start = std::time::Instant::now();

        let query = SearchQuery {
            text: self.search_text.clone(),
            column: None,
            mode: SearchMode::FullText,
            limit: 5000,
            sheet: None,
            invert: false,
            context_lines: None,
            conditions: Vec::new(),
        };

        match self.engine.search(&query) {
            Ok((results, stats)) => {
                let elapsed = start.elapsed();
                self.results = results;
                self.status = format!(
                    "🔍 找到 {} 条结果 ({} 行已搜索, {}ms)",
                    stats.total_matches, stats.total_rows_searched, elapsed.as_millis()
                );
                if stats.truncated {
                    self.status
                        .push_str(&format!(" [截断, 仅显示前 {} 条]", self.results.len()));
                }
            }
            Err(e) => {
                self.status = format!("❌ 搜索失败: {}", e);
                self.results.clear();
            }
        }
    }
}

impl eframe::App for SpikeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📁 打开文件").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Excel/CSV", &["xlsx", "xls", "xlsm", "xlsb", "ods", "csv"])
                        .pick_file()
                    {
                        self.import_file(path);
                    }
                }

                if let Some(ref name) = self.imported_file_name {
                    ui.label(format!("📄 {}", name));
                }

                ui.separator();

                // IME validation point: type Chinese here
                ui.label("🔍 搜索:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.search_text)
                        .hint_text("在此输入中文测试 IME...")
                        .desired_width(280.0),
                );

                if ui.button("搜索").clicked() {
                    self.do_search();
                }

                if ui.button("✕ 清除").clicked() {
                    self.search_text.clear();
                    self.results.clear();
                    self.status = "已清除".into();
                }

                ui.separator();
                ui.label("[全文搜索]");

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.weak("Spike v0.1 — IME + 表格 + 引擎");
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.heading("grep_excel GUI Spike");
                    ui.add_space(16.0);
                    ui.label("验证清单:");
                    ui.add_space(8.0);
                    ui.label("  1️⃣  点击「📁 打开文件」加载 Excel/CSV 文件");
                    ui.label("  2️⃣  在搜索框中用中文输入法输入关键词 (IME 测试)");
                    ui.label("  3️⃣  点击「搜索」查看结果表格");
                    ui.label("  4️⃣  观察中文是否正确渲染 (不应显示 □ 方块)");
                    ui.label("  5️⃣  尝试拖拽列宽、滚动表格");
                    ui.add_space(16.0);
                    if self.file_loaded {
                        ui.colored_label(egui::Color32::GREEN, "✅ 文件已加载，开始搜索吧");
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(200, 200, 200),
                            "⬆ 请先加载文件",
                        );
                    }
                });
                return;
            }

            let all_cols = &self.results[0].col_names;

            egui::ScrollArea::both().show(ui, |ui| {
                let mut table = TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .min_scrolled_height(0.0);

                table = table.column(Column::initial(120.0).resizable(true));
                table = table.column(Column::initial(100.0).resizable(true));
                for _ in all_cols {
                    table = table.column(Column::initial(100.0).resizable(true));
                }

                table
                    .header(22.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("File");
                        });
                        header.col(|ui| {
                            ui.strong("Sheet");
                        });
                        for col_name in all_cols {
                            header.col(|ui| {
                                ui.strong(col_name);
                            });
                        }
                    })
                    .body(|body| {
                        body.rows(20.0, self.results.len(), |mut row| {
                            let result = &self.results[row.index()];
                            row.col(|ui| {
                                ui.label(&result.file_name);
                            });
                            row.col(|ui| {
                                ui.label(&result.sheet_name);
                            });
                            for col_name in all_cols {
                                let value = result
                                    .col_names
                                    .iter()
                                    .position(|c| c == col_name)
                                    .and_then(|idx| result.row.get(idx))
                                    .map(|s| s.as_str())
                                    .unwrap_or("");

                                let is_matched = result
                                    .col_names
                                    .iter()
                                    .position(|c| c == col_name)
                                    .map(|idx| result.matched_columns.contains(&idx))
                                    .unwrap_or(false);

                                row.col(|ui| {
                                    if is_matched {
                                        ui.colored_label(
                                            egui::Color32::from_rgb(255, 200, 80),
                                            value,
                                        );
                                    } else {
                                        ui.label(value);
                                    }
                                });
                            }
                        });
                    });
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if !self.results.is_empty() {
                        ui.label(format!("{} 行", self.results.len()));
                    }
                });
            });
        });

        ctx.request_repaint();
    }
}
