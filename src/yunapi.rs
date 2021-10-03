use super::error::ApiError;
use super::FilePtr;
use super::QuotaInfo;
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

    ///根据目录名得到该目录下的文件列表
    ///
    ///其中参数dir表示目录名,limit表示每次最多的条数(即每页limit个条目),start表示当前查询的总序号.
    ///limit不可超过10000
    ///返回信息的具体的字段见[FilePtr]
    pub fn get_file_list(
        &self,
        dir: &str,
        start: i64,
        limit: i64,
    ) -> Result<Vec<FilePtr>, ApiError> {
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
                let file_info: FilePtr =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
                file_vec.push(file_info);
            }
            return Ok(file_vec);
        } else {
            return Err(ApiError::new(errno, "Get files list error."));
        }
    }

    //pub fn get_file_dlink(&self,file:&FilePtr)->Result<String,ApiError>{
    //}

    ///根据提供的文件列表返回相应的下载链接
    ///
    ///注意:
    ///- 传递的列表中只处理文件类型，而不处理目录类型
    ///- 得到的链接只存活8小时
    pub fn get_files_dlink_vec(&self, files: Vec<FilePtr>) -> Result<Vec<String>, ApiError> {
        //先构造参数
        let mut array_string = String::new();
        let len = files.len();
        for (index, item) in files.iter().filter(|f| f.isdir == 0).enumerate() {
            array_string.push_str(&item.fs_id.to_string());
            if index != len - 1 {
                array_string.push(',');
            }
        }
        let args = format!("fsids=[{}];dlink=1", array_string);

        let value = self.reqest(YunNode::GetFileInfo, &args).unwrap();
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
    pub fn get_file_dlink(&self, file: FilePtr) -> Result<String, ApiError> {
        let file_vec = vec![file];
        match self.get_files_dlink_vec(file_vec) {
            Ok(mut link_vec) => {
                //这里直接取出,无需复制
                let link = std::mem::take(&mut link_vec[0]);
                Ok(link)
            }
            Err(error) => Err(ApiError::new(error.ret_errno(), "Get file dlink error.")),
        }
    }
}
