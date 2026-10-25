//! 文档与版本一致性门禁。
//!
//! KissLight 是一个小工具，文档只保留 4 份（README / HISTORY / AGENTS / 第三方声明）。
//! 本文件把这件事钉住，并核对版本号、许可与发布目录——都是「不检查就会悄悄腐化」的事实：
//!
//! 1. 受控文档存在且非空；
//! 2. 仓库里不允许出现别的 `.md`（防止文档再次膨胀，例如给 assets/ 加 README）；
//! 3. 版本号在 Cargo.toml / app.manifest / build.rs / README（下载链接）/ HISTORY 五处一致，
//!    关于对话框的标题取自 `CARGO_PKG_VERSION`（不手写版本）；
//! 4. `LICENSE` 是 AGPL-3.0 全文；`licenses/` 里的 MIT 原文与 `.3rd` 逐字一致；
//! 5. 已部署的 `.build/KissLight.exe` 与两份许可齐备且与正本一致；
//! 6. 源码/构建脚本引用的 `assets/` 文件真的存在。

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn project_root() -> PathBuf {
    repo_root()
        .parent()
        .expect("仓库的上一级是项目根")
        .to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("读 {path:?} 失败：{error}"))
}

fn cargo_package_version() -> String {
    let manifest = read(&repo_root().join("Cargo.toml"));
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package {
            if let Some(rest) = trimmed.strip_prefix("version = ") {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    panic!("Cargo.toml 的 [package] 段里没有 version");
}

fn collect(dir: &Path, extension: &str, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, extension, out);
        } else if path.extension().and_then(|value| value.to_str()) == Some(extension) {
            out.push(path);
        }
    }
}

#[test]
fn controlled_documents_exist() {
    let root = repo_root();
    for relative in [
        "README.md",
        "HISTORY.md",
        "AGENTS.md",
        "THIRD-PARTY-NOTICES.md",
    ] {
        let path = root.join(relative);
        assert!(path.is_file(), "受控文档缺失：{relative}");
        assert!(
            read(&path).lines().count() > 10,
            "{relative} 内容过短——空文档会在核对时制造「已检查」的错觉"
        );
    }
}

/// 文档预算：仓库里只允许这 4 份 markdown。
///
/// 想加文档先问一句「它比 README 多解决了什么」；能并进 README/AGENTS 的就别新开文件。
#[test]
fn repository_holds_only_four_markdown_files() {
    let root = repo_root();
    let mut files = Vec::new();
    collect(&root, "md", &mut files);
    const ALLOWED: &[&str] = &[
        "README.md",
        "HISTORY.md",
        "AGENTS.md",
        "THIRD-PARTY-NOTICES.md",
    ];
    let mut unexpected = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .expect("文件在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with(".git/") {
            continue;
        }
        if !ALLOWED.contains(&relative.as_str()) {
            unexpected.push(relative);
        }
    }
    assert!(
        unexpected.is_empty(),
        "仓库里出现了预算外的文档：{unexpected:?}\n\
         本项目只保留 README / HISTORY / AGENTS / THIRD-PARTY-NOTICES 四份；\
         能并进去的就别新开文件。"
    );
}

#[test]
fn version_is_consistent_everywhere() {
    let version = cargo_package_version();
    assert!(
        version.split('.').count() == 3,
        "版本号应是 x.y.z：{version}"
    );

    let checks: [(&str, String); 5] = [
        ("app.manifest", format!("version=\"{version}.0\"")),
        ("build.rs", format!("\"ProductVersion\", \"{version}\"")),
        ("build.rs", format!("\"FileVersion\", \"{version}\"")),
        ("README.md", format!("releases/download/{version}/")),
        ("HISTORY.md", format!("## v{version} (")),
    ];
    for (relative, needle) in checks {
        let body = read(&repo_root().join(relative));
        assert!(
            body.contains(&needle),
            "{relative} 里找不到 {needle:?}——版本号必须与 Cargo.toml 的 {version} 同步"
        );
    }

    // 关于对话框不手写版本，避免成为第六处需要同步的地方。
    let about = read(&repo_root().join("src/ui/about.rs"));
    assert!(
        about.contains("env!(\"CARGO_PKG_VERSION\")"),
        "关于对话框的标题必须取自 CARGO_PKG_VERSION，不要手写版本号"
    );
}

/// AGPL-3.0 第 11 条（专利）的官方措辞；许可文本被改写时最先在这里现形。
const AGPL3_S11_FINGERPRINT: &str =
    "you convey, or propagate by procuring conveyance of, a covered work";

#[test]
fn license_is_agpl3_full_text() {
    let path = repo_root().join("LICENSE");
    let raw = std::fs::read(&path).expect("缺少 LICENSE——AGPL 要求随二进制分发许可全文");
    assert!(
        !raw.starts_with(&[0xEF, 0xBB, 0xBF]),
        "LICENSE 带 UTF-8 BOM——许可文本应是无 BOM 纯文本"
    );
    let body = String::from_utf8(raw).expect("LICENSE 不是合法 UTF-8");
    let flat = body.split_whitespace().collect::<Vec<_>>().join(" ");
    for needle in [
        "GNU AFFERO GENERAL PUBLIC LICENSE",
        "Version 3, 19 November 2007",
        "END OF TERMS AND CONDITIONS",
        AGPL3_S11_FINGERPRINT,
    ] {
        assert!(
            flat.contains(needle),
            "LICENSE 缺少 AGPL-3.0 官方锚点 {needle:?}——许可被改写或不是全文"
        );
    }
}

#[test]
fn third_party_notices_and_licence_text_are_present() {
    let root = repo_root();
    let notices = read(&root.join("THIRD-PARTY-NOTICES.md"));
    for needle in [
        "win-nightlight-cli",
        "win-nightlight-lib",
        "MIT",
        "AGPL-3.0",
    ] {
        assert!(
            notices.contains(needle),
            "THIRD-PARTY-NOTICES.md 未提到 {needle:?}"
        );
    }

    let shipped = root.join("licenses/win-nightlight-lib-MIT.txt");
    assert!(
        shipped.is_file(),
        "缺少 licenses/win-nightlight-lib-MIT.txt"
    );
    let text = read(&shipped);
    assert!(text.contains("MIT License"), "第三方许可文件不是 MIT 全文");
    assert!(text.lines().count() > 15, "第三方许可文件疑似被截断");

    // 源码还在时逐字比对；.3rd 被腾空则跳过（发布目录里的副本仍然必须在）。
    let source = project_root().join(".3rd/win-nightlight-cli-0.5.0/LICENSE");
    if source.is_file() {
        assert_eq!(
            std::fs::read(&shipped).expect("读 licenses 副本失败"),
            std::fs::read(&source).expect("读 .3rd 许可源失败"),
            "licenses/ 下的 MIT 副本与 .3rd 原文不一致——许可文本必须逐字"
        );
    }
}

/// 已部署时（`../.build` 存在），发布目录必须齐备且与正本一致。
#[test]
fn deployed_bundle_is_complete() {
    let deploy = project_root().join(".build");
    if !deploy.is_dir() {
        return; // 还没部署过，不做判据
    }

    let exe = deploy.join("KissLight.exe");
    assert!(
        exe.is_file(),
        "发布目录缺少 {}——程序产物固定叫 KissLight.exe",
        exe.display()
    );

    let shipped_license = deploy.join("LICENSE");
    assert!(shipped_license.is_file(), "发布目录缺少 LICENSE");
    assert_eq!(
        std::fs::read(&shipped_license).expect("读发布版 LICENSE 失败"),
        std::fs::read(repo_root().join("LICENSE")).expect("读正本 LICENSE 失败"),
        "发布目录的 LICENSE 与正本不一致——请重新运行 build/build.ps1"
    );

    let shipped_third_party = deploy.join("LICENSE-win-nightlight-lib");
    assert!(shipped_third_party.is_file(), "发布目录缺少第三方许可原文");
    assert_eq!(
        std::fs::read(&shipped_third_party).expect("读发布版第三方许可失败"),
        std::fs::read(repo_root().join("licenses/win-nightlight-lib-MIT.txt"))
            .expect("读第三方许可正本失败"),
        "发布目录的第三方许可与 licenses/ 正本不一致"
    );
}

#[test]
fn assets_referenced_by_code_exist() {
    let root = repo_root();
    for asset in [
        "assets/logo.png",
        "assets/WeChatPay.png",
        "assets/ALiPay.png",
    ] {
        assert!(root.join(asset).is_file(), "缺少资源文件 {asset}");
    }
    assert!(
        read(&root.join("build.rs")).contains("assets/logo.png"),
        "build.rs 应从 assets/logo.png 生成图标"
    );
    let about = read(&root.join("src/ui/about.rs"));
    for needle in ["assets/WeChatPay.png", "assets/ALiPay.png"] {
        assert!(about.contains(needle), "关于对话框应引用 {needle}");
    }
}
