use crate::models::*;
use crate::util;
use crate::yunapi::YunApi;

// 以下测试需要真实的 access_token 和网络访问,默认不运行。
// 运行方式: cargo test -- --ignored --test-threads=1
// 注意: 百度接口频控严格(errno=31034),建议单线程顺序执行,避免并行触发限流

/// 从环境变量 BAIDU_ACCESS_TOKEN 读取,回退到项目根目录的 .env 文件(见 .env.example)
fn load_key() -> Option<String> {
    if let Ok(key) = std::env::var("BAIDU_ACCESS_TOKEN") {
        return Some(key.trim().to_string());
    }
    let content = std::fs::read_to_string(".env").ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("BAIDU_ACCESS_TOKEN=")
            .map(|v| v.trim().to_string())
    })
}

#[test]
#[ignore]
fn test_api_method_signatures() {
    let api = YunApi::new("test_token");

    // 无效 token 的请求必然返回 Err:
    // 本测试同时验证公共 API 签名兼容性(编译期)与错误路径(运行期)
    assert!(api.get_user_info().is_err());
    assert!(api.get_quota_info().is_err());

    assert!(api.get_files_list("/", 0, 100).is_err());
    assert!(api.get_files_info(&[123i64, 456i64]).is_err());
    assert!(api.search_with_key("test", "/", false, 1, 50, true).is_err());
}

#[test]
#[ignore]
fn test_api_method_with_fileinfo() {
    let api = YunApi::new("test_token");

    let file_info = FileInfo {
        path: "/test.txt".to_string(),
        category: 4,
        fs_id: 123,
        isdir: 0,
        local_ctime: 0,
        local_mtime: 0,
        server_ctime: 0,
        server_mtime: 0,
        server_filename: "test.txt".to_string(),
        md5: Some("abc123".to_string()),
        size: 1024,
        thumbs: None,
        dir_empty: None,
    };

    // 无效 token 的请求必然返回 Err
    assert!(api.get_files_info(&[file_info]).is_err());
}

#[test]
#[ignore]
fn test_api_method_with_searchresult() {
    let api = YunApi::new("test_token");

    let search_result = SearchResult {
        category: 4,
        fs_id: 123,
        isdir: 0,
        local_ctime: 0,
        local_mtime: 0,
        server_ctime: 0,
        server_mtime: 0,
        md5: Some("abc123".to_string()),
        size: 1024,
        thumbs: None,
    };

    // 无效 token 的请求必然返回 Err
    assert!(api.get_files_info(&[search_result]).is_err());
}

#[test]
#[ignore]
fn test_api_method_get_file_dlink() {
    let api = YunApi::new("test_token");

    // 无效 token 的请求必然返回 Err
    assert!(api.get_file_dlink(123i64).is_err());
}

#[test]
#[ignore]
fn test_api_method_get_files_dlink_vec() {
    let api = YunApi::new("test_token");

    // 无效 token 的请求必然返回 Err
    assert!(api.get_files_dlink_vec(&[123i64, 456i64]).is_err());
}

#[test]
#[ignore]
fn test_api() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let list = api.get_files_list("/", 0, 10).unwrap();
    let list_vec: Vec<FileInfo> = list.collect();
    assert!(!list_vec.is_empty(), "根目录不应为空");
    assert!(list_vec.len() <= 10, "limit=10 时最多返回 10 条");
    println!("list len = {}", list_vec.len());
}

#[test]
#[ignore]
fn error_key() {
    // 无效 token 应当返回认证失败错误,而不是 panic 或成功
    // 百度文档化行为: 无效 token 返回 errno=-6 (Authentication failed)
    let key = "invalid_access_token_for_test";
    let api = YunApi::new(key);
    let result = api.get_files_list("/", 0, 10);
    let error = match result {
        Ok(_) => panic!("invalid token should produce an error"),
        Err(e) => e,
    };
    assert_eq!(
        error.ret_errno(),
        -6,
        "无效 token 应被百度拒绝并返回 errno=-6,实际错误: {}",
        error
    );
}

#[test]
#[ignore]
fn test_search() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let r = api
        .search_with_key("唱戏机", "/", true, 1, 100, false)
        .expect("搜索请求应成功");
    for item in r {
        println!("item = {}", item.fs_id);
    }
}

#[test]
#[ignore]
fn get_dlink_flow() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    // 完整链路验证: 浏览目录 -> 定位目标文件 -> 获取真实下载链接
    // 注意: 仅验证取链,不实际下载文件,避免覆盖本地磁盘
    let api = YunApi::new(&key);
    let mut myfs = util::YunFs::new(&api);
    println!("current dir ===> {}", myfs.pwd().unwrap());
    myfs.chdir("学习资料/").unwrap();
    println!("current dir ===> {}", myfs.pwd().unwrap());
    let files = myfs.ls().unwrap();
    let mut target: Option<FileInfo> = None;
    for item in files {
        if item.server_filename.contains("数据库系统概念") {
            target = Some(item);
            break;
        }
    }
    let file = target.expect("网盘上未找到目标文件(数据库系统概念),请确认 学习资料/ 目录内容");
    let link = api.get_file_dlink(file).unwrap();
    assert!(
        link.starts_with("http"),
        "下载链接应以 http(s) 开头,实际: {}",
        link
    );
    println!("dlink = {}", link);
}
