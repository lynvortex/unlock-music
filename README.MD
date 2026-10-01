# Unlock Music 音乐解锁（原生桌面版）

> 原生桌面音乐解锁工具，用 Rust 编写。
> 界面与解密调度不依赖浏览器；解密算法直接调用 `lib_um_crypto_rust`，不做二次实现。

在原机解锁加密的音乐文件，全部处理都在本机完成，不上传任何数据。

## 支持的格式

| 平台 | 扩展名 | 密钥 |
| --- | --- | --- |
| QQ 音乐 QMCv1 | `.qmc3` / `.qmcflac` 等 | 不需要 |
| QQ 音乐 QMCv2（PC） | `.mflac` / `.mgg` 等 | 需要 ekey |
| QQ 音乐 QMCv2（安卓） | `.mflac0` / `.mgg1` / `.mggl` 等 | 需要 ekey |
| QQ 音乐 QMCv2（iOS） | `.mgalaxy` 等 | 需要 ekey |
| QQ 音乐 QMCv2（Mac） | `.mflach` 等 | 需要 ekey |
| 网易云音乐 | `.ncm` | 不需要 |
| 虾米音乐 | `.xm` | 不需要 |
| 酷我音乐 | `.kwm` | KWMv2 需要 ekey |
| 酷狗音乐 | `.kgm` / `.vpr` | 不需要 |
| 酷狗音乐 | `.kgg` | 需要密钥数据库 |
| 喜马拉雅 | `.x2m` / `.x3m` / `.xm` | 不需要 |
| 咪咕音乐 | `.mg3d` | 自动猜测密钥 |
| 蜻蜓 FM | `.qta` | 需要设备密钥 |

> QQ 音乐 PC 客户端仅支持 v19.43 或更低版本；安卓 / iOS 需要先提取密钥数据库。
> QQ 音乐海外版 JOOX（`.ofl_en`）暂不支持。

## 项目结构

```
um-rs/
├── crates/
│   ├── um-core/   解密核心：格式识别、解密调度、各格式解密器
│   ├── um-cli/    命令行
│   └── um-gui/    原生桌面界面（eframe/egui）
└── vendor/
    └── lib_um_crypto_rust/   官方解密算法库（见下方说明）
```

`um-core` 不依赖浏览器、GUI 或文件系统，只做「字节进、字节出」的解密，因此 CLI 与 GUI 共用同一套实现。

## 构建与运行

需要 Rust 工具链（1.85 以上）。

```sh
cd um-rs

cargo run -p um-gui                      # 启动桌面界面
cargo build --release -p um-gui          # 构建发布版

cargo run -p um-cli -- decrypt <文件>     # 命令行解密
cargo run -p um-cli -- detect <文件>      # 只探测类型
cargo test                               # 运行全部测试
```

桌面界面支持拖放文件、批量选择、按需填写密钥；命令行支持多文件与各类密钥参数。

## 命令行参数

| 参数 | 说明 |
| --- | --- |
| `--qmc2-key` | QQ 音乐 QMCv2 用户密钥（ekey 字符串） |
| `--kwm2-key` | 酷我 KWMv2 密钥 |
| `--kugou-key` | 酷狗密钥数据库路径 |
| `--qingting-key` | 蜻蜓 FM 设备密钥（十六进制） |
| `-o, --output` | 输出目录，默认为源文件所在目录 |
| `-f, --force` | 输出文件已存在时覆盖 |

## 关于 vendor

`um-rs/vendor/lib_um_crypto_rust` 是解密算法库 `lib_um_crypto_rust` 的源码快照，
它是本项目的编译依赖，请勿删除。该库以 Apache-2.0 + MIT 双协议授权，
授权文件随源码一并保留（`LICENSE_APACHE` / `LICENSE_MIT`）。

为减小体积，本地对快照做了裁剪，仅保留解密算法相关 crate：

- 移除 `um_wasm`、`um_wasm_loader`、`um_cli`（分别是 WASM 导出层、npm 打包脚本、调试用 CLI）
- 移除 `um_crypto/joox`、`um_crypto/qrc`（本工具未支持这两种格式）
- 移除 `Dockerfile`、`build.sh`、`bump_versions.py` 等 WASM / npm 发布流水线脚本
- 同步更新了其 `Cargo.toml` 的 workspace 成员表

该库的来源与构建方式记录在 [docs/develop-with-um_crypto.zh.md](docs/develop-with-um_crypto.zh.md)。

## 授权

- 界面与解密调度：MIT，见 [LICENSE](LICENSE)
- 解密算法库 `lib_um_crypto_rust`：Apache-2.0 + MIT 双协议

本项目以学习和技术研究为目的创建，修改与再分发请遵循上述授权协议。

## 仓库说明

- 原有的 React / 浏览器版本已移除，源码可在 git 历史 `aa8bd88` 中找到。
- `docs/` 保留了迁移前的文档，其中 `getting-started.zh.md` 描述的是 web 版构建流程，已不再适用。
