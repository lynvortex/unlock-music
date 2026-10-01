use eframe::egui;
use egui::{Align, CornerRadius, Layout, RichText, Sense, Stroke};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::faq::{self, FaqPage};
use crate::settings::Settings;
use crate::theme;
use crate::worker::{DecryptPool, DecryptRequest, JobId, Phase, Response};

/// 文件选择框里列出的扩展名，对应 README 里支持的格式。
const KNOWN_EXTENSIONS: &[&str] = &[
    "ncm", "kgm", "vpr", "kgg", "kwm", "xm", "x2m", "x3m", "qta", "mflac", "mflac0", "mflac1",
    "mgg", "mgg0", "mgg1", "mggl", "mgalaxy", "mflach", "qmc3", "qmcflac", "qmc0", "mg3d",
];

/// 后台解密线程数。
const WORKER_THREADS: usize = 4;

/// 文件卡片宽度与间距。
const CARD_WIDTH: f32 = 320.0;
const CARD_GAP: f32 = 18.0;

const TAB_NAMES: &[&str] = &["应用", "设置", "答疑"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Home,
    Settings,
    Faq,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsPage {
    Home,
    Qmc,
    Kwm,
    Kgg,
    Qtfm,
}

impl SettingsPage {
    /// 侧栏顺序。
    const ALL: &'static [(SettingsPage, &'static str)] = &[
        (SettingsPage::Home, "设置"),
        (SettingsPage::Qmc, "QMCv2 密钥"),
        (SettingsPage::Kwm, "KWMv2 密钥"),
        (SettingsPage::Kgg, "KGG 密钥"),
        (SettingsPage::Qtfm, "蜻蜓 FM"),
    ];
}

#[derive(Debug)]
enum JobState {
    Pending,
    Running(Phase),
    Done {
        cipher: String,
        extension: String,
        output: PathBuf,
    },
    Failed(String),
}

struct Job {
    id: JobId,
    input: PathBuf,
    file_name: String,
    state: JobState,
    selected: bool,
}

impl Job {
    fn is_pending(&self) -> bool {
        matches!(self.state, JobState::Pending)
    }
}

/// 卡片上产生的操作。先收集、再统一执行，避免与 `jobs` 的可变借用打架。
enum Action {
    ToggleSelect(JobId),
    Remove(JobId),
    OpenDir(PathBuf),
}

pub struct UmApp {
    tab: Tab,
    settings_page: SettingsPage,
    faq_page: FaqPage,
    jobs: Vec<Job>,
    next_id: JobId,
    pool: DecryptPool,
    settings: Settings,
    settings_dirty: bool,
    message: String,
}

impl UmApp {
    pub fn new() -> Self {
        Self {
            tab: Tab::Home,
            settings_page: SettingsPage::Home,
            faq_page: FaqPage::Home,
            jobs: Vec::new(),
            next_id: 1,
            pool: DecryptPool::new(WORKER_THREADS),
            settings: Settings::load(),
            settings_dirty: false,
            message: String::new(),
        }
    }

    // —— 文件管理 ——

    fn add_paths(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let mut added = 0usize;
        let mut skipped = 0usize;

        for path in paths {
            if !path.is_file() {
                skipped += 1;
                continue;
            }
            // 同一个文件不重复添加
            if self.jobs.iter().any(|job| job.input == path) {
                skipped += 1;
                continue;
            }

            let file_name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();

            self.jobs.push(Job {
                id: self.next_id,
                input: path,
                file_name,
                state: JobState::Pending,
                selected: false,
            });
            self.next_id += 1;
            added += 1;
        }

        self.message = if skipped > 0 {
            format!("已添加 {added} 个文件，跳过 {skipped} 个")
        } else {
            format!("已添加 {added} 个文件")
        };
    }

    /// 递归收集目录下扩展名已知的文件。
    fn add_folder(&mut self, dir: &Path) {
        let mut found = Vec::new();
        collect_files(dir, &mut found);
        self.add_paths(found);
    }

    fn pick_files(&mut self) {
        if let Some(files) = rfd::FileDialog::new()
            .add_filter("加密音乐", KNOWN_EXTENSIONS)
            .add_filter("全部文件", &["*"])
            .pick_files()
        {
            self.add_paths(files);
        }
    }

    fn start_decrypt(&mut self) {
        let mut submitted = 0usize;
        let mut errors = 0usize;

        for job in &mut self.jobs {
            if !job.is_pending() {
                continue;
            }
            let request = DecryptRequest {
                id: job.id,
                input: job.input.clone(),
                output_dir: resolved_output_dir(&self.settings, &job.input),
                options: self.settings.to_options(job.file_name.clone()),
            };
            match self.pool.submit(request) {
                Ok(()) => {
                    job.state = JobState::Running(Phase::Reading);
                    submitted += 1;
                }
                Err(_) => errors += 1,
            }
        }

        self.message = if errors == 0 {
            format!("已提交 {submitted} 个文件")
        } else {
            format!("已提交 {submitted} 个文件，{errors} 个提交失败")
        };
    }

    /// 收取后台消息：更新阶段，解密完成后把临时文件落成最终输出。
    fn collect_results(&mut self) {
        let responses = self.pool.drain();
        if responses.is_empty() {
            return;
        }

        // 本批次里所有输入文件都不允许被输出覆盖 —— 不只是发起该任务的那一个。
        // 否则同一批里 a.flac 与 a.ncm 会让后者把前者的输出写到 a.flac 上。
        let protected: Vec<PathBuf> = self.jobs.iter().map(|job| job.input.clone()).collect();

        for response in responses {
            match response {
                Response::Phase { id, phase } => {
                    if let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) {
                        // 只在仍处于运行中时接受阶段更新，避免覆盖已完成的状态
                        if matches!(job.state, JobState::Running(_)) {
                            job.state = JobState::Running(phase);
                        }
                    }
                }
                Response::Finished { id, result } => {
                    let Some(index) = self.jobs.iter().position(|job| job.id == id) else {
                        // 任务已被移除：把临时文件清掉，不要留下垃圾
                        if let Ok(success) = result {
                            let _ = std::fs::remove_file(&success.temp_path);
                        }
                        continue;
                    };

                    match result {
                        Ok(success) => {
                            let input = self.jobs[index].input.clone();
                            match finalize_output(
                                &self.settings,
                                &input,
                                &success.extension,
                                &success.temp_path,
                                &protected,
                            ) {
                                Ok(path) => {
                                    self.jobs[index].state = JobState::Done {
                                        cipher: success.cipher_name,
                                        extension: success.extension,
                                        output: path,
                                    };
                                }
                                Err(error) => self.jobs[index].state = JobState::Failed(error),
                            }
                        }
                        Err(error) => self.jobs[index].state = JobState::Failed(error),
                    }
                }
            }
        }
    }

    fn counts(&self) -> Counts {
        let mut counts = Counts {
            total: self.jobs.len(),
            ..Counts::default()
        };
        for job in &self.jobs {
            match job.state {
                JobState::Pending => counts.pending += 1,
                JobState::Running(_) => counts.running += 1,
                JobState::Done { .. } => counts.done += 1,
                JobState::Failed(_) => counts.failed += 1,
            }
        }
        counts
    }

    fn any_running(&self) -> bool {
        self.jobs
            .iter()
            .any(|job| matches!(job.state, JobState::Running(_)))
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });

        if dropped.is_empty() {
            return;
        }

        let mut files = Vec::new();
        let mut directories = Vec::new();
        for path in dropped {
            if path.is_dir() {
                directories.push(path);
            } else {
                files.push(path);
            }
        }

        // 规则：明确拖进来的文件一律接受（用户已经挑过了）；
        // 只有扫描目录时才按扩展名过滤，见 `has_known_extension`。
        self.add_paths(files);
        for dir in directories {
            self.add_folder(&dir);
        }
        // 拖进来后直接切回「应用」，否则看不到刚加的文件
        self.tab = Tab::Home;
    }

    fn save_settings(&mut self) {
        self.message = match self.settings.save() {
            Ok(()) => {
                self.settings_dirty = false;
                "设置已保存".to_owned()
            }
            Err(error) => error,
        };
    }

    // —— 界面：应用页 ——

    fn home_tab(&mut self, ui: &mut egui::Ui) {
        // 内容区居中且限宽
        let max_width = (ui.available_width() * 0.8).clamp(480.0, 1180.0);
        let left_pad = ((ui.available_width() - max_width) * 0.5).max(0.0);

        ui.horizontal_top(|ui| {
            ui.add_space(left_pad);
            ui.vertical(|ui| {
                ui.set_width(max_width);
                ui.add_space(16.0);

                if self.settings_dirty {
                    if theme::warning_alert(
                        ui,
                        "警告",
                        "有尚未储存的设置，设定将在保存后生效。",
                        "立即储存",
                    ) {
                        self.save_settings();
                    }
                    ui.add_space(14.0);
                }

                self.drop_zone(ui);
                ui.add_space(20.0);
                self.file_grid(ui);
                ui.add_space(16.0);
            });
        });
    }

    /// 拖放 / 点击选择文件的区域。
    fn drop_zone(&mut self, ui: &mut egui::Ui) {
        let hovering = ui.ctx().input(|input| !input.raw.hovered_files.is_empty());
        let (fill, stroke_color, stroke_width) = if hovering {
            (theme::PRIMARY.gamma_multiply(0.10), theme::PRIMARY, 2.0)
        } else {
            (theme::BASE_200, theme::BASE_300, 1.0)
        };

        let response = egui::Frame::new()
            .fill(fill)
            .stroke(Stroke::new(stroke_width, stroke_color))
            .corner_radius(CornerRadius::same(theme::RADIUS))
            .inner_margin(28.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("拖放或点我选择需要解密的文件").size(16.0));
                    ui.add_space(4.0);
                    theme::hint(ui, "在本机完成解锁，零上传");
                });
            })
            .response;

        if response.interact(Sense::click()).clicked() {
            self.pick_files();
        }
    }

    fn file_grid(&mut self, ui: &mut egui::Ui) {
        if self.jobs.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(theme::weak_text("暂无文件，请先选择音乐文件"));
            });
            return;
        }

        self.list_toolbar(ui);
        ui.add_space(14.0);

        let mut actions = Vec::new();
        let columns = self.card_columns(ui.available_width());
        let jobs_len = self.jobs.len();

        let mut index = 0;
        while index < jobs_len {
            ui.horizontal_top(|ui| {
                for offset in 0..columns {
                    if index + offset >= jobs_len {
                        break;
                    }
                    if offset > 0 {
                        ui.add_space(CARD_GAP);
                    }
                    render_card(ui, &self.jobs[index + offset], &mut actions);
                }
            });
            index += columns;
            ui.add_space(CARD_GAP);
        }

        for action in actions {
            match action {
                Action::ToggleSelect(id) => {
                    if let Some(job) = self.jobs.iter_mut().find(|job| job.id == id) {
                        job.selected = !job.selected;
                    }
                }
                Action::Remove(id) => self.jobs.retain(|job| job.id != id),
                Action::OpenDir(path) => reveal_in_file_manager(&path),
            }
        }
    }

    fn card_columns(&self, available_width: f32) -> usize {
        let count = ((available_width + CARD_GAP) / (CARD_WIDTH + CARD_GAP)).floor();
        (count as usize).clamp(1, 3)
    }

    fn list_toolbar(&mut self, ui: &mut egui::Ui) {
        let counts = self.counts();
        let selected = self.jobs.iter().filter(|job| job.selected).count();
        let all_selected = selected == counts.total && counts.total > 0;

        ui.horizontal_wrapped(|ui| {
            let mut select_all = all_selected;
            if ui
                .checkbox(&mut select_all, if all_selected { "取消全选" } else { "全选" })
                .changed()
            {
                for job in &mut self.jobs {
                    job.selected = select_all;
                }
            }
            ui.label(theme::weak_text(format!("({selected}/{})", counts.total)));

            ui.add_space(10.0);

            ui.add_enabled_ui(counts.pending > 0, |ui| {
                if ui
                    .add(theme::primary_button(format!(
                        "开始解密（待处理 {}）",
                        counts.pending
                    )))
                    .clicked()
                {
                    self.start_decrypt();
                }
            });

            let has_selection = selected > 0;
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("移除选中").clicked() {
                    self.jobs.retain(|job| !job.selected);
                }
            });

            if ui.button("清空列表").clicked() {
                self.jobs.clear();
            }

            ui.add_space(10.0);
            if ui.button("添加文件").clicked() {
                self.pick_files();
            }
            if ui.button("添加文件夹").clicked() {
                if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                    self.add_folder(&dir);
                }
            }
        });
    }

    // —— 界面：设置页 ——

    fn settings_tab(&mut self, ui: &mut egui::Ui) {
        let full_height = ui.available_height();

        ui.horizontal_top(|ui| {
            egui::Frame::new()
                .fill(theme::BASE_200)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(150.0);
                    // 减去上下 inner margin，让底色铺满整个内容区高度
                    ui.set_min_height((full_height - 24.0).max(0.0));
                    for (page, name) in SettingsPage::ALL {
                        if theme::side_nav_item(ui, name, self.settings_page == *page) {
                            self.settings_page = *page;
                        }
                        ui.add_space(4.0);
                    }
                });

            ui.add_space(14.0);

            ui.vertical(|ui| {
                ui.add_space(4.0);
                let width = (ui.available_width() - 8.0).max(320.0);
                ui.set_width(width);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.settings_content(ui);
                });
            });
        });
    }

    fn settings_content(&mut self, ui: &mut egui::Ui) {
        match self.settings_page {
            SettingsPage::Home => {
                theme::heading(ui, "设置");
                ui.add_space(6.0);
                ui.label("在这里你可以设置应用的基本配置。");

                ui.add_space(14.0);
                theme::section_frame().show(ui, |ui| {
                    theme::sub_heading(ui, "输出");
                    ui.add_space(8.0);

                    ui.label("输出目录");
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 90.0).max(160.0);
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.settings.output_dir)
                                    .desired_width(width)
                                    .hint_text("留空则输出到源文件所在目录"),
                            )
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        if ui.button("浏览").clicked() {
                            if let Some(dir) = rfd::FileDialog::new().pick_folder() {
                                self.settings.output_dir = dir.display().to_string();
                                self.settings_dirty = true;
                            }
                        }
                    });

                    ui.add_space(8.0);
                    if ui
                        .checkbox(&mut self.settings.overwrite, "覆盖同名文件")
                        .changed()
                    {
                        self.settings_dirty = true;
                    }
                    ui.add_space(2.0);
                    theme::hint(
                        ui,
                        "未开启时，遇到同名文件会自动追加序号；本批次里的任何输入文件都不会被覆盖。",
                    );

                    ui.add_space(10.0);
                    theme::hint(ui, "密钥以明文保存在本机配置文件中，请不要把这个文件分享给别人。");
                });
            }

            SettingsPage::Qmc => {
                theme::heading(ui, "QMCv2 密钥");
                ui.add_space(6.0);
                ui.label("绝大多数 QMCv2 文件内嵌了 ekey，可以直接解密。");
                ui.label("只有内嵌 ekey 缺失时，才需要在这里手动填写。");

                ui.add_space(14.0);
                theme::section_frame().show(ui, |ui| {
                    theme::sub_heading(ui, "用户密钥（ekey）");
                    ui.add_space(8.0);
                    if text_field(ui, &mut self.settings.qmc2_key, true) {
                        self.settings_dirty = true;
                    }
                    ui.add_space(6.0);
                    theme::hint(ui, "填写后，解码时会优先尝试该密钥，再回退到文件内嵌的 ekey。");
                });

                ui.add_space(12.0);
                theme::section_frame().show(ui, |ui| {
                    theme::sub_heading(ui, "获取方式");
                    ui.add_space(6.0);
                    ui.label("· PC 客户端：仅支持 v19.43 或更低版本");
                    ui.label("· 安卓客户端：需要超级管理员权限提取密钥数据库");
                    ui.label("· iOS 客户端：需要越狱，或完整备份后提取密钥数据库");
                    ui.label("· Mac 客户端：需要导入密钥数据库");
                });
            }

            SettingsPage::Kwm => {
                theme::heading(ui, "KWMv2 密钥");
                ui.add_space(6.0);
                ui.label("旧版 KWMv1 不需要配置；KWMv2 必须提供 ekey，否则解密会失败。");

                ui.add_space(14.0);
                theme::section_frame().show(ui, |ui| {
                    theme::sub_heading(ui, "KWMv2 密钥（ekey）");
                    ui.add_space(8.0);
                    if text_field(ui, &mut self.settings.kwm2_key, true) {
                        self.settings_dirty = true;
                    }
                });
            }

            SettingsPage::Kgg => {
                theme::heading(ui, "KGG 密钥");
                ui.add_space(6.0);
                ui.label("酷狗 PC / 安卓客户端的 .kgg 曲库文件需要密钥数据库才能解密。");

                ui.add_space(14.0);
                theme::section_frame().show(ui, |ui| {
                    theme::sub_heading(ui, "密钥数据库文件");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let width = (ui.available_width() - 90.0).max(160.0);
                        if ui
                            .add(
                                egui::TextEdit::singleline(&mut self.settings.kugou_key)
                                    .desired_width(width)
                                    .hint_text("选择数据库文件路径"),
                            )
                            .changed()
                        {
                            self.settings_dirty = true;
                        }
                        if ui.button("浏览").clicked() {
                            if let Some(file) = rfd::FileDialog::new().pick_file() {
                                self.settings.kugou_key = file.display().to_string();
                                self.settings_dirty = true;
                            }
                        }
                    });
                    ui.add_space(6.0);
                    theme::hint(ui, "数据库本身是加密的，程序会自动解开后取用其中的密钥。");
                });
            }

            SettingsPage::Qtfm => {
                theme::heading(ui, "蜻蜓 FM");
                ui.add_space(6.0);
                ui.label("蜻蜓 FM 的 .qta 文件使用设备密钥加密，需要提供 16 字节密钥。");
                ui.add_space(4.0);
                ui.label("文件 IV 由文件名自动推导，无需填写。");

                ui.add_space(14.0);
                theme::section_frame().show(ui, |ui| {
                    theme::sub_heading(ui, "设备密钥");
                    ui.add_space(8.0);
                    if text_field(ui, &mut self.settings.qingting_key, true) {
                        self.settings_dirty = true;
                    }
                    ui.add_space(6.0);
                    theme::hint(ui, "填写十六进制字符串，共 32 个字符（16 字节）。");
                });
            }
        }

        ui.add_space(16.0);
        if ui
            .add_enabled(self.settings_dirty, theme::primary_button("保存设置"))
            .clicked()
        {
            self.save_settings();
        }
        if self.settings_dirty {
            ui.add_space(4.0);
            theme::hint(ui, "有未保存的改动");
        }
    }

    // —— 界面：答疑页 ——

    fn faq_tab(&mut self, ui: &mut egui::Ui) {
        let full_height = ui.available_height();

        ui.horizontal_top(|ui| {
            egui::Frame::new()
                .fill(theme::BASE_200)
                .inner_margin(12.0)
                .show(ui, |ui| {
                    ui.set_width(150.0);
                    // 减去上下 inner margin，让底色铺满整个内容区高度
                    ui.set_min_height((full_height - 24.0).max(0.0));
                    for (page, name) in FaqPage::ALL {
                        if theme::side_nav_item(ui, name, self.faq_page == *page) {
                            self.faq_page = *page;
                        }
                        ui.add_space(4.0);
                    }
                });

            ui.add_space(14.0);

            ui.vertical(|ui| {
                ui.add_space(4.0);
                let width = (ui.available_width() - 8.0).max(320.0);
                ui.set_width(width);
                faq::render(ui, self.faq_page);
            });
        });
    }

    // —— 底部 ——

    /// 汇总信息与整体进度。
    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let counts = self.counts();
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(format!(
                "共 {} 个文件 · 成功 {} · 失败 {}",
                counts.total, counts.done, counts.failed
            ));

            // 结束数除以总数就是整体进度；没有任何文件时不显示进度条
            if counts.total > 0 {
                let finished = counts.done + counts.failed;
                let fraction = finished as f32 / counts.total as f32;
                ui.add_space(10.0);
                ui.add(
                    egui::ProgressBar::new(fraction)
                        .desired_width(220.0)
                        .text(format!("{finished}/{}", counts.total)),
                );
            }

            if !self.message.is_empty() {
                ui.add_space(10.0);
                ui.label(theme::weak_text(&self.message));
            }
        });
        ui.add_space(4.0);
    }

    /// 版本与链接。
    fn footer(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                ui.label(theme::weak_text(format!(
                    "音乐解锁 · v{} · MIT",
                    env!("CARGO_PKG_VERSION")
                )));
                ui.label(theme::weak_text("·"));
                ui.hyperlink_to("lynvortex/unlock-music", crate::faq::REPO_URL);
            });
        });
        ui.add_space(6.0);
    }
}

impl eframe::App for UmApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_dropped_files(&ctx);

        // 有任务在跑时保持刷新，否则 100ms 一次足够
        if self.any_running() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
        self.collect_results();

        egui::Panel::top(egui::Id::new("top_tabs"))
            .show_separator_line(true)
            .show(ui, |ui| {
                ui.add_space(10.0);
                let current = match self.tab {
                    Tab::Home => 0,
                    Tab::Settings => 1,
                    Tab::Faq => 2,
                };
                if let Some(index) = theme::tab_bar(ui, TAB_NAMES, current) {
                    self.tab = match index {
                        0 => Tab::Home,
                        1 => Tab::Settings,
                        _ => Tab::Faq,
                    };
                }
                ui.add_space(8.0);
            });

        egui::Panel::bottom(egui::Id::new("footer")).show(ui, |ui| self.footer(ui));
        egui::Panel::bottom(egui::Id::new("status")).show(ui, |ui| self.status_bar(ui));

        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Home => self.home_tab(ui),
            Tab::Settings => self.settings_tab(ui),
            Tab::Faq => self.faq_tab(ui),
        });
    }
}

// —— 卡片 ——

fn render_card(ui: &mut egui::Ui, job: &Job, actions: &mut Vec<Action>) {
    theme::card_frame().show(ui, |ui| {
        ui.set_width(CARD_WIDTH);
        ui.set_min_height(150.0);

        // 顶部：选中
        ui.horizontal(|ui| {
            let mut selected = job.selected;
            if ui.checkbox(&mut selected, "").changed() {
                actions.push(Action::ToggleSelect(job.id));
            }
            theme::hint(ui, "选中");
        });

        ui.add_space(6.0);

        // 标题：文件名 + 扩展名徽章
        ui.horizontal(|ui| {
            let title = egui::Label::new(RichText::new(&job.file_name).strong())
                .truncate()
                .sense(Sense::hover());
            ui.add(title);
            if let JobState::Done { extension, .. } = &job.state {
                theme::badge(ui, extension, theme::ACCENT);
            }
        });

        ui.add_space(8.0);

        // 状态区
        match &job.state {
            JobState::Pending => {
                ui.label(theme::weak_text("排队中"));
            }
            JobState::Running(phase) => {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label(RichText::new(phase.label()).color(theme::INFO));
                });
            }
            JobState::Done {
                cipher, output, ..
            } => {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("完成").small().color(theme::SUCCESS).strong());
                    ui.label(theme::weak_text(format!("· {cipher}")).small());
                });
                ui.add_space(4.0);
                ui.label(theme::weak_text(output.display().to_string()).small());
            }
            JobState::Failed(message) => {
                egui::Frame::new()
                    .fill(theme::ERROR.gamma_multiply(0.10))
                    .stroke(Stroke::new(1.0, theme::ERROR.gamma_multiply(0.45)))
                    .corner_radius(CornerRadius::same(theme::RADIUS))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new("解密失败").small().color(theme::ERROR).strong());
                        ui.add_space(2.0);
                        ui.label(RichText::new(message).small());
                    });
            }
        }

        ui.add_space(10.0);

        // 操作行
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(theme::danger_button("删除")).clicked() {
                actions.push(Action::Remove(job.id));
            }
            if let JobState::Done { output, .. } = &job.state {
                if ui.add(theme::primary_button("打开所在目录")).clicked() {
                    actions.push(Action::OpenDir(output.clone()));
                }
            }
        });
    });
}

// —— 工具函数 ——

/// 单行文本输入框，返回内容是否被修改。
fn text_field(ui: &mut egui::Ui, value: &mut String, password: bool) -> bool {
    let width = ui.available_width();
    ui.add(
        egui::TextEdit::singleline(value)
            .password(password)
            .desired_width(width),
    )
    .changed()
}

/// 在文件管理器中定位文件。
fn reveal_in_file_manager(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        use std::process::Command;
        let _ = Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
    }
}

/// 递归收集目录下扩展名已知的文件。
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    const MAX_DEPTH: usize = 32;
    collect_files_at(dir, out, 0, MAX_DEPTH);
}

fn collect_files_at(dir: &Path, out: &mut Vec<PathBuf>, depth: usize, max_depth: usize) {
    // 深度上限：即使遇到自引用目录也不会无限递归
    if depth >= max_depth {
        return;
    }

    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // 用 entry.file_type()（不跟随链接）判断目录：
        // 跟随目录符号链接 / junction 可能成环，导致无限递归
        let is_real_dir = entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false);
        if is_real_dir {
            collect_files_at(&path, out, depth + 1, max_depth);
        } else if path.is_file() && has_known_extension(&path) {
            // path.is_file() 会跟随链接，所以指向文件的符号链接仍然会被收集
            out.push(path);
        }
    }
}

fn has_known_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let ext = ext.to_ascii_lowercase();
            KNOWN_EXTENSIONS.contains(&ext.as_str())
        })
        .unwrap_or(false)
}

#[derive(Default)]
struct Counts {
    total: usize,
    pending: usize,
    running: usize,
    done: usize,
    failed: usize,
}

/// 把解密出的临时文件重命名成最终输出文件，返回最终路径。
///
/// 两条硬性约束：
/// - 绝不覆盖 `protected`（本批次的全部输入文件）里的任何一个；
/// - 关闭「覆盖同名文件」时不覆盖任何已存在的文件，改为追加序号。
fn finalize_output(
    settings: &Settings,
    input: &Path,
    extension: &str,
    temp_path: &Path,
    protected: &[PathBuf],
) -> Result<PathBuf, String> {
    let stem = input
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".to_owned());

    // 临时文件就写在输出目录里，所以父目录即输出目录
    let dir = temp_path.parent().unwrap_or(Path::new("."));

    let mut target = dir.join(format!("{stem}.{extension}"));
    let hits_source = protected.iter().any(|source| same_path(source, &target));
    if !settings.overwrite || hits_source {
        let mut index = 1;
        while target.exists() {
            target = dir.join(format!("{stem} ({index}).{extension}"));
            index += 1;
        }
    }

    if let Err(error) = std::fs::rename(temp_path, &target) {
        // 重命名失败就把临时文件清掉，不留半个成品
        let _ = std::fs::remove_file(temp_path);
        return Err(format!("重命名输出失败：{error}"));
    }
    Ok(target)
}

/// 解析某个输入对应的输出目录。
fn resolved_output_dir(settings: &Settings, input: &Path) -> PathBuf {
    let configured = settings.output_dir.trim();
    if configured.is_empty() {
        input.parent().unwrap_or(Path::new(".")).to_path_buf()
    } else {
        PathBuf::from(configured)
    }
}

/// 归一化路径用于比较。
///
/// 目标文件可能还不存在，所以只解析其父目录（父目录一定存在），再拼回文件名。
fn normalize_path(path: &Path) -> PathBuf {
    let (dir, name) = match (path.parent(), path.file_name()) {
        (Some(dir), Some(name)) if !dir.as_os_str().is_empty() => (dir, Some(name)),
        _ => (path, None),
    };

    let base = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    match name {
        Some(name) => base.join(name),
        None => base,
    }
}

/// 判断两个路径是否指向同一个文件。
///
/// Windows 路径大小写不敏感，直接比较 `PathBuf` 会漏判（`Song.MP3` 与 `song.mp3`
/// 会被当成两个文件），所以统一转小写后比较。
fn same_path(a: &Path, b: &Path) -> bool {
    let a = normalize_path(a);
    let b = normalize_path(b);
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每个测试用独立的临时目录，避免相互干扰。
    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("um-gui-test-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("创建测试目录失败");
        dir
    }

    fn write(path: &Path, content: &str) {
        std::fs::write(path, content).expect("写测试文件失败");
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).expect("读测试文件失败")
    }

    #[test]
    fn never_overwrites_another_input_in_the_batch() {
        let dir = temp_dir("protect");

        // 用户自己的 a.flac，同时它也是本批的另一个输入
        let source = dir.join("a.flac");
        write(&source, "原始内容");

        // a.ncm 解密后的临时文件
        let temp = dir.join(".a.ncm.1.part");
        write(&temp, "解密结果");

        let settings = Settings {
            overwrite: true,
            ..Settings::default()
        };
        let out = finalize_output(
            &settings,
            &dir.join("a.ncm"),
            "flac",
            &temp,
            std::slice::from_ref(&source),
        )
        .expect("应当成功写出");

        assert_ne!(out, source, "不能写到输入文件上");
        assert_eq!(read(&source), "原始内容", "源文件必须原样保留");
        assert_eq!(read(&out), "解密结果");
    }

    #[test]
    fn appends_index_when_target_exists_and_overwrite_is_off() {
        let dir = temp_dir("no-overwrite");

        let existing = dir.join("song.flac");
        write(&existing, "已有文件");

        let temp = dir.join(".song.ncm.2.part");
        write(&temp, "新结果");

        let settings = Settings {
            overwrite: false,
            ..Settings::default()
        };
        let out = finalize_output(&settings, &dir.join("song.ncm"), "flac", &temp, &[])
            .expect("应当成功写出");

        assert_eq!(out.file_name().unwrap(), "song (1).flac");
        assert_eq!(read(&existing), "已有文件", "同名文件不能被改动");
    }

    #[test]
    fn replaces_existing_file_when_overwrite_is_on() {
        let dir = temp_dir("overwrite");

        let existing = dir.join("song.flac");
        write(&existing, "旧内容");

        let temp = dir.join(".song.ncm.3.part");
        write(&temp, "新内容");

        let settings = Settings {
            overwrite: true,
            ..Settings::default()
        };
        let out = finalize_output(&settings, &dir.join("song.ncm"), "flac", &temp, &[])
            .expect("应当成功写出");

        assert_eq!(out, existing);
        assert_eq!(read(&out), "新内容");
    }

    #[test]
    fn reports_error_instead_of_panicking_when_temp_is_missing() {
        let dir = temp_dir("rename-fail");
        let result = finalize_output(
            &Settings::default(),
            &dir.join("missing.ncm"),
            "flac",
            &dir.join(".missing.part"),
            &[],
        );
        assert!(result.is_err());
    }

    #[test]
    fn collects_files_from_nested_directories() {
        let dir = temp_dir("collect");
        let nested = dir.join("album").join("disc1");
        std::fs::create_dir_all(&nested).expect("创建测试目录失败");

        write(&dir.join("a.ncm"), "x");
        write(&nested.join("b.mflac"), "x");
        write(&nested.join("readme.txt"), "x");

        let mut found = Vec::new();
        collect_files(&dir, &mut found);
        found.sort();

        assert_eq!(found.len(), 2, "只收已知扩展名，且要能进子目录");
        assert!(found[0].ends_with("a.ncm"));
        assert!(found[1].ends_with("b.mflac"));
    }
}
