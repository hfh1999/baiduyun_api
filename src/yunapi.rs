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
    FileManager,
    Create,
    LocateUpload,
    #[allow(dead_code)]
    PreCreate, // 三步上传,1st，
    #[allow(dead_code)]
    UpLoad, //2ed
    #[allow(dead_code)]
    Create2, //3rd
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
        // 两种错误字段:xpan 系列用 errno/errmsg,pcs 系列(upload 等)用 error_code/error_msg
        let errno = value["errno"]
            .as_i64()
            .or_else(|| value["error_code"].as_i64());
        if let Some(errno) = errno {
            let errmsg = value["errmsg"]
                .as_str()
                .or_else(|| value["error_msg"].as_str())
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
        YunNode::FileManager => {
            String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=filemanager")
        }
        YunNode::Create => String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=create"),
        YunNode::LocateUpload => {
            String::from("https://d.pcs.baidu.com/rest/2.0/pcs/file?method=locateupload")
        }
        YunNode::PreCreate => {
            String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=precreate")
        }
        YunNode::UpLoad => {
            String::from("https://d.pcs.baidu.com/rest/2.0/pcs/superfile2?method=upload")
        }
        YunNode::Create2 => String::from("https://pan.baidu.com/rest/2.0/xpan/file?method=create"),
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
    /// 解析 HTTP 响应文本为 Value(状态码检查 + JSON 解析)
    fn parse_http_response(response: blocking::Response) -> Result<Value, ApiError> {
        let status = response.status();
        let text = response
            .text()
            .map_err(|e| ApiError::from(format!("decode text error: {}", e).as_str()))?;
        parse_response(status, text)
    }
    /// GET 请求,参数进 query
    fn request_get<T: Serialize>(&self, in_node: YunNode, params: &T) -> Result<Value, ApiError> {
        let addr = self.get_addr(in_node, params)?;
        let response = self
            .client
            .get(&addr)
            .header(USER_AGENT, "pan.baidu.com")
            .send()
            .map_err(|e| ApiError::from(format!("send request error: {}", e).as_str()))?;
        Self::parse_http_response(response)
    }
    /// POST 请求,query_params 进 query、body_params 进 form body
    ///
    /// 实测确认:filemanager 的 opera 等参数必须放 URL query(body 会报 errno=2)
    fn request_post<Q: Serialize, B: Serialize>(
        &self,
        in_node: YunNode,
        query_params: &Q,
        body_params: &B,
    ) -> Result<Value, ApiError> {
        let addr = self.get_addr(in_node, query_params)?;
        let response = self
            .client
            .post(&addr)
            .header(USER_AGENT, "pan.baidu.com")
            .form(body_params)
            .send()
            .map_err(|e| ApiError::from(format!("send request error: {}", e).as_str()))?;
        Self::parse_http_response(response)
    }
    ///得到用户的基本信息
    ///
    ///返回信息的具体字段参见[UserInfo]
    ///
    /// # Example
    /// ```no_run
    /// use baiduyun_api::YunApi;
    ///
    /// let api = YunApi::new("你的access_token");
    /// let user = api.get_user_info().unwrap();
    /// println!("百度账号: {}", user.baidu_name);
    /// ```
    pub fn get_user_info(&self) -> Result<UserInfo, ApiError> {
        let params = EmptyParams;
        let value = self.request_get(YunNode::GetUserInfo, &params)?;
        Self::check_errno(&value)?;
        serde_json::from_value(value)
            .map_err(|e| ApiError::from(format!("malformed user info: {}", e).as_str()))
    }

    ///得到网盘的空间占用信息
    ///
    ///返回信息的具体的字段见[QuotaInfo]
    pub fn get_quota_info(&self) -> Result<QuotaInfo, ApiError> {
        let params = EmptyParams;
        let value = self.request_get(YunNode::GetQuotaInfo, &params)?;
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
        let value = self.request_get(YunNode::GetFileInfo, &params)?;
        Self::check_errno(&value)?;
        Self::parse_list::<FileInfoEx>(&value)
    }

    ///根据目录名得到该目录下的文件
    ///
    ///其中参数dir表示目录名,limit表示每次最多的条数(即每页limit个条目),start表示当前查询的总序号.
    ///limit不可超过10000
    ///返回信息的具体的字段见[FileInfo]
    /// [FileInfoIter] 是一个FileInfo的迭代器.
    ///
    /// # Example
    /// ```no_run
    /// use baiduyun_api::YunApi;
    ///
    /// let api = YunApi::new("你的access_token");
    /// let list = api.get_files_list("/apps", 0, 100).unwrap();
    /// for file in list {
    ///     println!("{}", file.server_filename);
    /// }
    /// ```
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
        let value = self.request_get(YunNode::GetFileList, &params)?;
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

        let value = self.request_get(YunNode::GetFileInfo, &params)?;
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

    /// 和 [Self::get_files_dlink_vec]类似,但是只查询单个文件
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
        let value = self.request_get(YunNode::Search, &params)?;
        Self::check_errno(&value)?;
        Self::parse_list::<SearchResult>(&value)
    }

    ///创建文件夹
    ///
    ///实测确认:百度 create 接口默认对重名目录自动重命名,故强制 `rtype=0`
    ///以获得"路径已存在即返回错误(-8)"的语义
    pub fn mkdir(&self, path: &str) -> Result<(), ApiError> {
        let params = CreateParams {
            path: path.to_string(),
            isdir: 1,
            rtype: 0,
        };
        let value = self.request_post(YunNode::Create, &EmptyParams, &params)?;
        Self::check_errno(&value)
    }

    /// filemanager 通用调用: 发送 opera 操作并检查响应(顶层 errno + info 数组单文件 errno)
    fn filemanager(&self, opera: &str, filelist: Vec<FileManagerItem>) -> Result<(), ApiError> {
        let query = FileManagerQuery {
            opera: opera.to_string(),
        };
        let body = FileManagerParams {
            filelist,
            async_: 0,
        };
        let value = self.request_post(YunNode::FileManager, &query, &body)?;
        Self::check_errno(&value)?;
        // info 数组中单文件错误需要逐个检查(如部分删除失败)
        if let Some(info) = value["info"].as_array() {
            for item in info {
                let errno = item["errno"].as_i64().unwrap_or(0);
                if errno != 0 {
                    let errmsg = item["errmsg"]
                        .as_str()
                        .unwrap_or("no errmsg from baidu");
                    return Err(ApiError::new(errno, errmsg));
                }
            }
        }
        Ok(())
    }

    ///删除文件/目录,支持批量
    ///
    ///可传路径字符串或实现了 [FilePath] 的类型(如 [FileInfo])
    ///注意:百度对不存在的文件静默成功(实测返回 errno=0)
    pub fn remove<T: FilePath>(&self, paths: &[T]) -> Result<(), ApiError> {
        let filelist: Vec<FileManagerItem> = paths
            .iter()
            .map(|p| FileManagerItem::Delete(p.ret_path()))
            .collect();
        self.filemanager("delete", filelist)
    }

    ///移动文件/目录到目标目录
    ///
    ///可传路径字符串或实现了 [FilePath] 的类型(如 [FileInfo])
    pub fn mv<T: FilePath>(&self, from: T, to_dir: &str) -> Result<(), ApiError> {
        let filelist = vec![FileManagerItem::MoveCopy {
            path: from.ret_path(),
            dest: to_dir.to_string(),
            newname: None,
        }];
        self.filemanager("move", filelist)
    }

    ///复制文件/目录到目标目录
    ///
    ///可传路径字符串或实现了 [FilePath] 的类型(如 [FileInfo])
    pub fn cp<T: FilePath>(&self, from: T, to_dir: &str) -> Result<(), ApiError> {
        let filelist = vec![FileManagerItem::MoveCopy {
            path: from.ret_path(),
            dest: to_dir.to_string(),
            newname: None,
        }];
        self.filemanager("copy", filelist)
    }

    ///重命名文件/目录(一次一个)
    ///
    ///可传路径字符串或实现了 [FilePath] 的类型(如 [FileInfo])
    pub fn rename<T: FilePath>(&self, path: T, new_name: &str) -> Result<(), ApiError> {
        let filelist = vec![FileManagerItem::Rename {
            path: path.ret_path(),
            newname: new_name.to_string(),
        }];
        self.filemanager("rename", filelist)
    }

    /// 获取上传域名(单步上传前置步骤)
    ///
    /// 实测确认: 不传 uploadid 也可用(文档标为必填,实际可选)
    fn get_upload_host(&self, remote_path: &str) -> Result<String, ApiError> {
        let params = LocateUploadParams {
            appid: 250528,
            path: remote_path.to_string(),
            upload_version: "2.0".to_string(),
        };
        let value = self.request_get(YunNode::LocateUpload, &params)?;
        // 成功时 error_code=0,失败时带 error_msg
        let code = value["error_code"].as_i64().unwrap_or(0);
        if code != 0 {
            let msg = value["error_msg"]
                .as_str()
                .unwrap_or("no error_msg from baidu");
            return Err(ApiError::new(code, msg));
        }
        value["servers"][0]["server"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ApiError::from("locateupload response has no servers field"))
    }

    ///上传本地文件到网盘(单步上传,限 2GB)
    ///
    ///- `local_path` 本地文件路径
    ///- `remote_path` 网盘目标路径,**实测确认必须位于 `/apps/{自己的应用名}/` 下**
    ///  (否则返回 31064 file is not authorized,与目录是否存在无关)
    ///- `ondup` 冲突策略(默认 [OnDup::Fail]: 文件已存在报 31061)
    ///
    ///流程: 获取上传域名 -> multipart POST {host}/rest/2.0/pcs/file?method=upload
    pub fn upload(
        &self,
        local_path: &str,
        remote_path: &str,
        ondup: OnDup,
    ) -> Result<UploadResult, ApiError> {
        let host = self.get_upload_host(remote_path)?;
        let query = serde_urlencoded::to_string(&UploadQuery {
            path: remote_path,
            ondup,
        })
        .map_err(|e| ApiError::from(format!("serialize upload query error: {}", e).as_str()))?;
        let upload_addr = format!(
            "{}/rest/2.0/pcs/file?method=upload&access_token={}&{}",
            host, self.access_token, query
        );
        let form = reqwest::blocking::multipart::Form::new()
            .file("file", local_path)
            .map_err(|e| {
                ApiError::from(format!("open local file for upload error: {}", e).as_str())
            })?;
        let response = self
            .client
            .post(&upload_addr)
            .header(USER_AGENT, "pan.baidu.com")
            .multipart(form)
            .send()
            .map_err(|e| ApiError::from(format!("send upload request error: {}", e).as_str()))?;
        let value = Self::parse_http_response(response)?;
        // 上传响应的错误字段是 error_code/error_msg(非 errno)
        let code = value["error_code"].as_i64().unwrap_or(0);
        if code != 0 {
            let msg = value["error_msg"]
                .as_str()
                .unwrap_or("no error_msg from baidu");
            return Err(ApiError::new(code, msg));
        }
        serde_json::from_value(value)
            .map_err(|e| ApiError::from(format!("malformed upload result: {}", e).as_str()))
    }
}

/// 上传请求的 query 参数(path 需 urlencode)
#[derive(Serialize)]
struct UploadQuery<'a> {
    path: &'a str,
    ondup: OnDup,
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
                "path": "/唱戏机",
                "server_filename": "唱戏机",
                "thumbs": {"icon": "https://icon", "url1": "https://u1", "url2": "https://u2", "url3": "https://u3"}
            }]
        });
        let list = YunApi::parse_list::<SearchResult>(&value).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].server_filename, "唱戏机");
        let thumbs = list[0].thumbs.as_ref().unwrap();
        assert_eq!(thumbs.url1.as_deref(), Some("https://u1"));
        assert_eq!(thumbs.icon.as_deref(), Some("https://icon"));
    }

    #[test]
    fn test_parse_list_file_info_ex_with_real_response_shape() {
        // 真实 filemetas 响应形状(实测):
        // 字段名是 filename/isdir(非 file_name/is_dir),视频文件无 height/width/date_taken
        let value = serde_json::json!({
            "errno": 0,
            "list": [{
                "category": 1,
                "dlink": "https://d.pcs.baidu.com/file/xxx",
                "filename": "LH.mkv",
                "fs_id": 885188,
                "isdir": 0,
                "server_ctime": 1545053884,
                "server_mtime": 1640416420,
                "size": 1718307682,
                "path": "/apps/LH.mkv"
            }]
        });
        let list = YunApi::parse_list::<FileInfoEx>(&value).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].file_name, "LH.mkv");
        assert_eq!(list[0].is_dir, 0);
        assert!(list[0].dlink.starts_with("https://"));
        assert_eq!(list[0].height, None);
    }

    #[test]
    fn test_filemanager_items_json() {
        // filemanager 的 filelist 是 JSON 字符串: delete 为裸路径数组,
        // mv/cp 为 {path,dest,newname?} 对象数组, rename 为 {path,newname} 数组
        let items = vec![
            FileManagerItem::Delete("/a.txt".to_string()),
            FileManagerItem::MoveCopy {
                path: "/a.txt".to_string(),
                dest: "/dest".to_string(),
                newname: None,
            },
            FileManagerItem::Rename {
                path: "/a.txt".to_string(),
                newname: "b.txt".to_string(),
            },
        ];
        let json = serde_json::to_string(&items).unwrap();
        assert_eq!(
            json,
            r#"["/a.txt",{"path":"/a.txt","dest":"/dest"},{"path":"/a.txt","newname":"b.txt"}]"#
        );
    }

    #[test]
    fn test_filemanager_params_form() {
        // 表单序列化: filelist 以 JSON 字符串形式出现在 form body
        let params = FileManagerParams {
            filelist: vec![FileManagerItem::Delete("/a.txt".to_string())],
            async_: 0,
        };
        let encoded = serde_urlencoded::to_string(&params).unwrap();
        assert_eq!(encoded, "filelist=%5B%22%2Fa.txt%22%5D&async=0");
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
    fn test_parse_response_http_error_with_error_code() {
        // pcs 系列接口(upload/locateupload)的错误字段是 error_code/error_msg(非 errno)
        let status = reqwest::StatusCode::BAD_REQUEST;
        let text = r#"{"error_code": 31061, "error_msg": "file already exists"}"#.to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 31061);
        assert!(format!("{}", error).contains("file already exists"));
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
