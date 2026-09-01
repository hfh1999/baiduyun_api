//! 演示 CLI: 网盘常用操作的命令行示例
//!
//! 准备: 先在项目根目录的 `.env` 中填入你的 access_token
//!   BAIDU_ACCESS_TOKEN=你的token
//! 可运行 `cargo run --example authorize` 自动获取并写入 .env;
//! 上传操作还需 `BAIDU_APP_NAME`(远端路径必须位于 /apps/{应用名}/ 下)。
//!
//! 用法:
//!   cargo run --example cli -- user
//!   cargo run --example cli -- quota
//!   cargo run --example cli -- ls [/目录]
//!   cargo run --example cli -- mkdir /apps/media_shell/新目录
//!   cargo run --example cli -- rm /路径1 [/路径2...]
//!   cargo run --example cli -- mv /源 /目标目录
//!   cargo run --example cli -- cp /源 /目标目录
//!   cargo run --example cli -- rename /路径 新名称
//!   cargo run --example cli -- upload ./本地文件 /apps/你的应用名/远端名
//!   cargo run --example cli -- search 关键字 [/搜索根目录]
//!
//! Git Bash 注意: `/` 开头的参数会被 MSYS 自动转换成 Windows 路径,
//! 请用 `MSYS_NO_PATHCONV=1` 前缀运行,或直接在 cmd / PowerShell 中执行。

use baiduyun_api::{util, ApiError, FileInfo, OnDup, SearchResult, YunApi};
use std::process::exit;

const BLUE: &str = "\x1b[34m"; // 目录
const GREEN: &str = "\x1b[32m"; // 成功
const RED: &str = "\x1b[31m"; // 错误
const DIM: &str = "\x1b[2m"; // 次要信息
const RESET: &str = "\x1b[0m";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(key) = load_key() else {
        eprintln!("{RED}错误: 未找到 BAIDU_ACCESS_TOKEN,请先运行 cargo run --example authorize{RESET}");
        exit(1);
    };
    let api = YunApi::new(&key);

    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let rest = &args[2..];
    let result: Result<(), ApiError> = match cmd {
        "user" => cmd_user(&api),
        "quota" => cmd_quota(&api),
        "ls" => cmd_ls(&api, rest),
        "mkdir" => cmd_mkdir(&api, rest),
        "rm" => cmd_rm(&api, rest),
        "mv" => cmd_mv_cp(&api, rest, false),
        "cp" => cmd_mv_cp(&api, rest, true),
        "rename" => cmd_rename(&api, rest),
        "upload" => cmd_upload(&api, rest),
        "search" => cmd_search(&api, rest),
        "help" => {
            print_usage();
            Ok(())
        }
        other => Err(ApiError::from(format!("未知命令: {other}").as_str())),
    };

    if let Err(e) = result {
        eprintln!("{RED}错误 (errno={}): {e}{RESET}", e.ret_errno());
        exit(1);
    }
}

// ---------- 命令实现 ----------

fn cmd_user(api: &YunApi) -> Result<(), ApiError> {
    let info = api.get_user_info()?;
    println!("百度账号 : {}", info.baidu_name);
    println!("网盘账号 : {}", info.netdisk_name);
    println!("会员类型 : {}", util::get_vip_type_str(info.vip_type)?);
    println!("用户 ID  : {}", info.uk);
    Ok(())
}

fn cmd_quota(api: &YunApi) -> Result<(), ApiError> {
    let quota = api.get_quota_info()?;
    let (_, total_mb, total_gb) = util::human_quota(quota.total);
    let (_, used_mb, used_gb) = util::human_quota(quota.used);
    let (_, free_mb, free_gb) = util::human_quota(quota.free);
    println!(
        "总空间: {GREEN}{:.1} GB{RESET} ({:.0} MB)    已用: {:.1} GB ({:.0} MB)    剩余: {:.1} GB ({:.0} MB)    7天内到期: {}",
        total_gb, total_mb, used_gb, used_mb, free_gb, free_mb,
        if quota.expire { "是" } else { "否" }
    );
    Ok(())
}

fn cmd_ls(api: &YunApi, rest: &[String]) -> Result<(), ApiError> {
    let dir = rest.first().map(String::as_str).unwrap_or("/");
    let list = api.get_files_list(dir, 0, 1000)?;
    let items: Vec<FileInfo> = list.collect();
    if items.is_empty() {
        println!("{DIM}(空目录){RESET}");
        return Ok(());
    }
    // 目录在前,文件在后
    let mut dirs: Vec<&FileInfo> = items.iter().filter(|f| f.isdir == 1).collect();
    let mut files: Vec<&FileInfo> = items.iter().filter(|f| f.isdir == 0).collect();
    dirs.sort_by(|a, b| a.server_filename.cmp(&b.server_filename));
    files.sort_by(|a, b| a.server_filename.cmp(&b.server_filename));

    let name_w = items
        .iter()
        .map(|f| f.server_filename.chars().count())
        .max()
        .unwrap_or(0)
        .max(8);
    println!("{:<n$}  {:>10}  {:<16}  {}", "名称", "大小", "修改时间(UTC)", "类型", n = name_w);
    for f in dirs {
        println!(
            "{BLUE}{:<n$}{RESET}  {:>10}  {:<16}  {BLUE}目录{RESET}",
            f.server_filename,
            "-",
            fmt_utc(f.server_mtime),
            n = name_w
        );
    }
    for f in files {
        let (_, mb, gb) = util::human_quota(f.size);
        let size = if gb >= 1.0 {
            format!("{:.1} GB", gb)
        } else if mb >= 1.0 {
            format!("{:.0} MB", mb)
        } else {
            format!("{} B", f.size)
        };
        println!(
            "{:<n$}  {:>10}  {:<16}  文件",
            f.server_filename,
            size,
            fmt_utc(f.server_mtime),
            n = name_w
        );
    }
    println!("{DIM}共 {} 项(最多显示前 1000 条){RESET}", items.len());
    Ok(())
}

fn cmd_mkdir(api: &YunApi, rest: &[String]) -> Result<(), ApiError> {
    let path = rest
        .first()
        .ok_or_else(|| ApiError::from("用法: cli mkdir <路径>"))?;
    api.mkdir(path)?;
    println!("{GREEN}已创建: {}{RESET}", path);
    Ok(())
}

fn cmd_rm(api: &YunApi, rest: &[String]) -> Result<(), ApiError> {
    if rest.is_empty() {
        return Err(ApiError::from("用法: cli rm <路径1> [路径2...]"));
    }
    api.remove(rest)?;
    println!("{GREEN}已删除 {} 个路径{RESET}", rest.len());
    Ok(())
}

fn cmd_mv_cp(api: &YunApi, rest: &[String], is_copy: bool) -> Result<(), ApiError> {
    let (from, to_dir) = match (rest.first(), rest.get(1)) {
        (Some(f), Some(t)) => (f.as_str(), t.as_str()),
        _ => {
            return Err(ApiError::from(if is_copy {
                "用法: cli cp <源> <目标目录>"
            } else {
                "用法: cli mv <源> <目标目录>"
            }))
        }
    };
    if is_copy {
        api.cp(from, to_dir)?;
        println!("{GREEN}已复制: {from} -> {to_dir}/{RESET}");
    } else {
        api.mv(from, to_dir)?;
        println!("{GREEN}已移动: {from} -> {to_dir}/{RESET}");
    }
    Ok(())
}

fn cmd_rename(api: &YunApi, rest: &[String]) -> Result<(), ApiError> {
    let (path, new_name) = match (rest.first(), rest.get(1)) {
        (Some(p), Some(n)) => (p.as_str(), n.as_str()),
        _ => return Err(ApiError::from("用法: cli rename <路径> <新名称>")),
    };
    api.rename(path, new_name)?;
    println!("{GREEN}已重命名: {path} -> {new_name}{RESET}");
    Ok(())
}

fn cmd_upload(api: &YunApi, rest: &[String]) -> Result<(), ApiError> {
    let (local, remote) = match (rest.first(), rest.get(1)) {
        (Some(l), Some(r)) => (l.as_str(), r.as_str()),
        _ => return Err(ApiError::from("用法: cli upload <本地文件> <远端路径>")),
    };
    println!("上传中: {local} -> {remote} ...");
    let result = api.upload(local, remote, OnDup::Fail)?;
    let (_, mb, gb) = util::human_quota(result.size);
    let size = if gb >= 1.0 {
        format!("{:.1} GB", gb)
    } else if mb >= 1.0 {
        format!("{:.0} MB", mb)
    } else {
        format!("{} B", result.size)
    };
    println!(
        "{GREEN}上传完成: {} ({size}, md5={}){RESET}",
        result.path,
        result.md5.as_deref().unwrap_or("(未返回)")
    );
    Ok(())
}

fn cmd_search(api: &YunApi, rest: &[String]) -> Result<(), ApiError> {
    let key = rest
        .first()
        .ok_or_else(|| ApiError::from("用法: cli search <关键字> [目录]"))?;
    let dir = rest.get(1).map(String::as_str).unwrap_or("/");
    let items = api.search_with_key(key, dir, true, 1, 100, false)?;
    if items.is_empty() {
        println!("{DIM}(无结果){RESET}");
        return Ok(());
    }
    println!("{:<16}  {:<30}  {}", "fs_id", "名称", "路径");
    for item in items.iter() {
        println!("{:<16}  {:<30}  {}", item.fs_id, item.path.rsplit('/').next().unwrap_or(""), item.path);
    }
    println!("{DIM}共 {} 条结果{RESET}", items.len());
    Ok(())
}

fn print_usage() {
    println!(
        r#"{DIM}用法: cargo run --example cli -- <命令> [参数]

  user                  显示用户信息
  quota                 显示空间信息
  ls [目录]             列出目录内容(默认 /)
  mkdir <路径>          创建目录
  rm <路径1> [路径2...]  删除(可多个)
  mv <源> <目标目录>     移动
  cp <源> <目标目录>     复制
  rename <路径> <新名>   重命名
  upload <本地> <远端>   单步上传(≤2GB, 远端须在 /apps/{{应用名}}/ 下)
  search <关键字> [目录] 搜索(默认递归根目录)
  help                  显示本帮助{RESET}"#
    );
}

// ---------- 工具函数 ----------

/// 从 .env 读取 BAIDU_ACCESS_TOKEN(与测试约定一致)
fn load_key() -> Option<String> {
    if let Ok(key) = std::env::var("BAIDU_ACCESS_TOKEN") {
        return Some(key.trim().to_string());
    }
    let content = std::fs::read_to_string(".env").ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("BAIDU_ACCESS_TOKEN=").map(|v| v.trim().to_string())
    })
}

/// 简化 UTC 时间格式化(Howard Hinnant 公历算法,无依赖)
fn fmt_utc(ts: i64) -> String {
    let days = ts.div_euclid(86400);
    let secs_of_day = ts.rem_euclid(86400);
    let (h, m) = (secs_of_day / 3600, (secs_of_day % 3600) / 60);
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mth = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mth <= 2 { y + 1 } else { y };
    format!("{year}-{mth:02}-{d:02} {h:02}:{m:02}")
}

// 让编译器不警告未使用的类型(SearchResult 在 search 输出中通过泛型使用)
#[allow(unused)]
fn _silence(_: SearchResult) {}
