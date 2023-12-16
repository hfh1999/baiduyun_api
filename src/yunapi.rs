use super::error::ApiError;
use super::FileId;
use super::FileInfo;
use super::FileInfoEx;
use super::FileInfoIter;
use super::QuotaInfo;
use super::SearchResult;
use super::UserInfo;
use reqwest::blocking;
use reqwest::header::USER_AGENT;
use serde_json::Value;
macro_rules! arg_to_string {
    ($item:expr) => {{
        trait PrintAsArg {
            fn as_arg_str(&self) -> String {
                String::new()
            }
        }

        impl PrintAsArg for Vec<i64> {
            fn as_arg_str(&self) -> String {
                let mut tmp_string = String::new();
                let len = self.len();
                for (index, item) in self.iter().enumerate() {
                    tmp_string.push_str(&(item.to_string()));
                    if index != len - 1 {
                        tmp_string.push(',');
                    }
                }
                format!("[{}]", tmp_string)
            }
        }
        impl PrintAsArg for i64 {
            fn as_arg_str(&self) -> String {
                self.to_string()
            }
        }
        impl PrintAsArg for &str {
            fn as_arg_str(&self) -> String {
                self.to_string()
            }
        }
        format!("{}={}", stringify!($item), $item.as_arg_str())
    }};
}
//用作参数构造的宏
macro_rules! args {
    ($($item:expr),+) => {{
        let mut tmp_string = String::new();
       $(
            tmp_string.push_str(&arg_to_string!($item));
            tmp_string.push_str(";");
       )
       +
       tmp_string
    }};
}
enum YunNode {
    GetUserInfo,
    GetQuotaInfo,
    GetFileList,
    GetFileInfo,
    Search,
    PreCreate, // 三步上传,1st
    UpLoad,    //2ed
    Create,    //3rd
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
    fn get_addr(&self, in_node: YunNode, args: &str) -> String {
        let arg_vec: Vec<&str> = args.split(';').filter(|s| *s != "").collect();
        let node_addr = get_node_addr(in_node);
        if node_addr.contains('?') {
            let mut addr = format!("{}&access_token={}", node_addr, self.access_token);
            for item in arg_vec {
                addr.push_str(&format!("{}{}", '&', item));
            }
            //debug;;; println!("{}", addr);
            addr
        } else {
            let mut addr = format!("{}?access_token={}", node_addr, self.access_token);
            for item in arg_vec {
                addr.push_str(&format!("{}{}", '&', item));
            }
            println!("{}", addr);
            addr
        }
    }
    fn reqest(&self, in_node: YunNode, args: &str) -> Result<Value, ApiError> {
        if let Ok(send_result) = self
            .client
            .get(self.get_addr(in_node, args))
            .header(USER_AGENT, "pan.baidu.com")
            .send()
        {
            if let Ok(text) = send_result.text() {
                let tmp = Ok(serde_json::from_str(&text).unwrap());
                // debug;;;   println!("{:?}", tmp);
                tmp
            } else {
                return Err(ApiError::new(8989, "decode text error."));
            }
        } else {
            return Err(ApiError::new(8989, "send request error."));
        }
    }
    ///得到用户的基本信息
    ///
    ///返回信息的具体字段参见[UserInfo]
    pub fn get_user_info(&self) -> Result<UserInfo, ApiError> {
        let value = self.reqest(YunNode::GetUserInfo, "").unwrap();
        let error = value["errno"].as_i64().unwrap();
        if error == 0 {
            return Ok(serde_json::from_value(value).unwrap());
        } else {
            return Err(ApiError::new(error, "Get User infomation error."));
        }
    }

    ///得到网盘的空间占用信息
    ///
    ///返回信息的具体的字段见[QuotaInfo]
    pub fn get_quota_info(&self) -> Result<QuotaInfo, ApiError> {
        let value = self.reqest(YunNode::GetQuotaInfo, "").unwrap();
        let error = value["errno"].as_i64().unwrap();
        if error == 0 {
            return Ok(serde_json::from_value(value).unwrap());
        } else {
            return Err(ApiError::new(error, "Get quta infomation error."));
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
        let value = self
            .reqest(YunNode::GetFileInfo, &args!(fsids, dlink, extra))
            .unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut info_vec = Vec::new();
            for index in 0..len {
                // 这里忽略了thumb的解析.
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
            return Ok(info_vec);
        } else {
            return Err(ApiError::new(errno, "Get files info error."));
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
        if limit < 0 || limit > 10000 {
            return Err(ApiError::new(8989, "limit arg error."));
        }
        if start < 0 {
            return Err(ApiError::new(8989, "start arg error."));
        }
        let value = self
            .reqest(YunNode::GetFileList, &args!(dir, start, limit))
            .unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut file_vec = Vec::new();
            for index in 0..len {
                let file_info: FileInfo =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
                file_vec.push(file_info);
            }
            return Ok(FileInfoIter::new(file_vec));
        } else {
            return Err(ApiError::new(errno, "Get files list error."));
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
        //先构造参数
        let fsids: Vec<i64> = files.iter().map(|x| x.ret_file_id()).collect();
        let dlink = 1_i64;

        let value = self
            .reqest(YunNode::GetFileInfo, &args!(fsids, dlink))
            .unwrap();
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
            return Err(ApiError::new(errno, "Get files dlinks error."));
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
        let key = search_key;
        let dir = search_dir;
        let recursion = is_recursive as i64;
        let page = in_page;
        let num = in_num;
        let web = in_web as i64;
        if page < 1 {
            return Err(ApiError::new(8989, "Page is less than 1."));
        }
        if num > 1000 {
            return Err(ApiError::new(8989, "Num is more than 1000."));
        }
        let value = self
            .reqest(YunNode::Search, &args!(key, dir, recursion, page, num, web))
            .unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut search_vec = Vec::new();
            for index in 0..len {
                let file_info: SearchResult =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
                search_vec.push(file_info);
            }
            return Ok(search_vec);
        } else {
            return Err(ApiError::new(errno, "Get files info error."));
        }
    }
}
