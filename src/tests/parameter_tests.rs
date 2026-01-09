use crate::models::*;
use serde_urlencoded::to_string;

#[test]
fn test_get_file_list_params_serialization() {
    let params = GetFileListParams {
        dir: "/test".to_string(),
        start: 0,
        limit: 100,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "dir=%2Ftest&start=0&limit=100");
}

#[test]
fn test_get_file_info_params_serialization() {
    let params = GetFileInfoParams {
        fsids: vec![123, 456],
        dlink: 1,
        extra: 1,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "fsids=%5B123%2C456%5D&dlink=1&extra=1");
}

#[test]
fn test_search_params_serialization() {
    let params = SearchParams {
        key: "test".to_string(),
        dir: "/".to_string(),
        recursion: 0,
        page: 1,
        num: 50,
        web: 1,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "key=test&dir=%2F&recursion=0&page=1&num=50&web=1");
}

#[test]
fn test_empty_params_serialization() {
    let params = EmptyParams;
    let result = to_string(&params).unwrap();
    assert_eq!(result, "");
}

#[test]
fn test_get_file_info_params_empty_fsids() {
    let params = GetFileInfoParams {
        fsids: vec![],
        dlink: 0,
        extra: 0,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "fsids=%5B%5D&dlink=0&extra=0");
}

#[test]
fn test_get_file_info_params_single_fsid() {
    let params = GetFileInfoParams {
        fsids: vec![789],
        dlink: 1,
        extra: 0,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "fsids=%5B789%5D&dlink=1&extra=0");
}

#[test]
fn test_get_file_list_params_boundary_negative_start() {
    let params = GetFileListParams {
        dir: "/".to_string(),
        start: -1,
        limit: 10000,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "dir=%2F&start=-1&limit=10000");
}

#[test]
fn test_get_file_list_params_boundary_large_limit() {
    let params = GetFileListParams {
        dir: "/".to_string(),
        start: 0,
        limit: 10001,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "dir=%2F&start=0&limit=10001");
}

#[test]
fn test_search_params_boundary_values() {
    let params = SearchParams {
        key: "test".to_string(),
        dir: "/".to_string(),
        recursion: 0,
        page: 0,
        num: 1001,
        web: 1,
    };
    let result = to_string(&params).unwrap();
    assert_eq!(result, "key=test&dir=%2F&recursion=0&page=0&num=1001&web=1");
}

#[test]
fn test_special_characters_in_params() {
    let params = SearchParams {
        key: "test?param".to_string(),
        dir: "/test&file".to_string(),
        recursion: 0,
        page: 1,
        num: 50,
        web: 1,
    };
    let result = to_string(&params).unwrap();
    assert!(result.contains("key=test%3Fparam"));
    assert!(result.contains("dir=%2Ftest%26file"));
}
