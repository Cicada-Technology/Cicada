//! `cicada new`：把 `template/` 渲染成**使用者自己的工程**（–2）。
//!
//! 刻意不引入 cargo-generate 作为运行时依赖：渲染规则很小（五个占位符 + 不落地模板
//! 自身的元数据文件），自己实现比要求用户先装一个工具更可靠，也让 `cicada new` 可以
//! 在任何环境下失败关闭地渲染。
//! `template/cargo-generate.toml` 仍然保留，装了 cargo-generate 的人也可以走
//! `cargo generate --path template`，两条路产出的文件一致。
//!
//! 渲染是**失败关闭**的：替换完仍残留 `{{...}}` 占位符（模板改了但渲染器没跟上）
//! 会直接报错，不做静默的半成品工程。

use std::path::PathBuf;

use crate::flag;

/// 默认 cicada 依赖来源：git tag。用 `--cicada` 换成本地工作区（`path=<工作区根>`）或别的 tag。
const DEFAULT_CICADA_DEP: &str =
    r#"git = "https://github.com/Cicada-Technology/cicada", tag = "v0.1.0""#;

/// 模板里直接引用 crate 名的那几个依赖对应的目录（相对工作区根）。
///
/// `path=` 模式必须逐 crate 给路径：cargo 的路径依赖要求指向**包目录**，
/// 而工作区根是虚拟清单（只对 git 依赖会自动在工作区成员里查找）。
const LAYER_DIRS: [(&str, &str); 4] = [
    ("cicada_kernel_dep", "crates/kernel"),
    ("cicada_macros_dep", "crates/macros"),
    ("cicada_audit_dep", "crates/audit"),
    ("cicada_web_dep", "crates/web"),
];

/// 模板目录：编译期定位到本仓库的 `template/`，也可运行时用 `--template` 覆盖。
const DEFAULT_TEMPLATE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../template");

/// 模板里不该出现在使用者工程里的文件。
const SKIP: [&str; 1] = ["cargo-generate.toml"];

/// 前端骨架里不进入生成工程的重目录（依赖与构建产物）。
const WEB_SKIP_DIRS: [&str; 2] = ["node_modules", "dist"];

/// Tauri 客户端里不进入生成工程的重目录（依赖、Rust target 与生成 schema/工程）。
/// `gen/` 里是 Tauri 按本仓库标识生成的 Android/iOS 工程，新工程必须用自己的标识重新 init。
const APP_SKIP_DIRS: [&str; 3] = ["node_modules", "target", "gen"];

#[derive(Debug)]
pub struct NewOptions {
    pub dir: PathBuf,
    pub name: String,
    /// 占位符 → 值：`project-name` 与五个 cicada 依赖来源。
    pub vars: Vec<(&'static str, String)>,
    pub template_dir: PathBuf,
}

impl NewOptions {
    /// 组装依赖来源占位符。
    ///
    /// `--cicada path=<工作区根>` 走本地路径（逐 crate 指到包目录），
    /// 其余写法（`git = ..., tag = ...` 等 TOML 片段）五处统一使用。
    fn deps(spec: &str) -> Vec<(&'static str, String)> {
        let mut vars: Vec<(&'static str, String)> = Vec::new();
        match spec.strip_prefix("path=").map(str::trim) {
            Some(ws) => {
                let ws = ws.trim_end_matches('/');
                vars.push(("cicada_dep", format!(r#"path = "{ws}/crates/all""#)));
                for (name, dir) in LAYER_DIRS {
                    vars.push((name, format!(r#"path = "{ws}/{dir}""#)));
                }
            }
            None => {
                vars.push(("cicada_dep", spec.to_string()));
                for (name, _) in LAYER_DIRS {
                    vars.push((name, spec.to_string()));
                }
            }
        }
        vars
    }

    pub fn from_args(rest: &[String]) -> Result<Self, String> {
        let dir = rest.iter().find(|a| !a.starts_with("--")).ok_or_else(|| {
            "用法：cicada new <目录> [--name <包名>] [--cicada <TOML 片段>]".to_string()
        })?;
        let dir = PathBuf::from(dir);
        let name = match flag(rest, "--name") {
            Some(n) => n,
            None => dir
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| "无法从目录名推断包名，请用 --name 指定".to_string())?
                .to_string(),
        };
        if name.is_empty()
            || !name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(format!(
                "包名 {name} 非法：只允许 ASCII 字母、数字、`-`、`_`"
            ));
        }
        let spec = flag(rest, "--cicada").unwrap_or_else(|| DEFAULT_CICADA_DEP.to_string());
        Ok(Self {
            dir,
            name,
            vars: Self::deps(&spec),
            template_dir: flag(rest, "--template")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_TEMPLATE_DIR)),
        })
    }
}

/// 生成工程。返回写入的文件数。
pub fn render(opts: &NewOptions) -> Result<usize, String> {
    if opts.dir.exists() {
        let empty = std::fs::read_dir(&opts.dir)
            .map_err(|e| format!("读取 {} 失败：{e}", opts.dir.display()))?
            .next()
            .is_none();
        if !empty {
            return Err(format!(
                "目录 {} 已存在且非空，拒绝覆盖",
                opts.dir.display()
            ));
        }
    }
    let src_root = &opts.template_dir;
    if !src_root.is_dir() {
        return Err(format!(
            "模板目录 {} 不存在（用 --template 指定）",
            src_root.display()
        ));
    }

    let mut written = 0;
    let mut stack = vec![src_root.clone()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).map_err(|e| format!("读取 {} 失败：{e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let rel = path
                .strip_prefix(src_root)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if SKIP.contains(&file_name) {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("{} 不是 UTF-8 文本：{e}", path.display()))?;
            let rendered = substitute(&text, &opts.name, &opts.vars).map_err(|left| {
                format!("{} 渲染后仍残留占位符：{}", rel.display(), left.join("、"))
            })?;
            let target = opts.dir.join(&rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::write(&target, rendered).map_err(|e| e.to_string())?;
            written += 1;
        }
    }
    written += copy_web(src_root, &opts.dir, &opts.name)?;
    written += copy_app(src_root, &opts.dir, &opts.name)?;
    Ok(written)
}

/// 把框架的 `web/` 复制到使用者工程，并改掉包名。
///
/// 前端是同一份 Vue 管理端：代码生成器产出的 `web/src/...` 只有落在它里面才能
/// 直接构建。`cargo generate --path template` 不会执行这一步，模板 README 会说明。
fn copy_web(
    src_root: &std::path::Path,
    target_dir: &std::path::Path,
    name: &str,
) -> Result<usize, String> {
    let Some(repo_root) = src_root.parent() else {
        return Ok(0);
    };
    copy_tree(
        &repo_root.join("web"),
        &target_dir.join("web"),
        &WEB_SKIP_DIRS,
        Some(&format!("{name}-web")),
    )
}

fn copy_app(
    src_root: &std::path::Path,
    target_dir: &std::path::Path,
    name: &str,
) -> Result<usize, String> {
    let Some(repo_root) = src_root.parent() else {
        return Ok(0);
    };
    let written = copy_tree(
        &repo_root.join("clients/app"),
        &target_dir.join("clients/app"),
        &APP_SKIP_DIRS,
        None,
    )?;
    rewrite_app_identity(target_dir, name)?;
    Ok(written)
}

/// 生成工程不能沿用框架的 identifier / deep-link scheme / keyring service：
/// 两个生成应用同时安装会撞包，深链也会互相抢。新工程按自己的名字重写这几处。
fn rewrite_app_identity(target_dir: &std::path::Path, name: &str) -> Result<(), String> {
    let id = app_slug(name);
    let scheme = id.replace('_', "-");
    let app_id = format!("dev.{id}.client");
    let replacements: &[(&str, String)] = &[
        ("dev.cicada.client", app_id.clone()),
        (
            "\"productName\": \"cicada\"",
            format!("\"productName\": \"{name}\""),
        ),
        (
            "\"name\": \"cicada-app\"",
            format!("\"name\": \"{id}-app\""),
        ),
        ("cicada://", format!("{scheme}://")),
        (
            "\"schemes\": [\"cicada\"]",
            format!("\"schemes\": [\"{scheme}\"]"),
        ),
        (
            "\"scheme\": [\"cicada\"]",
            format!("\"scheme\": [\"{scheme}\"]"),
        ),
        (
            "url.scheme() != \"cicada\"",
            format!("url.scheme() != \"{scheme}\""),
        ),
        (
            "parsed.protocol !== 'cicada:'",
            format!("parsed.protocol !== '{scheme}:'"),
        ),
    ];
    for rel in [
        "clients/app/package.json",
        "clients/app/README.md",
        "clients/app/src-tauri/tauri.conf.json",
        "clients/app/src-tauri/src/lib.rs",
        "web/src/utils/native.ts",
        "config/default.yaml",
    ] {
        let path = target_dir.join(rel);
        if !path.is_file() {
            continue;
        }
        let mut text = std::fs::read_to_string(&path)
            .map_err(|e| format!("{} 读取失败：{e}", path.display()))?;
        for (from, to) in replacements {
            text = text.replace(from, to);
        }
        std::fs::write(&path, text).map_err(|e| format!("{} 写入失败：{e}", path.display()))?;
    }
    Ok(())
}

/// 小写、只留字母数字和下划线；保证首字符是字母（Android applicationId 的约束）。
fn app_slug(name: &str) -> String {
    let mut slug: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    while slug.contains("__") {
        slug = slug.replace("__", "_");
    }
    let slug = slug.trim_matches('_').to_string();
    if slug.is_empty() {
        return "app".to_string();
    }
    match slug.chars().next() {
        Some(c) if c.is_ascii_alphabetic() => slug,
        _ => format!("app_{slug}"),
    }
}

fn copy_tree(
    src: &std::path::Path,
    dest: &std::path::Path,
    skip_dirs: &[&str],
    package_name: Option<&str>,
) -> Result<usize, String> {
    if !src.is_dir() {
        return Ok(0);
    }
    let mut written = 0;
    let mut stack = vec![src.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            std::fs::read_dir(&dir).map_err(|e| format!("读取 {} 失败：{e}", dir.display()))?;
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let rel = path
                .strip_prefix(src)
                .map_err(|e| e.to_string())?
                .to_path_buf();
            if rel
                .components()
                .any(|c| skip_dirs.iter().any(|skip| c.as_os_str() == *skip))
            {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let target = dest.join(&rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            match (rel == std::path::Path::new("package.json"), package_name) {
                (true, Some(name)) => {
                    let text = std::fs::read_to_string(&path)
                        .map_err(|e| format!("{} 不是 UTF-8：{e}", path.display()))?;
                    let text =
                        text.replace("\"name\": \"cicada-web\"", &format!("\"name\": \"{name}\""));
                    std::fs::write(&target, text).map_err(|e| e.to_string())?;
                }
                _ => {
                    std::fs::copy(&path, &target).map_err(|e| e.to_string())?;
                }
            }
            written += 1;
        }
    }
    Ok(written)
}

/// 占位符替换：只认有限的几个，替换完仍残留 `{{...}}` 即失败（模板与渲染器不漂移）。
pub fn substitute(
    text: &str,
    name: &str,
    vars: &[(&'static str, String)],
) -> Result<String, Vec<String>> {
    let mut out = text.replace("{{project-name}}", name);
    for (key, value) in vars {
        out = out.replace(&format!("{{{{{key}}}}}"), value);
    }
    let leftovers = leftovers(&out);
    if leftovers.is_empty() {
        Ok(out)
    } else {
        Err(leftovers)
    }
}

/// 找出所有残留的 `{{...}}` 占位符（去重）。
fn leftovers(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("{{") {
        let Some(end) = rest[start..].find("}}") else {
            found.push(rest[start..].to_string());
            break;
        };
        let token = rest[start..start + end + 2].to_string();
        if !found.contains(&token) {
            found.push(token);
        }
        rest = &rest[start + end + 2..];
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(dir: PathBuf, name: &str) -> NewOptions {
        NewOptions {
            dir,
            name: name.to_string(),
            vars: NewOptions::deps("path=/tmp/cicada"),
            template_dir: PathBuf::from(DEFAULT_TEMPLATE_DIR),
        }
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("cicada-new-test-{tag}-{}", std::process::id()));
        p
    }

    #[test]
    fn git_spec_applies_to_all_five_deps() {
        let vars = NewOptions::deps(r#"git = "https://example.com/cicada", tag = "v1""#);
        assert_eq!(vars.len(), 5);
        assert!(
            vars.iter().all(|(_, v)| v.contains("example.com")),
            "{vars:?}"
        );
    }

    #[test]
    fn path_spec_points_each_crate_at_its_own_dir() {
        let vars = NewOptions::deps("path=/ws");
        let get = |k: &str| vars.iter().find(|(n, _)| *n == k).unwrap().1.clone();
        assert_eq!(get("cicada_dep"), r#"path = "/ws/crates/all""#);
        assert_eq!(get("cicada_web_dep"), r#"path = "/ws/crates/web""#);
    }

    #[test]
    fn renders_template_without_leftovers() {
        let dir = temp_dir("ok");
        let _ = std::fs::remove_dir_all(&dir);
        let n = render(&opts(dir.clone(), "my-app")).unwrap();
        assert!(n > 5, "应写入多个文件，实际 {n}");

        let main = std::fs::read_to_string(dir.join("src/main.rs")).unwrap();
        assert!(main.contains("my-app"), "包名应替换：{main}");
        let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
        assert!(cargo.contains(r#"name = "my-app""#), "{cargo}");
        assert!(
            cargo.contains(r#"path = "/tmp/cicada/crates/all""#),
            "{cargo}"
        );
        assert!(
            cargo.contains(r#"cicada-web = { path = "/tmp/cicada/crates/web" }"#),
            "{cargo}"
        );
        assert!(!cargo.contains("{{"), "依赖来源应替换：{cargo}");
        assert_eq!(leftovers(&main), Vec::<String>::new());
        // 模板自身的元数据不进产物。
        assert!(!dir.join("cargo-generate.toml").exists());
        // 业务代码、迁移与前端骨架都在。
        assert!(dir.join("business/goods/module.rs").exists());
        assert!(dir.join("migrations/mod.rs").exists());
        assert!(dir.join("web/src/main.ts").exists());
        assert!(dir.join("clients/app/src-tauri/Cargo.toml").exists());
        let web_pkg = std::fs::read_to_string(dir.join("web/package.json")).unwrap();
        assert!(web_pkg.contains("\"name\": \"my-app-web\""), "{web_pkg}");
        // 桌面/移动壳的身份按工程名重写：包名、keyring 服务与 deep-link scheme 不能沿用框架。
        let app_conf =
            std::fs::read_to_string(dir.join("clients/app/src-tauri/tauri.conf.json")).unwrap();
        assert!(
            app_conf.contains("\"identifier\": \"dev.my_app.client\""),
            "{app_conf}"
        );
        assert!(app_conf.contains("\"scheme\": [\"my-app\"]"), "{app_conf}");
        let shell = std::fs::read_to_string(dir.join("clients/app/src-tauri/src/lib.rs")).unwrap();
        assert!(
            shell.contains("KEYRING_SERVICE: &str = \"dev.my_app.client\""),
            "{shell}"
        );
        let native = std::fs::read_to_string(dir.join("web/src/utils/native.ts")).unwrap();
        assert!(native.contains("parsed.protocol !== 'my-app:'"), "{native}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn refuses_non_empty_target() {
        let dir = temp_dir("busy");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("keep.txt"), "x").unwrap();
        let err = render(&opts(dir.clone(), "my-app")).unwrap_err();
        assert!(err.contains("非空"), "{err}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_placeholder_fails_closed() {
        let err = substitute("a {{mystery}} b", "x", &[]).unwrap_err();
        assert_eq!(err, vec!["{{mystery}}".to_string()]);
    }

    #[test]
    fn rejects_illegal_package_name() {
        let rest: Vec<String> = ["demo/with/slash", "--name", "bad name"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let err = NewOptions::from_args(&rest).unwrap_err();
        assert!(err.contains("非法"), "{err}");
    }
}
