//! 半自动授权工具: 生成授权 URL -> 打开浏览器 -> 粘贴地址栏 URL -> 自动提取 token 写入 .env
//!
//! 用法:
//!   cargo run --example authorize -- --app-key=你的APP_KEY
//!   cargo run --example authorize -- --app-key=xxx --token-only   # 只打印 token 不写文件
//!
//! 原理说明: 百度隐式流(response_type=token)的 token 位于回调 URL 的 fragment(# 之后),
//! fragment 不会随 HTTP 请求发送,故无法全自动回调接收,采用"粘贴地址栏 URL"半自动方案。

use std::io::{self, BufRead};

const AUTHORIZE_URL: &str = "https://openapi.baidu.com/oauth/2.0/authorize";

fn main() {
    // 1. 解析参数(--app-key=xxx 或环境变量 BAIDU_APP_KEY)
    let args: Vec<String> = std::env::args().collect();
    let mut app_key = String::new();
    let mut token_only = false;
    for arg in &args[1..] {
        if let Some(v) = arg.strip_prefix("--app-key=") {
            app_key = v.to_string();
        } else if arg == "--token-only" {
            token_only = true;
        }
    }
    if app_key.is_empty() {
        app_key = std::env::var("BAIDU_APP_KEY").unwrap_or_default();
    }
    if app_key.is_empty() {
        eprintln!("错误: 缺少 --app-key=xxx 参数(你的开放平台应用 App Key,见 pan.baidu.com/union 控制台)");
        std::process::exit(1);
    }

    // 2. 生成授权 URL 并打开浏览器
    let url = format!(
        "{AUTHORIZE_URL}?response_type=token&client_id={app_key}&redirect_uri=oob&scope=netdisk"
    );
    println!("1. 授权 URL(已尝试自动打开浏览器):\n   {url}\n");
    open_browser(&url);

    // 3. 用户粘贴地址栏 URL
    println!("2. 在浏览器中点\"授权/允许\"后,会跳转到空白页,地址栏形如:");
    println!("   http://openapi.baidu.com/oauth/2.0/login_success#access_token=xxx&expires_in=...");
    println!("   请把**整个地址栏内容**复制粘贴到这里,回车确认:");
    let stdin = io::stdin();
    let line = stdin
        .lock()
        .lines()
        .next()
        .expect("读取输入失败")
        .expect("读取输入失败")
        .trim()
        .to_string();

    // 4. 解析 fragment 提取 access_token / expires_in
    let Some(fragment) = line.split('#').nth(1) else {
        eprintln!("错误: 粘贴的内容中没有 # 之后的 fragment,请确认是授权后的完整地址栏 URL");
        std::process::exit(1);
    };
    let mut token = None;
    let mut expires_in = None;
    for pair in fragment.split('&') {
        let mut it = pair.splitn(2, '=');
        match (it.next(), it.next()) {
            (Some("access_token"), Some(v)) => token = Some(v.to_string()),
            (Some("expires_in"), Some(v)) => expires_in = Some(v.to_string()),
            _ => {}
        }
    }
    let Some(token) = token else {
        eprintln!("错误: fragment 中没有 access_token,授权可能被拒绝或 URL 不完整");
        std::process::exit(1);
    };
    println!("3. access_token = {token}");
    if let Some(secs) = &expires_in {
        println!("   expires_in  = {secs} 秒(30 天内持续使用不过期,过期重新授权即可)");
    }

    if token_only {
        println!("{token}");
        return;
    }

    // 5. 写入 .env(保留原有内容,仅替换/追加 BAIDU_ACCESS_TOKEN 行)
    match upsert_env("BAIDU_ACCESS_TOKEN", &token) {
        Ok(()) => println!(
            "4. 已写入 .env(.env 已被 gitignore,不会提交)\n   可运行网络测试: cargo test -- --ignored --test-threads=1"
        ),
        Err(e) => {
            eprintln!("写入 .env 失败: {e}");
            std::process::exit(1);
        }
    }
}

/// 跨平台打开浏览器(cmd start / open / xdg-open)
fn open_browser(url: &str) {
    // Windows 的 cmd 会把 URL 中的 & 当命令分隔符,需要转义为 ^&
    #[cfg(target_os = "windows")]
    let escaped = url.replace('&', "^&");
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", &escaped])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    let result = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "unsupported platform",
    ));

    if let Ok(mut child) = result {
        let _ = child.wait();
    } else {
        println!("   (自动打开浏览器失败,请手动复制上面的 URL 打开)");
    }
}

/// 更新 .env 中的 key=value(不存在则追加),保留其他行
fn upsert_env(key: &str, value: &str) -> io::Result<()> {
    let path = ".env";
    let mut lines: Vec<String> = if let Ok(content) = std::fs::read_to_string(path) {
        content.lines().map(|l| l.to_string()).collect()
    } else {
        Vec::new()
    };
    let mut replaced = false;
    for line in lines.iter_mut() {
        if line.starts_with(&format!("{key}=")) {
            *line = format!("{key}={value}");
            replaced = true;
        }
    }
    if !replaced {
        lines.push(format!("{key}={value}"));
    }
    std::fs::write(path, format!("{}\n", lines.join("\n")))
}
