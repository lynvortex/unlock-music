//! 答疑页内容。
//!
//! 包含 QQ 音乐 / 酷我音乐 / 酷狗音乐 / 安卓模拟器 / 其它问题 / 关于项目 六页，
//! 内容以本仓库 README 与 `docs/` 为依据。
//!
//! 密钥提取类页面只保留可核对的结论性说明，不搬运分步截图。

use eframe::egui;
use egui::RichText;

use crate::theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaqPage {
    Home,
    QqMusic,
    Kuwo,
    Kugou,
    AndroidEmulator,
    Other,
    About,
}

impl FaqPage {
    /// 侧栏顺序。
    pub const ALL: &'static [(FaqPage, &'static str)] = &[
        (FaqPage::Home, "答疑"),
        (FaqPage::QqMusic, "QQ 音乐"),
        (FaqPage::Kuwo, "酷我音乐"),
        (FaqPage::Kugou, "酷狗音乐"),
        (FaqPage::AndroidEmulator, "安卓模拟器"),
        (FaqPage::Other, "其它问题"),
        (FaqPage::About, "关于项目"),
    ];
}

/// 项目仓库地址。
pub const REPO_URL: &str = "https://github.com/lynvortex/unlock-music";
/// 交流群。
const CHAT_URL: &str = "https://t.me/unlock_music_chat";

pub fn render(ui: &mut egui::Ui, page: FaqPage) {
    egui::ScrollArea::vertical().show(ui, |ui| match page {
        FaqPage::Home => home(ui),
        FaqPage::QqMusic => qq_music(ui),
        FaqPage::Kuwo => kuwo(ui),
        FaqPage::Kugou => kugou(ui),
        FaqPage::AndroidEmulator => android_emulator(ui),
        FaqPage::Other => other(ui),
        FaqPage::About => about(ui),
    });
}

fn home(ui: &mut egui::Ui) {
    theme::heading(ui, "答疑");
    ui.add_space(6.0);
    ui.label("从左侧选择一项，查看相关说明。");
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("也欢迎造访");
        ui.hyperlink_to("「音乐解锁-交流」交流群", CHAT_URL);
        ui.label("进行交流。");
    });
}

/// 各格式与所需密钥的对照表。数据来自 README 的支持列表。
fn format_table(ui: &mut egui::Ui) {
    egui::Grid::new("format_support")
        .striped(true)
        .num_columns(3)
        .spacing([16.0, 6.0])
        .show(ui, |ui| {
            ui.strong("平台");
            ui.strong("扩展名");
            ui.strong("是否需要密钥");
            ui.end_row();

            for (platform, ext, key) in [
                ("QQ 音乐 QMCv1", ".qmc3 / .qmcflac 等", "不需要"),
                ("QQ 音乐 QMCv2（PC）", ".mflac / .mgg 等", "需要 ekey"),
                ("QQ 音乐 QMCv2（安卓）", ".mflac0 / .mgg1 / .mggl 等", "需要 ekey"),
                ("QQ 音乐 QMCv2（iOS）", ".mgalaxy 等", "需要 ekey"),
                ("QQ 音乐 QMCv2（Mac）", ".mflach 等", "需要 ekey"),
                ("网易云音乐", ".ncm", "不需要"),
                ("虾米音乐", ".xm", "不需要"),
                ("酷我音乐", ".kwm", "KWMv2 需要 ekey"),
                ("酷狗音乐", ".kgm / .vpr", "不需要"),
                ("酷狗音乐", ".kgg", "需要密钥数据库"),
                ("喜马拉雅", ".x2m / .x3m / .xm", "不需要"),
                ("咪咕音乐", ".mg3d", "不需要（自动猜密钥）"),
                ("蜻蜓 FM", ".qta", "需要设备密钥"),
            ] {
                ui.label(platform);
                ui.label(ext);
                ui.label(key);
                ui.end_row();
            }
        });
}

fn qq_music(ui: &mut egui::Ui) {
    theme::heading(ui, "QQ 音乐");
    ui.add_space(6.0);
    ui.label("QMCv1 使用固定密钥，直接解密即可，不需要任何额外配置。");
    ui.add_space(8.0);
    theme::sub_heading(ui, "QMCv2");
    ui.label("QMCv2 的文件里通常内嵌了加密后的 ekey，绝大多数情况可以直接解密。");
    ui.label("只有在内嵌 ekey 缺失时，才需要你在「设置 → QMCv2 密钥」里手动填写。");
    ui.add_space(8.0);

    theme::section_frame().show(ui, |ui| {
        ui.label(RichText::new("按平台获取 ekey").strong());
        ui.add_space(4.0);
        ui.label("· PC 客户端：仅支持 v19.43 或更低版本");
        ui.label("· 安卓客户端：需要超级管理员权限提取密钥数据库");
        ui.label("· iOS 客户端：需要越狱，或对设备做完整备份后提取密钥数据库");
        ui.label("· Mac 客户端：需要导入密钥数据库");
    });

    ui.add_space(8.0);
    theme::hint(ui, "提取密钥数据库的具体步骤请参考仓库文档，本页不重复展开。");
}

fn kuwo(ui: &mut egui::Ui) {
    theme::heading(ui, "酷我音乐");
    ui.add_space(6.0);
    ui.label("KWMv1（旧版 .kwm）由文件头里的资源 ID 推导密钥，不需要配置。");
    ui.add_space(6.0);
    ui.label("KWMv2 需要 ekey，请在「设置 → KWMv2 密钥」里填写；不填的话这类文件会解密失败。");
    ui.add_space(8.0);
    theme::hint(
        ui,
        "酷我安卓客户端的密钥通常保存在应用的 MMKV 数据里，需要先取出该文件。",
    );
}

fn kugou(ui: &mut egui::Ui) {
    theme::heading(ui, "酷狗音乐");
    ui.add_space(6.0);
    ui.label("KGM / VPR 由文件头推导密钥，不需要配置。");
    ui.add_space(6.0);
    ui.label("KGG（PC / 安卓客户端的曲库文件）需要提供密钥数据库，否则无法解密。");
    ui.add_space(8.0);
    theme::section_frame().show(ui, |ui| {
        ui.label(RichText::new("密钥数据库").strong());
        ui.add_space(4.0);
        ui.label("在「设置 → KGG 密钥」里填写密钥数据库文件的路径。");
        ui.label("数据库本身是加密的，程序会自动解开后取用其中的密钥。");
    });
}

fn android_emulator(ui: &mut egui::Ui) {
    theme::heading(ui, "安卓模拟器");
    ui.add_space(6.0);
    ui.label("在电脑上用安卓模拟器安装音乐客户端，比在真机上折腾权限要方便得多。");
    ui.add_space(8.0);
    ui.label("整体思路：");
    ui.add_space(4.0);
    ui.label("1. 在模拟器里安装并登录对应的音乐客户端");
    ui.add_space(2.0);
    ui.label("2. 用 ADB 把应用的数据目录整体拉取到电脑上");
    ui.add_space(2.0);
    ui.label("3. 从中找到密钥数据库或 MMKV 文件，在「设置」里导入");
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("仓库里有现成的 ADB 拉取脚本，详见");
        ui.hyperlink_to("项目文档", REPO_URL);
        ui.label("。");
    });
}

fn other(ui: &mut egui::Ui) {
    theme::heading(ui, "其它问题");
    ui.add_space(6.0);
    theme::sub_heading(ui, "支持的格式");
    ui.add_space(4.0);
    format_table(ui);

    ui.add_space(14.0);
    theme::sub_heading(ui, "不支持");
    ui.label("· QQ 音乐海外版 JOOX（.ofl_en）暂不支持");

    ui.add_space(14.0);
    theme::sub_heading(ui, "解密失败了怎么办");
    ui.add_space(4.0);
    ui.label("文件卡片上会显示是哪个解密器认领了这个文件，以及失败原因。");
    ui.label("需要密钥的格式（QMCv2 / KWMv2 / KGG / 蜻蜓 FM）缺少密钥时，提示会直接说明。");
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("如果遇到不支持的格式，可以带上样本与客户端版本信息反馈到");
        ui.hyperlink_to("GitHub Issues", format!("{REPO_URL}/issues"));
        ui.label("。");
    });
}

fn about(ui: &mut egui::Ui) {
    theme::heading(ui, "关于项目");
    ui.add_space(6.0);
    ui.label("原生桌面音乐解锁工具，全部处理都在本机完成：");
    ui.add_space(4.0);
    ui.label("· 界面与解密调度使用 Rust 编写，不依赖浏览器");
    ui.label("· 解密算法直接调用 lib_um_crypto_rust，不做二次实现");
    ui.label("· 解密过程完全在本机进行，不上传任何文件");

    ui.add_space(14.0);
    theme::sub_heading(ui, "授权");
    ui.add_space(4.0);
    ui.label("· 界面与解密调度：MIT");
    ui.label("· 解密算法库 lib_um_crypto_rust：Apache-2.0 + MIT 双协议");

    ui.add_space(14.0);
    theme::sub_heading(ui, "链接");
    ui.add_space(4.0);
    ui.hyperlink_to("lynvortex/unlock-music", REPO_URL);
    ui.hyperlink_to("音乐解锁交流群", CHAT_URL);

    ui.add_space(14.0);
    theme::section_frame().show(ui, |ui| {
        ui.label(
            RichText::new("本项目以学习和技术研究为目的创建，修改与再分发请遵循上述授权协议。")
                .color(theme::BASE_CONTENT.gamma_multiply(0.75)),
        );
    });
}
