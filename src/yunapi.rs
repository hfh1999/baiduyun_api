use super::error::ApiError;
use super::models::*;
use serde::Serialize;
use serde_json::Value;
use serde_urlencoded::to_string;
use std::io::{Read, Write};

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
    agent: ureq::Agent,
    //pwd: String, //当前路径
}
/// 纯函数:根据 HTTP 状态码和响应文本产出 Value 或错误,便于离线测试
///
/// - 非 2xx:先尝试解析 body 的 `errno`/`errmsg` 透传百度真实错误;解析失败回退内部错误
/// - 2xx:JSON 解析失败返回带解析详情的内部错误
///
/// `status` 用 u16(HTTP 状态码纯数字),不依赖任何 HTTP 客户端类型——同步(ureq)与异步(reqwest)后端共用
fn parse_response(status: u16, text: String) -> Result<Value, ApiError> {
    if (200..300).contains(&status) {
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
            agent: Self::new_agent(),
            //pwd: String::from("/"),
        }
    }
    /// 构建同步后端 agent(ureq)
    ///
    /// `http_status_as_error(false)`:非 2xx 不报错,由 `parse_response` 统一解析
    /// status + body(与 reqwest 时代行为一致,百度错误走 errno/error_code 透传)
    fn new_agent() -> ureq::Agent {
        let config = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build();
        config.into()
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
    /// GET 请求,参数进 query
    fn request_get<T: Serialize>(&self, in_node: YunNode, params: &T) -> Result<Value, ApiError> {
        let addr = self.get_addr(in_node, params)?;
        let mut response = self
            .agent
            .get(&addr)
            .header("User-Agent", "pan.baidu.com")
            .call()
            .map_err(|e| ApiError::from(format!("send request error: {}", e).as_str()))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| ApiError::from(format!("decode text error: {}", e).as_str()))?;
        parse_response(status, text)
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
        let body = to_string(body_params)
            .map_err(|e| ApiError::from(format!("serialize params error: {}", e).as_str()))?;
        let mut response = self
            .agent
            .post(&addr)
            .header("User-Agent", "pan.baidu.com")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .send(body)
            .map_err(|e| ApiError::from(format!("send request error: {}", e).as_str()))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| ApiError::from(format!("decode text error: {}", e).as_str()))?;
        parse_response(status, text)
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
        // multipart 表单(ureq: Form::file 流式发送,2GB 内不读进内存)
        let form = ureq::unversioned::multipart::Form::new()
            .file("file", local_path)
            .map_err(|e| {
                ApiError::from(format!("open local file for upload error: {}", e).as_str())
            })?;
        let mut response = self
            .agent
            .post(&upload_addr)
            .header("User-Agent", "pan.baidu.com")
            .send(form)
            .map_err(|e| ApiError::from(format!("send upload request error: {}", e).as_str()))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| ApiError::from(format!("decode text error: {}", e).as_str()))?;
        let value = parse_response(status, text)?;
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

    /// 下载文件到本地(自动拼接 access_token,流式落盘,覆盖已存在)
    ///
    /// - `dlink` 来自 [Self::get_file_dlink] / [Self::get_files_dlink_vec](链接 8 小时有效)
    /// - `dst` 本地文件路径;**已存在会被覆盖**(非续写)
    /// - 返回实际下载字节数,可与远端 `FileInfo.size` 对比校验完整性
    ///
    /// 便捷入口,等价 [Self::download_with] + [DownloadOpts::default]。
    pub fn download(&self, dlink: &str, dst: &str) -> Result<u64, ApiError> {
        self.download_with(dlink, dst, DownloadOpts::default())
    }

    /// 下载文件,支持断点续传与分块并发(自动拼接 access_token,流式落盘)
    ///
    /// - `offset` = 0:从头下载,已存在的 `dst` 会被覆盖(truncate)
    /// - `offset` > 0:断点续传——从该字节偏移继续,追加写入 `dst`;
    ///   若服务器忽略 Range 返回 200 全量,自动回退从头下载(truncate)
    ///   (`offset` 应取上次调用返回的字节累计,配合中断后重试)
    /// - `threads` = 1:单连接;>1:分块并发([offset, 文件尾) 切块并行拉取)
    /// - 返回本次实际下载字节数
    pub fn download_with(
        &self,
        dlink: &str,
        dst: &str,
        opts: DownloadOpts,
    ) -> Result<u64, ApiError> {
        if opts.threads <= 1 {
            self.download_single(dlink, dst, opts.offset)
        } else {
            self.download_parallel(dlink, dst, opts.offset, opts.threads)
        }
    }

    /// 单连接下载:offset=0 全量 truncate;offset>0 Range+append(200 时回退 truncate)
    fn download_single(&self, dlink: &str, dst: &str, offset: u64) -> Result<u64, ApiError> {
        let url = Self::with_access_token(dlink, &self.access_token);
        let mut request = self.agent.get(&url).header("User-Agent", "pan.baidu.com");
        if offset > 0 {
            request = request.header("Range", &format!("bytes={offset}-"));
        }
        let mut response = request.call().map_err(|e| {
            ApiError::from(format!("send download request error: {}", e).as_str())
        })?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            // 非 2xx:body 是 JSON 错误体(实测 403 -> {"error_code":31045,...})
            let text = response
                .body_mut()
                .read_to_string()
                .map_err(|e| ApiError::from(format!("decode download error: {}", e).as_str()))?;
            return parse_response(status, text).map(|_| 0);
        }
        // 断点双态:206 + offset>0 -> append 续写;200(服务器忽略 Range)或从头 -> truncate
        let resume = status == 206 && offset > 0;
        let mut options = std::fs::OpenOptions::new();
        options.create(true).write(true);
        if resume {
            options.append(true);
        } else {
            options.truncate(true);
        }
        let mut file = options
            .open(dst)
            .map_err(|e| ApiError::from(format!("open local file error: {}", e).as_str()))?;
        Self::stream_to_file(response.body_mut().as_reader(), &mut file)
    }

    /// 分块并发下载:探测文件总大小 -> [offset, total) 切块 -> 多线程并行拉取,逐块定位写入
    fn download_parallel(
        &self,
        dlink: &str,
        dst: &str,
        offset: u64,
        threads: usize,
    ) -> Result<u64, ApiError> {
        let url = Self::with_access_token(dlink, &self.access_token);

        // 1. 探测总大小(Range: bytes=0-0,206 时 content-range 携带 total)
        let mut probe = self
            .agent
            .get(&url)
            .header("User-Agent", "pan.baidu.com")
            .header("Range", "bytes=0-0")
            .call()
            .map_err(|e| ApiError::from(format!("send download request error: {}", e).as_str()))?;
        let status = probe.status().as_u16();
        if !(200..300).contains(&status) {
            let text = probe
                .body_mut()
                .read_to_string()
                .map_err(|e| ApiError::from(format!("decode download error: {}", e).as_str()))?;
            return parse_response(status, text).map(|_| 0);
        }
        if status != 206 {
            // 服务器不支持 Range:回退单连接(读完探测 body 以释放连接)
            let _ = probe.body_mut().read_to_vec().map_err(|e| {
                ApiError::from(format!("decode download error: {}", e).as_str())
            })?;
            return self.download_single(dlink, dst, offset);
        }
        let total: u64 = probe
            .headers()
            .get("content-range")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.rsplit('/').next())
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| ApiError::from("download probe: no content-range total"))?;
        // 读完 1 字节探测 body,释放连接回池
        let mut buf = [0u8; 64];
        while probe.body_mut().as_reader().read(&mut buf).unwrap_or(0) > 0 {}
        if offset >= total {
            return Err(ApiError::from("download offset beyond file size"));
        }

        // 2. 预置文件长度(create/set_len 生成空洞文件,各块 seek 写入)
        std::fs::File::create(dst)
            .and_then(|f| f.set_len(total))
            .map_err(|e| ApiError::from(format!("open local file error: {}", e).as_str()))?;

        // 3. 切块并发下载
        let len = total - offset;
        let block_count = threads.min(len as usize).max(1);
        let base = len / block_count as u64;
        let mut ranges = Vec::with_capacity(block_count);
        for i in 0..block_count as u64 {
            let start = offset + i * base;
            let end = if i + 1 == block_count as u64 { total } else { start + base };
            ranges.push((start, end - start));
        }
        let mut downloaded: u64 = 0;
        std::thread::scope(|s| {
            let mut handles = Vec::with_capacity(ranges.len());
            for (start, blk_len) in ranges {
                let agent = &self.agent;
                let url = &url;
                let dst = dst;
                handles.push(s.spawn(move || {
                    Self::download_block(agent, url, dst, start, blk_len)
                }));
            }
            for h in handles {
                downloaded += h.join().unwrap_or_else(|_| {
                    Err(ApiError::from("download block thread panicked"))
                })?;
            }
            Ok::<_, ApiError>(())
        })?;
        Ok(downloaded)
    }

    /// 单块下载:Range 请求 [start, start+len),206 校验,定位写入本地文件对应偏移
    fn download_block(
        agent: &ureq::Agent,
        url: &str,
        dst: &str,
        start: u64,
        blk_len: u64,
    ) -> Result<u64, ApiError> {
        let mut response = agent
            .get(url)
            .header("User-Agent", "pan.baidu.com")
            .header("Range", &format!("bytes={}-{}", start, start + blk_len - 1))
            .call()
            .map_err(|e| ApiError::from(format!("send download request error: {}", e).as_str()))?;
        let status = response.status().as_u16();
        if status != 206 {
            let text = response
                .body_mut()
                .read_to_string()
                .map_err(|e| ApiError::from(format!("decode download error: {}", e).as_str()))?;
            return parse_response(status, text).map(|_| 0);
        }
        // 定位写入(不 truncate:文件已在并行入口 set_len 预置)
        let mut options = std::fs::OpenOptions::new();
        options.write(true);
        let mut file = options
            .open(dst)
            .map_err(|e| ApiError::from(format!("open local file error: {}", e).as_str()))?;
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(start))
            .map_err(|e| ApiError::from(format!("seek local file error: {}", e).as_str()))?;
        Self::stream_to_file(response.body_mut().as_reader(), &mut file)
    }

    /// 流式落盘:64KB 缓冲循环 read -> write,返回写入字节数
    ///
    /// 必须完整读到 EOF(ureq Agent 连接池要求 body 读净才能复用连接)
    fn stream_to_file(
        mut reader: impl std::io::Read,
        file: &mut std::fs::File,
    ) -> Result<u64, ApiError> {
        let mut buf = [0u8; 64 * 1024];
        let mut total: u64 = 0;
        loop {
            let n = match reader.read(&mut buf) {
                Ok(n) => n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => {
                    return Err(ApiError::from(format!(
                        "read download body error: {}",
                        e
                    )
                    .as_str()))
                }
            };
            if n == 0 {
                break; // EOF = 下载完成
            }
            file.write_all(&buf[..n]).map_err(|e| {
                ApiError::from(format!("write local file error: {}", e).as_str())
            })?;
            total += n as u64;
        }
        Ok(total)
    }

    /// 下载 URL 拼接: dlink 必须带 access_token(实测缺失返回 403 user not exists)
    ///
    /// 已带 `access_token` 参数时不重复追加
    fn with_access_token(dlink: &str, token: &str) -> String {
        if dlink.contains("access_token=") {
            dlink.to_string()
        } else {
            format!("{dlink}&access_token={token}")
        }
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
        let status: u16 = 400;
        let text = r#"{"error_code": 31061, "error_msg": "file already exists"}"#.to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 31061);
        assert!(format!("{}", error).contains("file already exists"));
    }

    #[test]
    fn test_parse_response_http_error_with_errno() {
        let status: u16 = 403;
        let text = r#"{"errno": 31034, "errmsg": "hit frequency control"}"#.to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 31034);
        let display = format!("{}", error);
        assert!(display.contains("hit frequency control"));
    }

    #[test]
    fn test_parse_response_http_error_non_json_body() {
        let status: u16 = 502;
        let text = "<html>502 Bad Gateway</html>".to_string();
        let result = parse_response(status, text);
        let error = result.unwrap_err();
        assert_eq!(error.ret_errno(), 8989);
        let display = format!("{}", error);
        assert!(display.contains("HTTP status 502"));
    }

    #[test]
    fn test_yunapi_send_sync() {
        // 编译期断言: YunApi 可跨线程共享(用户层文件级并发的前提,防未来改动破坏)
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<YunApi>();
    }

    #[test]
    fn test_with_access_token_appends() {
        // dlink 不带 access_token(正常情况,filemetas 返回的 dlink 无 token)
        let dlink = "https://d.pcs.baidu.com/file/abc?fid=1&sign=xx";
        let url = YunApi::with_access_token(dlink, "tok123");
        assert_eq!(url, format!("{dlink}&access_token=tok123"));
    }

    #[test]
    fn test_with_access_token_no_duplicate() {
        // dlink 已带 access_token 时不重复追加(调用方自行拼接过的场景)
        let dlink = "https://d.pcs.baidu.com/file/abc?fid=1&access_token=already";
        let url = YunApi::with_access_token(dlink, "tok123");
        assert_eq!(url, dlink);
    }

    #[test]
    fn test_parse_response_success() {
        let status: u16 = 200;
        let text = r#"{"errno": 0, "list": [1, 2]}"#.to_string();
        let result = parse_response(status, text);
        let value = result.unwrap();
        assert_eq!(value["errno"], 0);
        assert_eq!(value["list"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_parse_response_success_invalid_json() {
        let status: u16 = 200;
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
