//! 静态卫生门禁（只读源码，不连网、不写盘）。
//!
//! 覆盖项目的两条硬规则（见 AGENTS.md）：
//! 1. **数据写入红线**：KissLight 只写夜灯相关数据 —— 不写任何文件、不写夜灯之外的
//!    注册表键。本文件把「允许写什么」钉成可执行判据，而不是靠口头约定。
//! 2. **代码卫生**：不写死本机绝对路径、不留调试打印与未完成标记、指针类 unsafe
//!    必须带 `// SAFETY:`、`core/` 不许摸 `windows` crate 与 UI。
//!
//! 约定：生产代码在前、`#[cfg(test)]` 之后是测试区（测试区豁免）。
//! 只做字符串扫描，不解析语法 —— 判据简单，误报率才低。

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("读 {path:?} 失败：{error}"))
}

/// 递归收集指定后缀的文件。
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

fn src_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect(&repo_root().join("src"), "rs", &mut files);
    files.sort();
    assert!(!files.is_empty(), "src/ 下没有 .rs 文件——扫描路径失效");
    files
}

/// 生产代码行（跳过注释；遇到 `#[cfg(test)]` 之后的内容一律不返回）。
fn production_lines(text: &str) -> Vec<(usize, &str)> {
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.trim() == "#[cfg(test)]" {
            break;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with('*') {
            continue;
        }
        lines.push((index + 1, line));
    }
    lines
}

fn scan_src_for(needles: &[&str], allowed_file: Option<&str>) -> Vec<String> {
    let mut violations = Vec::new();
    for path in src_files() {
        let relative = path
            .strip_prefix(repo_root())
            .expect("src 文件应在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        if allowed_file == Some(relative.as_str()) {
            continue;
        }
        let text = read(&path);
        for (number, line) in production_lines(&text) {
            for needle in needles {
                if line.contains(needle) {
                    violations.push(format!("{relative}:{number}: {needle} —— {}", line.trim()));
                }
            }
        }
    }
    violations
}

/// 文件写入类 API：KissLight 不写任何文件（配置、日志、缓存都不写）。
#[test]
fn no_file_writes_anywhere() {
    let violations = scan_src_for(
        &[
            "fs::write",
            "fs::create_dir",
            "fs::remove_file",
            "fs::remove_dir",
            "fs::rename",
            "fs::copy",
            "fs::hard_link",
            "File::create",
            "OpenOptions",
            "write_all",
            "set_len",
        ],
        None,
    );
    assert!(
        violations.is_empty(),
        "KissLight 不写文件（不落配置/日志/缓存），请去掉这些调用：\n  {}\n\
         需要保存状态时走夜灯注册表（core::nightlight），写入清单见 README「只写夜灯相关数据」。",
        violations.join("\n  ")
    );
}

/// 注册表写入只允许出现在 core/gamma.rs（HKLM 的 GdiICMGammaRange）。
#[test]
fn registry_writes_only_in_gamma() {
    let violations = scan_src_for(
        &[
            "RegCreateKeyExW",
            "RegSetValueExW",
            "RegDeleteValueW",
            "RegDeleteKeyW",
            "RegSetKeyValueW",
            "SHSetValueW",
        ],
        Some("src/core/gamma.rs"),
    );
    assert!(
        violations.is_empty(),
        "注册表写入只允许出现在 src/core/gamma.rs（GdiICMGammaRange）；\
         夜灯数据必须经 win-nightlight-lib 写：\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_debug_prints() {
    let violations = scan_src_for(
        &["println!", "eprintln!", "print!", "eprint!", "dbg!"],
        None,
    );
    assert!(
        violations.is_empty(),
        "生产代码里不要留调试打印：\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn no_unfinished_markers() {
    // 未完成标记只出现在注释里，所以这一查**不跳过**注释行（其余检查都跳过）。
    let mut violations = Vec::new();
    for path in src_files() {
        let relative = path
            .strip_prefix(repo_root())
            .expect("src 文件应在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        let text = read(&path);
        for (number, line) in text.lines().enumerate() {
            if line.trim() == "#[cfg(test)]" {
                break;
            }
            if ["TODO:", "FIXME:", "XXX:"]
                .iter()
                .any(|marker| line.contains(marker))
            {
                violations.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "生产代码里不要留未完成标记：\n  {}",
        violations.join("\n  ")
    );
}

/// `core/` 是纯逻辑层：不许用 `windows` crate，也不许知道 UI / 装配层。
#[test]
fn core_stays_pure() {
    let mut violations = Vec::new();
    let mut files = Vec::new();
    collect(&repo_root().join("src/core"), "rs", &mut files);
    for path in files {
        let relative = path
            .strip_prefix(repo_root())
            .expect("core 文件应在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        let text = read(&path);
        for (number, line) in production_lines(&text) {
            for needle in ["use windows::", "use crate::ui", "use crate::app"] {
                if line.contains(needle) {
                    violations.push(format!("{relative}:{number}: {needle}"));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "core/ 必须保持纯逻辑（不引用 windows crate / ui / app）：\n  {}",
        violations.join("\n  ")
    );
}

/// 指针类 unsafe 必须写 `// SAFETY:`（本行或上方 4 行内）。
#[test]
fn pointer_unsafe_has_safety_comment() {
    const POINTER_OPS: &[&str] = &[
        "from_raw(",
        "from_raw_parts",
        "transmute",
        "get_unchecked",
        "copy_nonoverlapping",
        "read_unaligned",
        "write_unaligned",
    ];
    let mut violations = Vec::new();
    for path in src_files() {
        let relative = path
            .strip_prefix(repo_root())
            .expect("src 文件应在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        let text = read(&path);
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue;
            }
            if !POINTER_OPS.iter().any(|op| line.contains(op)) {
                continue;
            }
            let start = index.saturating_sub(4);
            let documented = lines[start..=index]
                .iter()
                .any(|candidate| candidate.contains("SAFETY"));
            if !documented {
                violations.push(format!("{relative}:{}: {}", index + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "指针类 unsafe 必须写 `// SAFETY:`（凭什么认为越界/对齐/生命周期/别名没问题）：\n  {}",
        violations.join("\n  ")
    );
}

/// 仓库任何文本文件都不得出现本机绝对路径或用户目录路径。
///
/// 判据是**通用的**：源码里不写任何本机目录名/用户名，避免「判据自己就是泄漏」。
/// 命中三类：
/// 1. 盘符绝对路径（盘符 + 冒号 + 反斜杠）；
/// 2. 用户目录路径（`Users` + 分隔符 + 非空名字；Windows 与 WSL/Git-Bash 两种写法都命中）；
/// 3. 本仓库自身的绝对路径（运行时由 CARGO_MANIFEST_DIR 推出，两种斜杠形式）。
#[test]
fn no_local_paths_or_fingerprints() {
    let root = repo_root();
    let local_roots = local_repo_forms();

    let mut files = Vec::new();
    collect_all_files(&root, &mut files);
    files.sort();

    let mut violations = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .expect("文件应在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with(".git/") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue; // 二进制文件不在本查范围
        };
        for (number, line) in text.lines().enumerate() {
            let lowered = line.to_lowercase();
            if has_drive_absolute_path(line)
                || has_profile_path(line)
                || local_roots.iter().any(|form| lowered.contains(form))
            {
                violations.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "仓库里不得出现本机绝对路径/用户目录路径（换台电脑就失效，且会泄露开发机目录）：
  {}
         改用相对路径或 CARGO_MANIFEST_DIR 派生路径。",
        violations.join(
            "
  "
        )
    );
}

/// 递归收集所有文件（`.git` 目录跳过，调用方按相对路径再筛一次）。
fn collect_all_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) != Some(".git") {
                collect_all_files(&path, out);
            }
        } else {
            out.push(path);
        }
    }
}

/// 本仓库绝对路径的两种写法（小写，便于大小写无关比对）。
fn local_repo_forms() -> Vec<String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut forms = Vec::new();
    for path in [Some(manifest), manifest.parent()].into_iter().flatten() {
        if let Some(text) = path.to_str() {
            forms.push(text.to_lowercase());
            forms.push(text.replace('\\', "/").to_lowercase());
        }
    }
    forms
}

/// 行内是否含「盘符 + 冒号 + 反斜杠」的绝对路径（排除 `https://` 这类）。
fn has_drive_absolute_path(line: &str) -> bool {
    let bytes = line.as_bytes();
    for index in 0..bytes.len().saturating_sub(2) {
        let is_drive = bytes[index].is_ascii_alphabetic()
            && bytes[index + 1] == b':'
            && bytes[index + 2] == b'\\';
        if !is_drive {
            continue;
        }
        let starts_path = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        if starts_path {
            return true;
        }
    }
    false
}

/// 行内是否含用户目录路径：`Users` + 分隔符 + 非空名字。
///
/// 名字到下一个分隔符/引号/空白为止；占位词（后面没有名字）不命中。
fn has_profile_path(line: &str) -> bool {
    const MARKER: &str = "Users";
    let bytes = line.as_bytes();
    let mut from = 0usize;
    while let Some(offset) = line[from..].find(MARKER) {
        let at = from + offset;
        let starts_word = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        if starts_word {
            let rest = &line[at + MARKER.len()..];
            let mut chars = rest.chars();
            let separator = chars.next();
            if separator == Some('\\') || separator == Some('/') {
                let name: String = chars
                    .take_while(|c| {
                        !c.is_whitespace()
                            && *c != '\\'
                            && *c != '/'
                            && !matches!(c, ')' | ']' | ',' | ';' | '|')
                    })
                    .collect();
                if !name.is_empty() {
                    return true;
                }
            }
        }
        from = at + MARKER.len();
    }
    false
}

/// 源码/文档里不得出现凭据样式的字符串（令牌、私钥、云厂商 Access Key）。
///
/// 判据只认「一眼就是凭据」的前缀；本文件是标记串的定义处，豁免。
#[test]
fn no_credential_like_strings() {
    const MARKERS: &[&str] = &["oauth2:", "ghp_", "github_pat_", "-----BEGIN", "AKIA"];
    let root = repo_root();
    let mut files = Vec::new();
    collect_all_files(&root, &mut files);
    files.sort();

    let mut violations = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(&root)
            .expect("文件应在仓库内")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.starts_with(".git/") || relative == "tests/hygiene.rs" {
            continue; // 本文件是标记串的定义处
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (number, line) in text.lines().enumerate() {
            if MARKERS.iter().any(|marker| line.contains(marker)) {
                violations.push(format!("{relative}:{}: {}", number + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "仓库里不得出现令牌/私钥样式的字符串（它们只能待在凭据管理器里）：
  {}",
        violations.join(
            "
  "
        )
    );
}

/// 判据自检：样本在运行时拼出来，源码里不出现真实路径或用户名。
#[test]
fn local_path_detectors_are_alive() {
    let drive_sample: String = ['C', ':', '\\', 'A', 'p', 'p'].iter().collect();
    assert!(has_drive_absolute_path(&drive_sample));
    assert!(!has_drive_absolute_path(
        "https://gitee.com/qiuzongman/kiss-light"
    ));
    let registry_sample = format!("HKLM{}Software{}Microsoft", '\\', '\\');
    assert!(!has_drive_absolute_path(&registry_sample));
    assert!(!has_drive_absolute_path("时间 12:30 出发"));

    let windows_profile = format!("{}{}someone{}notes", "Users", '\\', '\\');
    let wsl_profile = format!("{}{}someone", "Users", '/');
    assert!(has_profile_path(&windows_profile));
    assert!(has_profile_path(&wsl_profile));
    assert!(!has_profile_path("Users 是复数"));
    let not_a_word = format!("SuperUsers{}x", '/');
    assert!(!has_profile_path(&not_a_word));
}

/// 夜灯写入必须经核心库：我们的代码里不该出现 CloudStore 键路径的硬编码。
#[test]
fn nightlight_paths_come_from_the_library() {
    let violations = scan_src_for(&["CloudStore", "bluelightreduction"], None);
    assert!(
        violations.is_empty(),
        "夜灯注册表路径由 win-nightlight-lib 提供，不要在源码里硬编码：\n  {}",
        violations.join("\n  ")
    );
}
