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
            String::from("http://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas")
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
    fn get_addr<T: Serialize>(&self, in_node: YunNode, params: &T) -> String {
        let node_addr = get_node_addr(in_node);
        let query_string = to_string(params).unwrap_or_default();

        if node_addr.contains('?') {
            let mut addr = format!("{}&access_token={}", node_addr, self.access_token);
            if !query_string.is_empty() {
                addr.push_str(&format!("&{}", query_string));
            }
            addr
        } else {
            let mut addr = format!("{}?access_token={}", node_addr, self.access_token);
            if !query_string.is_empty() {
                addr.push_str(&format!("&{}", query_string));
            }
            addr
        }
    }
    fn reqest<T: Serialize>(&self, in_node: YunNode, params: &T) -> Result<Value, ApiError> {
        let addr = self.get_addr(in_node, params);
        if let Ok(send_result) = self
            .client
            .get(&addr)
            .header(USER_AGENT, "pan.baidu.com")
            .send()
        {
            if let Ok(text) = send_result.text() {
                Ok(serde_json::from_str(&text).unwrap())
            } else {
                Err(ApiError::from("decode text error."))
            }
        } else {
            Err(ApiError::from("send request error."))
        }
    }
    ///得到用户的基本信息
    ///
    ///返回信息的具体字段参见[UserInfo]
    pub fn get_user_info(&self) -> Result<UserInfo, ApiError> {
        let params = EmptyParams;
        let value = self.reqest(YunNode::GetUserInfo, &params).unwrap();
        let error = value["errno"].as_i64().unwrap();
        if error == 0 {
            Ok(serde_json::from_value(value).unwrap())
        } else {
            Err(ApiError::new(error, "Get User infomation error."))
        }
    }

    ///得到网盘的空间占用信息
    ///
    ///返回信息的具体的字段见[QuotaInfo]
    pub fn get_quota_info(&self) -> Result<QuotaInfo, ApiError> {
        let params = EmptyParams;
        let value = self.reqest(YunNode::GetQuotaInfo, &params).unwrap();
        let error = value["errno"].as_i64().unwrap();
        if error == 0 {
            Ok(serde_json::from_value(value).unwrap())
        } else {
            Err(ApiError::new(error, "Get quta infomation error."))
        }
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
        let value = self.reqest(YunNode::GetFileInfo, &params).unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut info_vec = Vec::new();
            for index in 0..len {
                let file_info: FileInfoEx = FileInfoEx {
                    category: value["list"][index]["category"].as_i64().unwrap(),
                    date_taken: value["list"][index]["date_taken"].as_i64().unwrap(),
                    dlink: value["list"][index]["dlink"].as_str().unwrap().to_string(),
                    file_name: value["list"][index]["file_name"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                    height: value["list"][index]["height"].as_i64().unwrap(),
                    is_dir: value["list"][index]["is_dir"].as_i64().unwrap(),
                    server_ctime: value["list"][index]["server_ctime"].as_i64().unwrap(),
                    server_mtime: value["list"][index]["server_mtime"].as_i64().unwrap(),
                    size: value["list"][index]["size"].as_i64().unwrap(),
                    width: value["list"][index]["width"].as_i64().unwrap(),
                };
                info_vec.push(file_info);
            }
            Ok(info_vec)
        } else {
            Err(ApiError::new(errno, "Get files info error."))
        }
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
        let value = self.reqest(YunNode::GetFileList, &params).unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut file_vec = Vec::new();
            for index in 0..len {
                let file_info: FileInfo =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
                file_vec.push(file_info);
            }
            Ok(FileInfoIter::new(file_vec))
        } else {
            Err(ApiError::new(errno, "Get files list error."))
        }
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

        let value = self.reqest(YunNode::GetFileInfo, &params).unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let mut dlink_vec: Vec<String> = Vec::new();
            let len = value["list"].as_array().unwrap().len();
            for index in 0..len {
                let dlink = value["list"][index]["dlink"].as_str().unwrap().to_string();
                dlink_vec.push(dlink);
            }
            Ok(dlink_vec)
        } else {
            Err(ApiError::new(errno, "Get files dlinks error."))
        }
    }

    /// 和 [get_files_dlink_vec]类似,但是只查询单个文件
    ///
    /// 只有实现了[FileId] trait的类型可以用在这里
    pub fn get_file_dlink<T>(&self, file: T) -> Result<String, ApiError>
    where
        T: FileId,
    {
        let file_vec = vec![file];
        match self.get_files_dlink_vec(&file_vec) {
            Ok(mut link_vec) => {
                //这里直接取出,无需复制
                let link = std::mem::take(&mut link_vec[0]);
                Ok(link)
            }
            Err(error) => Err(ApiError::new(error.ret_errno(), "Get file dlink error.")),
        }
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
        let value = self.reqest(YunNode::Search, &params).unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut search_vec = Vec::new();
            for index in 0..len {
                let file_info: SearchResult =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
                search_vec.push(file_info);
            }
            Ok(search_vec)
        } else {
            Err(ApiError::new(errno, "Get files info error."))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_addr_with_existing_query_params() {
        let api = YunApi::new("test_token");
        let params = EmptyParams;
        let addr = api.get_addr(YunNode::GetUserInfo, &params);
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
        let addr = api.get_addr(YunNode::GetFileList, &params);
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
        let addr = api.get_addr(YunNode::Search, &params);
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
        let addr = api.get_addr(YunNode::GetFileInfo, &params);
        assert!(addr.contains("fsids=%5B123%2C456%5D"));
        assert!(addr.contains("dlink=1"));
        assert!(addr.contains("extra=1"));
        assert!(addr.contains("access_token=test_token"));
    }

    #[test]
    fn test_get_addr_with_quota_info() {
        let api = YunApi::new("test_token");
        let params = EmptyParams;
        let addr = api.get_addr(YunNode::GetQuotaInfo, &params);
        assert_eq!(
            addr,
            "https://pan.baidu.com/api/quota?checkfree=1&checkexpire=1&access_token=test_token"
        );
    }
}
