//! 这是一个rust写成的百度云api库, **不提供**作弊功能!
//!
//! 
//!# 简介
//!这个库提供方便地使用百度云官方api的方法
//!
//! 对用户的云盘进行访问前首先要获取access_token,具体请看官网的[这里](https://pan.baidu.com/union/document/entrance#%E6%8E%A5%E5%85%A5%E6%B5%81%E7%A8%8B)
//!
//!**注意:本库不提供作弊功能!!!**
//!# 1.列出用户信息
//!```
//!use baiduyun_api::YunApi;
//!//...
//!//--snip--
//!//...
//!let api = YunApi::new();
//!let access_token ="User's access_token";
//!let user_info = test.get_user_info().unwrap();
//!    println!("baidu_name :{}", user_info.baidu_name);
//!    println!("vip :{}", user_info.vip_type);
//!```

use reqwest::blocking;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt::Display;
use reqwest::header::USER_AGENT;    
macro_rules! arg_to_string {
    ($item:expr) => {{
        trait PrintAsArg {
            fn as_arg_str(&self) -> String{
               String::new()
            }
        }

        impl PrintAsArg for Vec<i64>{
            fn as_arg_str(&self) -> String{
                let mut tmp_string = String::new();
                let len = self.len();
                for (index,item) in self.iter().enumerate(){
                    tmp_string.push_str(&(item.to_string()));
                    if index != len - 1{
                        tmp_string.push(',');
                    }
                }
                format!("[{}]",tmp_string)
            }
    }
        impl PrintAsArg for i64{
            fn as_arg_str(&self) -> String{
                self.to_string()
            }
        }
        impl PrintAsArg for &str{
            fn as_arg_str(&self) -> String{
                self.to_string()
            }
        }
        format!("{}={}",stringify!($item),$item.as_arg_str())
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
///提供实用工具
///
pub mod util;
#[derive(Debug)]
/// 本api的专有错误类型
///
/// 可以填充和返回错误原因
pub struct ApiError {
    prompt: String,
}
impl ApiError {
    /// 新建一个ApiError类型
    ///
    /// # Example
    ///```
    ///let myerror = ApiError::new("unknow error.");
    ///println!("{}",myerror.ret_prompt());
    ///```
    pub fn new(prompt_str: &str) -> ApiError {
        ApiError {
            prompt: String::from(prompt_str),
        }
    }
    /// 返回api错误的原因
    pub fn ret_prompt(&self) -> String {
        return self.prompt.clone();
    }
}
impl Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.prompt)
    }
}

enum YunNode {
    GetUserInfo,
    GetQuotaInfo,
    GetFileList,
    GetFileInfo,
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
///用户信息结构体,由[YunApi::get_user_info()]返回
///
///包含了以下五个字段
///- baidu_name,百度账号名
///- netdisk_name,网盘账号名
///- avatar_url,头像图片url
///- vip_type,vip的类型
///- uk,用户id
#[derive(Serialize, Deserialize)]
pub struct UserInfo {
    pub baidu_name: String,
    pub netdisk_name: String,
    pub avatar_url: String, // 头像的url
    pub vip_type: i64,      //vip 类型
    pub uk: i64,            //用户id
}

///配额信息结构体,由[YunApi::get_quota_info()]返回
///
///包含了以下四个字段
///- total,总空间大小
///- expire,7天内是否由容量到期
///- used,已使用大小,单位B
///- free,剩余大小,单位B
///
///要想要方便的进行单位转换参看[这个函数](util::human_quota())
#[derive(Serialize, Deserialize)]
pub struct QuotaInfo {
    pub total: i64,
    pub expire: bool,
    pub used: i64,
    pub free: i64,
}

///文件信息结构体
///
///包含了以下字段：
///- path,文件的绝对路径
///- category,文件类型:1 视频、2 音频、3 图片、4 文档、5 应用、6 其他、7 种子
///- fs_id,文件在云端的唯一标识ID
///- isdir,是否目录，0 文件、1 目录
///- local_ctime,文件在客户端创建时间
///- local_mtime,文件在客户端修改时间
///- server_ctime,文件在服务器创建时间
///- server_mtime,文件在服务器修改时间
///- server_filename,文件名称
///- md5,文件的md5值，只有是文件类型时，该KEY才存在
///- size,文件大小,单位B,要想要方便的进行单位转换参看[这个函数](util::human_quota())
///- thumbs,只有请求参数带WEB且该条目分类为图片时，该KEY才存在，包含三个尺寸的缩略图URL
///- dir_empty,该目录是否存在子目录,只有请求参数带WEB且该条目为目录时,该KEY才存在,0为存在,1为不存在
#[derive(Serialize, Deserialize,Debug)]
pub struct FilePtr {
    pub path: String,
    pub category: i64,
    pub fs_id: i64,
    pub isdir: i64,
    pub local_ctime: i64,
    pub local_mtime: i64,
    pub server_ctime: i64,
    pub server_mtime: i64,
    pub server_filename: String,
    pub md5: Option<String>,
    pub size: i64,
    pub thumbs: Option<String>,
    pub dir_empty:Option<i64>,
}
///要使用本api,必须使用YunApi结构体
pub struct YunApi {
    access_token: String,
    client: blocking::Client,
    //pwd: String, //当前路径
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
        let arg_vec: Vec<&str> = args.split(';').filter(|s| *s !="").collect();
        let node_addr = get_node_addr(in_node);
        if node_addr.contains('?') {
            let mut addr = format!("{}&access_token={}", node_addr, self.access_token);
            for item in arg_vec {
                addr.push_str(&format!("{}{}", '&', item));
            }
            println!("{}",addr);
            addr
        } else {
            let mut addr = format!("{}?access_token={}", node_addr, self.access_token);
            for item in arg_vec {
                addr.push_str(&format!("{}{}", '&', item));
            }
            println!("{}",addr);
            addr
        }
    }
    fn reqest(&self, in_node: YunNode, args: &str) -> Result<Value, ApiError> {
        if let Ok(send_result) = self.client.get(self.get_addr(in_node, args)).header(USER_AGENT, "pan.baidu.com").send() {
            if let Ok(text) = send_result.text() {
                let tmp = Ok(serde_json::from_str(&text).unwrap());
                println!("{:?}",tmp);
                tmp
            } else {
                return Err(ApiError::new("decode text error."));
            }
        } else {
            return Err(ApiError::new("send request error."));
        }
    }
    ///得到用户的基本信息
    ///
    ///返回信息的具体字段参见[UserInfo]
    pub fn get_user_info(&self) -> Result<UserInfo, ApiError> {
        let value = self.reqest(YunNode::GetUserInfo, "").unwrap();
        if value["errno"].as_i64().unwrap() == 0 {
            return Ok(serde_json::from_value(value).unwrap());
        } else {
            return Err(ApiError::new("Get User infomation error."));
        }
    }

    ///得到网盘的空间占用信息
    ///
    ///返回信息的具体的字段见[QuotaInfo]
    pub fn get_quota_info(&self) -> Result<QuotaInfo, ApiError> {
        let value = self.reqest(YunNode::GetQuotaInfo, "").unwrap();
        if value["errno"].as_i64().unwrap() == 0 {
            return Ok(serde_json::from_value(value).unwrap());
        } else {
            return Err(ApiError::new("Get quta infomation error."));
        }
    }

    ///根据目录名得到该目录下的文件列表
    ///
    ///其中参数dir表示目录名,limit表示每次最多的条数(即每页limit个条目),start表示当前查询的总序号.
    ///
    ///返回信息的具体的字段见[FilePtr]
    pub fn get_file_list(
        &self,
        dir: &str,
        start: i64,
        limit: i64,
    ) -> Result<Vec<FilePtr>, ApiError> {
        if limit < 0 || limit > 10000 {
            return Err(ApiError::new("limit arg error."));
        }
        if start < 0 {
            return Err(ApiError::new("start arg error."));
        }
        let value = self.reqest(YunNode::GetFileList, &args!(dir,start,limit)).unwrap();
        if value["errno"].as_i64().unwrap() == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut file_vec = Vec::new();
            for index in 0..len {
                let file_info: FilePtr =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
                file_vec.push(file_info);
            }
            return Ok(file_vec);
        } else {
            return Err(ApiError::new("Get files list error."));
        }
    }

    //pub fn get_file_dlink(&self,file:&FilePtr)->Result<String,ApiError>{
    //}

    ///根据提供的文件列表返回相应的下载链接
    ///
    ///注意:
    ///- 传递的列表中只处理文件类型，而不处理目录类型
    ///- 得到的链接只存活8小时
    pub fn get_files_dlink_vec(&self,files:Vec<FilePtr>)->Result<Vec<String>,ApiError>{
        //先构造参数
        let mut array_string = String::new();
        let len = files.len();
        for (index,item) in files.iter().filter(|f| f.isdir == 0).enumerate(){
            array_string.push_str(&item.fs_id.to_string());
            if index != len - 1{
                array_string.push(',');
            }
        }
        let args = format!("fsids=[{}];dlink=1",array_string);

        let value = self.reqest(YunNode::GetFileInfo, &args).unwrap();
        if value["errno"].as_i64().unwrap() == 0 {
            let mut dlink_vec:Vec<String> = Vec::new();
            let len = value["list"].as_array().unwrap().len();
            for index in 0..len{
                let dlink = value["list"][index]["dlink"].as_str().unwrap().to_string();
                dlink_vec.push(dlink);
            }
            Ok(dlink_vec)

        }else{
            return Err(ApiError::new("Get file dlink error."));
        }
    }
}
