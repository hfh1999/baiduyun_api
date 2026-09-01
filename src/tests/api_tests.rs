use crate::models::*;
use crate::util;
use crate::yunapi::YunApi;

// 以下测试需要真实的 access_token 和网络访问,默认不运行。
// 运行方式: cargo test -- --ignored

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

    let _ = api.get_user_info();
    let _ = api.get_quota_info();

    let _ = api.get_files_list("/", 0, 100);
    let _ = api.get_files_info(&[123i64, 456i64]);
    let _ = api.search_with_key("test", "/", false, 1, 50, true);

    assert!(true);
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

    let _ = api.get_files_info(&[file_info]);

    assert!(true);
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

    let _ = api.get_files_info(&[search_result]);

    assert!(true);
}

#[test]
#[ignore]
fn test_api_method_get_file_dlink() {
    let api = YunApi::new("test_token");

    let _ = api.get_file_dlink(123i64);

    assert!(true);
}

#[test]
#[ignore]
fn test_api_method_get_files_dlink_vec() {
    let api = YunApi::new("test_token");

    let _ = api.get_files_dlink_vec(&[123i64, 456i64]);

    assert!(true);
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
    assert_eq!(list_vec.len(), 10);
    println!("list len = {}", list_vec.len());
}

#[test]
#[ignore]
fn error_key() {
    // 无效 token 应当返回错误,而不是 panic 或成功
    let key = "invalid_access_token_for_test";
    let api = YunApi::new(key);
    let result = api.get_files_list("/", 0, 10);
    assert!(result.is_err(), "invalid token should produce an error");
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
        .unwrap();
    for item in r {
        println!("item = {}", item.fs_id);
    }
}

#[test]
#[ignore]
fn download_test() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let mut myfs = util::YunFs::new(&api);
    println!("current dir ===> {}", myfs.pwd().unwrap());
    myfs.chdir("学习资料/").unwrap();
    println!("current dir ===> {}", myfs.pwd().unwrap());
    let files = myfs.ls().unwrap();
    let mut file_to_download: FileInfo = FileInfo::default();
    for item in files {
        if item
            .server_filename
            .contains("中文第六版@www.java1234.com.pdf")
        {
            println!("pdf: -> {}; id ={} ", item.server_filename, item.fs_id);
            file_to_download = item;
        }
    }
    let link = api.get_file_dlink(file_to_download).unwrap();
    println!("{}", link);
    util::download(&link, "D:/test.pdf", 100, &key, true);
}
