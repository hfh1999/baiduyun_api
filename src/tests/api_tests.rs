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

/// 从环境变量/.env 读取开放平台应用名(上传接口要求路径位于 /apps/{应用名}/ 下)
fn load_app_name() -> Option<String> {
    if let Ok(name) = std::env::var("BAIDU_APP_NAME") {
        return Some(name.trim().to_string());
    }
    let content = std::fs::read_to_string(".env").ok()?;
    content.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("BAIDU_APP_NAME=")
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
        path: "/test.txt".to_string(),
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
    println!("current dir ===> {}", myfs.pwd());
    myfs.chdir("学习资料/").unwrap();
    println!("current dir ===> {}", myfs.pwd());
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

/// 从根目录(必要时回退到"学习资料")动态收集 n 个真实文件 fs_id,避免硬编码用户数据
fn collect_file_ids(api: &YunApi, n: usize) -> Vec<i64> {
    let mut ids: Vec<i64> = Vec::new();
    for dir in ["/", "/学习资料"] {
        if let Ok(list) = api.get_files_list(dir, 0, 100) {
            for item in list {
                if item.isdir == 0 {
                    ids.push(item.fs_id);
                    if ids.len() >= n {
                        return ids;
                    }
                }
            }
        }
    }
    ids
}

/// 生成带随机后缀的临时测试路径(避免并发冲突与残留)
/// 路径位于 /apps/{应用名}/ 下(上传接口要求;mkdir/filemanager 不受限,统一放这里更一致)
fn temp_path(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let app_dir = load_app_name().unwrap_or_else(|| "bypy".to_string());
    format!("/apps/{}/{}_{}_{}", app_dir, prefix, std::process::id(), nanos)
}

#[test]
#[ignore]
fn test_mkdir_remove_roundtrip() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let dir = temp_path("yunfs_mkdir");

    // 创建目录并确认可列出
    api.mkdir(&dir).expect("mkdir 应成功");
    let list = api.get_files_list(&dir, 0, 10).expect("新建目录应可列出");
    assert_eq!(list.count(), 0, "新建目录应为空");

    // 重复创建应报错(-8 已存在)
    let dup = api.mkdir(&dir);
    assert!(dup.is_err(), "重复创建应失败");
    assert_eq!(dup.unwrap_err().ret_errno(), -8, "重复创建应返回 errno=-8");

    // 删除后应可重新创建(验证清理彻底)
    api.remove(&[dir.clone()]).expect("删除应成功");
    api.mkdir(&dir).expect("删除后应可重新创建");
    api.remove(&[dir]).expect("清理应成功");
}

#[test]
#[ignore]
fn test_mv_cp_rename_roundtrip() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let base = temp_path("yunfs_move");
    let a = format!("{}/a", base);
    let b = format!("{}/b", base);

    // 准备: 用目录作为载体(不依赖上传)
    api.mkdir(&base).expect("创建 base 应成功");
    api.mkdir(&a).expect("创建 a 应成功");
    api.mkdir(&b).expect("创建 b 应成功");

    // rename: a -> a_renamed
    api.rename(&a, "a_renamed").expect("rename 应成功");
    let a2 = format!("{}/a_renamed", base);
    let list = api.get_files_list(&base, 0, 10).unwrap().collect::<Vec<_>>();
    assert!(
        list.iter().any(|f| f.server_filename == "a_renamed"),
        "rename 后应看到 a_renamed"
    );

    // mv: a_renamed -> b/
    api.mv(&a2, &b).expect("mv 应成功");
    let list_b = api.get_files_list(&b, 0, 10).unwrap().collect::<Vec<_>>();
    assert!(
        list_b.iter().any(|f| f.server_filename == "a_renamed"),
        "mv 后 b 下应看到 a_renamed"
    );

    // cp: b/a_renamed -> base/(复制回上层,源保留)
    let src = format!("{}/a_renamed", b);
    api.cp(&src, &base).expect("cp 应成功");
    let list_base = api.get_files_list(&base, 0, 10).unwrap().collect::<Vec<_>>();
    assert!(
        list_base.iter().any(|f| f.server_filename == "a_renamed"),
        "cp 后 base 下应看到 a_renamed"
    );
    let list_b2 = api.get_files_list(&b, 0, 10).unwrap().collect::<Vec<_>>();
    assert!(
        list_b2.iter().any(|f| f.server_filename == "a_renamed"),
        "cp 后源 b/a_renamed 应保留"
    );

    // 清理
    api.remove(&[base]).expect("清理应成功");
}

#[test]
#[ignore]
fn test_upload_roundtrip() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let Some(app_name) = load_app_name() else {
        println!("skip: 缺少 BAIDU_APP_NAME(上传路径需位于 /apps/{{应用名}}/ 下)");
        return;
    };
    let _ = app_name;
    let api = YunApi::new(&key);
    let remote = temp_path("yunfs_upload");

    // 本地临时文件
    let local = std::env::temp_dir().join(format!("baiduyun_test_{}.txt", std::process::id()));
    std::fs::write(&local, "hello baiduyun api upload test").expect("写本地临时文件应成功");

    // 上传成功
    let result = api
        .upload(local.to_str().unwrap(), &remote, OnDup::Fail)
        .expect("上传应成功");
    assert_eq!(result.path, remote, "响应的 path 应与请求一致");
    assert!(result.size > 0, "size 应大于 0");
    assert!(result.md5.is_some(), "上传成功应返回 md5");
    println!("uploaded: {} ({} bytes)", result.path, result.size);

    // 重复上传应报 31061(文件已存在)
    let dup = api.upload(local.to_str().unwrap(), &remote, OnDup::Fail);
    let dup_err = dup.expect_err("重复上传应失败");
    assert_eq!(
        dup_err.ret_errno(),
        31061,
        "重复上传应返回 errno=31061,实际: {}",
        dup_err
    );

    // 清理: 远端文件 + 本地临时文件
    api.remove(&[remote]).expect("清理远端文件应成功");
    std::fs::remove_file(&local).ok();
}

#[test]
#[ignore]
fn test_yunfs_mkdir_rm_relative() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let base = temp_path("yunfs_fs");
    let mut fs = util::YunFs::new(&api);

    // 绝对路径创建 + 切换
    fs.mkdir(&base).expect("mkdir 应成功");
    fs.chdir(&base).expect("chdir 应成功");

    // 相对路径创建子目录
    fs.mkdir("sub").expect("相对路径 mkdir 应成功");
    let list = fs.ls().expect("ls 应成功").collect::<Vec<_>>();
    assert!(
        list.iter().any(|f| f.server_filename == "sub"),
        "ls 应看到 sub"
    );

    // 相对路径删除
    fs.rm("sub").expect("相对路径 rm 应成功");
    let list2 = fs.ls().expect("ls 应成功").collect::<Vec<_>>();
    assert!(
        !list2.iter().any(|f| f.server_filename == "sub"),
        "rm 后不应再看到 sub"
    );

    // 清理
    fs.rm(&base).expect("清理应成功");
}

#[test]
#[ignore]
fn test_user_info() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let info = api.get_user_info().expect("get_user_info 应成功");
    assert!(!info.baidu_name.is_empty(), "baidu_name 不应为空");
    println!("baidu_name = {}", info.baidu_name);
}

#[test]
#[ignore]
fn test_quota_info() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let quota = api.get_quota_info().expect("get_quota_info 应成功");
    assert!(quota.total > 0, "总空间应大于 0,实际: {}", quota.total);
    assert!(quota.free >= 0, "剩余空间不应为负");
    println!("total = {}, free = {}", quota.total, quota.free);
}

#[test]
#[ignore]
fn test_files_info_real() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let ids = collect_file_ids(&api, 1);
    assert!(
        !ids.is_empty(),
        "网盘中未找到任何文件,无法验证 get_files_info 成功路径"
    );
    let infos = api
        .get_files_info(&ids)
        .expect("get_files_info 应成功解析真实 filemetas 响应");
    assert_eq!(infos.len(), 1);
    let info = &infos[0];
    assert!(!info.file_name.is_empty(), "file_name 不应为空");
    assert!(info.dlink.starts_with("http"), "dlink 应以 http 开头");
    println!("file_name = {}, size = {}", info.file_name, info.size);
}

#[test]
#[ignore]
fn test_files_dlink_vec_real() {
    let Some(key) = load_key() else {
        println!("skip: no BAIDU_ACCESS_TOKEN in env or .env file");
        return;
    };
    let api = YunApi::new(&key);
    let ids = collect_file_ids(&api, 2);
    assert_eq!(ids.len(), 2, "需要至少 2 个真实文件验证批量取链");
    let links = api
        .get_files_dlink_vec(&ids)
        .expect("批量取链应成功");
    assert_eq!(links.len(), 2);
    for link in &links {
        assert!(link.starts_with("http"), "链接应以 http 开头");
    }
    println!("got {} dlinks", links.len());
}
