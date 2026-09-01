use super::error::ApiError;
use super::models::*;
use reqwest::blocking;
use reqwest::header::USER_AGENT;
use serde::Serialize;
use serde_json::Value;
use serde_urlencoded::to_string;

enum YunNode {
    GetUserInfo,
    GetQuotaInfo,
    GetFileList,
    GetFileInfo,
    Search,
    #[allow(dead_code)]
    PreCreate, // 三步上传,1st，
    #[allow(dead_code)]
    UpLoad, //2ed
    #[allow(dead_code)]
    Create, //3rd
}

///要使用本api,必须使用YunApi结构体
pub struct YunApi {
    access_token: String,
    client: blocking::Client,
    //pwd: String, //当前路径
}
/// 纯函数:根据 HTTP 状态码和响应文本产出 Value 或错误,便于离线测试
///
/// - 非 2xx:先尝试解析 body 的 `errno`/`errmsg` 透传百度真实错误;解析失败回退内部错误
/// - 2xx:JSON 解析失败返回带解析详情的内部错误
fn parse_response(status: reqwest::StatusCode, text: String) -> Result<Value, ApiError> {
    if status.is_success() {
        return serde_json::from_str(&text).map_err(|e| {
            ApiError::from(format!("parse json error: {}", e).as_str())
        });
    }
    if let Ok(value) = serde_json::from_str::<Value>(&text) {
        if let Some(errno) = value["errno"].as_i64() {
            let errmsg = value["errmsg"]
                .as_str()
                .unwrap_or("no errmsg from baidu")
                .to_string();
            return Err(ApiError::new(errno, &errmsg));
        }
    }
    Err(ApiError::from(
        format!("HTTP status {} from baidu api", status).as_str(),
    ))
}
fn get_node_addr(in_node: YunNode) -> String {
    match in_node {
        YunNode::GetUserInfo => {
            String::from("https://pan.baidu.com/rest/2.0/xpan/nas?method=uinfo")
        }
        YunNode::GetQuotaInfo => {
            String::from("https://pan.baidu.com/api/quota?checkfree=1&checkexpire=1")
        }
        YunNode::GetFileList => {
            String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=list")
        }
        YunNode::GetFileInfo => {
            String::from("https://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas")
        }
        YunNode::Search => String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=search"),
        YunNode::PreCreate => {
            String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=precreate")
        }
        YunNode::UpLoad => {
            String::from("https://d.pcs.baidu.com/rest/2.0/pcs/superfile2?method=upload")
        }
        YunNode::Create => String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=create"),
    }
}
impl YunApi {
    ///创建一个YunApi结构体.
    ///
    ///其中参数in_token表示的是用户认证后获得的[access_token](https://pan.baidu.com/union/document/entrance#%E6%8E%A5%E5%85%A5%E6%B5%81%E7%A8%8B)
    pub fn new(in_token: &str) -> YunApi {
        YunApi {
            access_token: String::from(in_token),
            client: blocking::Client::new(),
            //pwd: String::from("/"),
        }
    }
    fn get_addr<T: Serialize>(&self, in_node: YunNode, params: &T) -> Result<String, ApiError> {
        let node_addr = get_node_addr(in_node);
        let query_string = to_string(params)
            .map_err(|e| ApiError::from(format!("serialize params error: {}", e).as_str()))?;

        if node_addr.contains('?') {
            let mut addr = format!("{}&access_token={}", node_addr, self.access_token);
            if !query_string.is_empty() {
                addr.push_str(&format!("&{}", query_string));
            }
            Ok(addr)
        } else {
            let mut addr = format!("{}?access_token={}", node_addr, self.access_token);
            if !query_string.is_empty() {
                addr.push_str(&format!("&{}", query_string));
            }
            Ok(addr)
        }
    }
    /// 把响应 "list" 字段解析为 Vec<T>,统一错误处理;空列表是合法响应
    fn parse_list<T: serde::de::DeserializeOwned>(value: &Value) -> Result<Vec<T>, ApiError> {
        let list = value["list"].as_array().ok_or_else(|| {
            ApiError::from("response has no list field or it is not an array")
        })?;
        list.iter()
            .map(|item| {
                serde_json::from_value(item.clone()).map_err(|e| {
                    ApiError::from(format!("malformed list item: {}", e).as_str())
                })
            })
            .collect()
    }
    /// 检查响应 errno 是否为 0;不为 0 则返回透传百度 errmsg 的错误
    fn check_errno(value: &Value) -> Result<(), ApiError> {
        let errno = value["errno"].as_i64().ok_or_else(|| {
            ApiError::from("response has no errno field")
        })?;
        if errno == 0 {
            Ok(())
        } else {
            let errmsg = value["errmsg"]
                .as_str()
                .unwrap_or("no errmsg from baidu")
                .to_string();
            Err(ApiError::new(errno, &errmsg))
        }
    }
    fn request<T: Serialize>(&self, in_node: YunNode, params: &T) -> Result<Value, ApiError> {
        let addr = self.get_addr(in_node, params)?;
        let response = self
            .client
            .get(&addr)
            .header(USER_AGENT, "pan.baidu.com")
            .send()
            .map_err(|e| ApiError::from(format!("send request error: {}", e).as_str()))?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|e| ApiError::from(format!("decode text error: {}", e).as_str()))?;
        parse_response(status, text)
    }
    ///得到用户的基本信息
    ///
    ///返回信息的具体字段参见[UserInfo]
    pub fn get_user_info(&self) -> Result<UserInfo, ApiError> {
        let params = EmptyParams;
        let value = self.request(YunNode::GetUserInfo, &params)?;
        Self::check_errno(&value)?;
        serde_json::from_value(value)
            .map_err(|e| ApiError::from(format!("malformed user info: {}", e).as_str()))
    }

    ///得到网盘的空间占用信息
    ///
    ///返回信息的具体的字段见[QuotaInfo]
    pub fn get_quota_info(&self) -> Result<QuotaInfo, ApiError> {
        let params = EmptyParams;
        let value = self.request(YunNode::GetQuotaInfo, &params)?;
        Self::check_errno(&value)?;
        serde_json::from_value(value)
            .map_err(|e| ApiError::from(format!("malformed quota info: {}", e).as_str()))
    }

    ///查询文件信息,可以获取下载链接之用.
    ///
    /// 只有实现了[FileId] trait的类型可以用在这里
    /// 查询结果 [FileInfoEx] 其各个字段详见其描述.
    pub fn get_files_info<T>(&self, file_ids: &[T]) -> Result<Vec<FileInfoEx>, ApiError>
    where
        T: FileId,
    {
        let dlink = 1_i64;
        let extra = 1_i64;
        let fsids: Vec<i64> = file_ids.iter().map(|x| x.ret_file_id()).collect();
        let params = GetFileInfoParams {
            fsids,
            dlink,
            extra,
        };
        let value = self.request(YunNode::GetFileInfo, &params)?;
        Self::check_errno(&value)?;
        Self::parse_list::<FileInfoEx>(&value)
    }

    ///根据目录名得到该目录下的文件
    ///
    ///其中参数dir表示目录名,limit表示每次最多的条数(即每页limit个条目),start表示当前查询的总序号.
    ///limit不可超过10000
    ///返回信息的具体的字段见[FileInfo]
    /// [FileInfoIter] 是一个FileInfo的迭代器.
    pub fn get_files_list(
        &self,
        dir: &str,
        start: i64,
        limit: i64,
    ) -> Result<FileInfoIter, ApiError> {
        if !(0..=10000).contains(&limit) {
            return Err(ApiError::new(8989, "limit arg error."));
        }
        if start < 0 {
            return Err(ApiError::new(8989, "start arg error."));
        }
        let params = GetFileListParams {
            dir: dir.to_string(),
            start,
            limit,
        };
        let value = self.request(YunNode::GetFileList, &params)?;
        Self::check_errno(&value)?;
        let list = Self::parse_list::<FileInfo>(&value)?;
        Ok(FileInfoIter::new(list))
    }

    //pub fn get_file_dlink(&self,file:&FilePtr)->Result<String,ApiError>{
    //}

    ///根据提供的文件列表返回相应的下载链接
    ///
    /// 只有实现了[FileId] trait的类型可以用在这里
    ///注意:
    ///- 传递的列表中只处理文件类型，而不处理目录类型
    ///- 得到的链接只存活8小时
    pub fn get_files_dlink_vec<T>(&self, files: &[T]) -> Result<Vec<String>, ApiError>
    where
        T: FileId,
    {
        let fsids: Vec<i64> = files.iter().map(|x| x.ret_file_id()).collect();
        let dlink = 1_i64;
        let params = GetFileInfoParams {
            fsids,
            dlink,
            extra: 0,
        };

        let value = self.request(YunNode::GetFileInfo, &params)?;
        Self::check_errno(&value)?;
        let list = Self::parse_list::<serde_json::Value>(&value)?;
        list.iter()
            .map(|item| {
                item["dlink"]
                    .as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| ApiError::from("dlink field missing"))
            })
            .collect()
    }

    /// 和 [get_files_dlink_vec]类似,但是只查询单个文件
    ///
    /// 只有实现了[FileId] trait的类型可以用在这里
    pub fn get_file_dlink<T>(&self, file: T) -> Result<String, ApiError>
    where
        T: FileId,
    {
        let file_vec = vec![file];
        let mut link_vec = self.get_files_dlink_vec(&file_vec)?;
        //单文件请求最多返回一个链接,直接取出,无需复制
        link_vec.pop().ok_or_else(|| ApiError::from("empty dlink list"))
    }

    /// 根据关键字进行搜索
    ///
    ///-  search_key 表示要搜索的关键字,可以使用中文.
    ///-  search_dir 表示要搜索的根目录.
    ///- is_recursive 表示是否递归地进行搜索.
    ///- in_num 表示每页的项目.
    ///- in_page表示当前搜索的页号.
    ///- in_web 表示是否返回缩略图
    pub fn search_with_key(
        &self,
        search_key: &str,
        search_dir: &str,
        is_recursive: bool,
        in_page: i64,
        in_num: i64,
        in_web: bool,
    ) -> Result<Vec<SearchResult>, ApiError> {
        let params = SearchParams {
            key: search_key.to_string(),
            dir: search_dir.to_string(),
            recursion: is_recursive as i64,
            page: in_page,
            num: in_num,
            web: in_web as i64,
        };
        if in_page < 1 {
            return Err(ApiError::new(8989, "Page is less than 1."));
        }
        if in_num > 1000 {
            return Err(ApiError::new(8989, "Num is more than 1000."));
        }
        let value = self.request(YunNode::Search, &params)?;
        Self::check_errno(&value)?;
        Self::parse_list::<SearchResult>(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_errno_error_with_errmsg() {
        let value = serde_json::json!({"errno": -6, "errmsg": "invalid token"});
        let error = YunApi::check_errno(&value).unwrap_err();
        assert_eq!(error.ret_errno(), -6);
        let display = format!("{}", error);
        assert!(display.contains("invalid token"));
    }

    #[test]
    fn test_check_errno_missing() {
        let value = serde_json::json!({"list": []});
        let error = YunApi::check_errno(&value).unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
    }

    #[test]
    fn test_parse_list_missing() {
        let value = serde_json::json!({"errno": 0});
        let error = YunApi::parse_list::<serde_json::Value>(&value).unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
    }

    #[test]
    fn test_parse_list_malformed_item() {
        // 缺 path 字段的 FileInfo 无法反序列化
        let value = serde_json::json!({"errno": 0, "list": [{"fs_id": 1}]});
        let error = YunApi::parse_list::<FileInfo>(&value).unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("malformed list item"));
    }

    #[test]
    fn test_parse_list_search_result_with_thumbs_object() {
        // 真实百度响应:thumbs 是对象而非字符串(官方文档参数表与示例矛盾,实测为对象)
        let value = serde_json::json!({
            "errno": 0,
            "list": [{
                "category": 6,
                "fs_id": 100613,
                "isdir": 1,
                "local_ctime": 1586248549,
                "local_mtime": 1586248549,
                "server_ctime": 1586248549,
                "server_mtime": 1712313928,
                "md5": "",
                "size": 0,
                "thumbs": {"icon": "https://icon", "url1": "https://u1", "url2": "https://u2", "url3": "https://u3"}
            }]
        });
        let list = YunApi::parse_list::<SearchResult>(&value).unwrap();
        assert_eq!(list.len(), 1);
        let thumbs = list[0].thumbs.as_ref().unwrap();
        assert_eq!(thumbs.url1.as_deref(), Some("https://u1"));
        assert_eq!(thumbs.icon.as_deref(), Some("https://icon"));
    }

    #[test]
    fn test_parse_list_ok() {
        let value = serde_json::json!({"errno": 0, "list": [{"a": 1}, {"a": 2}]});
        let list = YunApi::parse_list::<serde_json::Value>(&value).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["a"], 1);
    }

    #[test]
    fn test_check_errno_ok() {
        let value = serde_json::json!({"errno": 0, "list": []});
        assert!(YunApi::check_errno(&value).is_ok());
    }

    #[test]
    fn test_parse_response_http_error_with_errno() {
        let status = reqwest::StatusCode::FORBIDDEN;
        let text = r#"{"errno": 31034, "errmsg": "hit frequency control"}"#.to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 31034);
        let display = format!("{}", error);
        assert!(display.contains("hit frequency control"));
    }

    #[test]
    fn test_parse_response_http_error_non_json_body() {
        let status = reqwest::StatusCode::BAD_GATEWAY;
        let text = "<html>502 Bad Gateway</html>".to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("HTTP status 502"));
    }

    #[test]
    fn test_parse_response_success() {
        let status = reqwest::StatusCode::OK;
        let text = r#"{"errno": 0, "list": [1, 2]}"#.to_string();
        let result = parse_response(status, text);
        let value = result.unwrap();
        assert_eq!(value["errno"], 0);
        assert_eq!(value["list"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_parse_response_success_invalid_json() {
        let status = reqwest::StatusCode::OK;
        let text = "not json at all".to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("parse json error"));
    }

    /// 用于触发 get_addr 参数序列化失败的测试类型
    struct FailingParams;
    impl Serialize for FailingParams {
        fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("intentional serialization failure"))
        }
    }

    #[test]
    fn test_get_addr_serialize_failure() {
        let api = YunApi::new("test_token");
        let result = api.get_addr(YunNode::GetUserInfo, &FailingParams);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("serialize params error"));
    }

    #[test]
    fn test_get_addr_with_existing_query_params() {
        let api = YunApi::new("test_token");
        let params = EmptyParams;
        let addr = api.get_addr(YunNode::GetUserInfo, &params).unwrap();
        assert_eq!(
            addr,
            "https://pan.baidu.com/rest/2.0/xpan/nas?method=uinfo&access_token=test_token"
        );
    }

    #[test]
    fn test_get_addr_without_existing_query_params() {
        let api = YunApi::new("test_token");
        let params = GetFileListParams {
            dir: "/".to_string(),
            start: 0,
            limit: 10,
        };
        let addr = api.get_addr(YunNode::GetFileList, &params).unwrap();
        assert_eq!(addr, "https://pan.baidu.com/rest/2.0/xpan/file?method=list&access_token=test_token&dir=%2F&start=0&limit=10");
    }

    #[test]
    fn test_get_addr_with_special_chars() {
        let api = YunApi::new("test_token");
        let params = SearchParams {
            key: "测试中文".to_string(),
            dir: "/test dir".to_string(),
            recursion: 0,
            page: 1,
            num: 50,
            web: 1,
        };
        let addr = api.get_addr(YunNode::Search, &params).unwrap();
        assert!(addr.contains("key=%E6%B5%8B%E8%AF%95%E4%B8%AD%E6%96%87"));
        assert!(addr.contains("dir=%2Ftest+dir"));
        assert!(addr.contains("access_token=test_token"));
    }

    #[test]
    fn test_get_addr_with_file_info_params() {
        let api = YunApi::new("test_token");
        let params = GetFileInfoParams {
            fsids: vec![123, 456],
            dlink: 1,
            extra: 1,
        };
        let addr = api.get_addr(YunNode::GetFileInfo, &params).unwrap();
        assert!(addr.contains("fsids=%5B123%2C456%5D"));
        assert!(addr.contains("dlink=1"));
        assert!(addr.contains("extra=1"));
        assert!(addr.contains("access_token=test_token"));
    }

    #[test]
    fn test_get_addr_with_quota_info() {
        let api = YunApi::new("test_token");
        let params = EmptyParams;
        let addr = api.get_addr(YunNode::GetQuotaInfo, &params).unwrap();
        assert_eq!(
            addr,
            "https://pan.baidu.com/api/quota?checkfree=1&checkexpire=1&access_token=test_token"
        );
    }
}
