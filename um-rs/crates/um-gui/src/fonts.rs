use eframe::egui;
use std::path::Path;
use std::sync::Arc;

/// 候选字体，按优先级排列。
///
/// egui 自带字体不含中文，不加载的话界面上的中文会显示成方块。
/// 优先选纯 TTF（TTC 字体集合的支持要依赖底层字体库，不保证可用）。
const CANDIDATES: &[&str] = &[
    "Deng.ttf",   // 等线
    "simhei.ttf", // 黑体
    "msyh.ttc",   // 微软雅黑
    "simsun.ttc", // 宋体
];

/// 从系统字体目录加载一个中文字体并注册到 egui。
///
/// 找不到时只打印警告：界面仍可用，但中文可能显示为方块。
pub fn install(ctx: &egui::Context) {
    let Some((name, bytes)) = load() else {
        eprintln!("[warn] 未在系统字体目录找到中文字体，界面中文可能显示异常");
        return;
    };

    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(name.clone(), Arc::new(egui::FontData::from_owned(bytes)));

    // 插到最前面：中文优先用这个字体，其余字符回落到 egui 自带的字体
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts.families.entry(family).or_default().insert(0, name.clone());
    }

    ctx.set_fonts(fonts);
}

fn load() -> Option<(String, Vec<u8>)> {
    let windir = std::env::var_os("WINDIR")?;
    let dir = Path::new(&windir).join("Fonts");

    CANDIDATES.iter().find_map(|file| {
        std::fs::read(dir.join(file))
            .ok()
            .map(|bytes| ((*file).to_owned(), bytes))
    })
}
