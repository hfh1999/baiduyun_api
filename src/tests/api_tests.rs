use crate::models::*;
use crate::yunapi::YunApi;

#[test]
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
fn test_api_method_get_file_dlink() {
    let api = YunApi::new("test_token");

    let _ = api.get_file_dlink(123i64);

    assert!(true);
}

#[test]
fn test_api_method_get_files_dlink_vec() {
    let api = YunApi::new("test_token");

    let _ = api.get_files_dlink_vec(&[123i64, 456i64]);

    assert!(true);
}

use crate::*;
use std::fs::read_to_string;
#[test]
fn test_api() {
    // load key form file to prevent key to reveal.
    let key = read_to_string("D:\\rust\\baiduyun_space\\baiduyun_api\\key.txt").unwrap();
    let api = YunApi::new(&key);
    let list = api.get_files_list("/", 0, 10).unwrap();
    let list_vec: Vec<FileInfo> = list.collect();
    assert!(list_vec.len() == 10);
    println!("list len = {}", list_vec.len());
}

#[test]
#[should_panic]
fn error_key() {
    let key =
        "++++123.64295f7207e0dcc4612276a7955e11f9.YaWhelqaKCPDHKxghpjx7shiRLRS44h1gcl4t7-.ckQMUQ";
    let api = YunApi::new(key);
    api.get_files_list("/", 0, 10).unwrap();
}

#[test]
fn test_search() {
    let key = read_to_string("D:\\rust\\baiduyun_space\\baiduyun_api\\key.txt").unwrap();
    let api = YunApi::new(&key);
    let r = api
        .search_with_key("唱戏机", "/", true, 1, 100, false)
        .unwrap();
    for item in r {
        println!("item = {}", item.fs_id);
    }
}

#[test]
fn download_test() {
    let key = read_to_string("D:\\rust\\baiduyun_space\\baiduyun_api\\key.txt").unwrap();
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
