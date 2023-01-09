use super::error::ApiError;
use super::CreateRet;
use super::FileId;
use super::FileInfo;
use super::FileInfoEx;
use super::FileInfoIter;
use super::PreUploadRet;
use super::QuotaInfo;
use super::SearchResult;
use super::UploadNameStrategy;
use super::UploadRet;
use super::UserInfo;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate serde_qs;
use reqwest::blocking;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize, Serializer};
use serde_json::Value;
// 自定义的url的数组序列化
fn serde_vec<S>(vec_ref: &Vec<i64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut inner_str = String::new();
    let mut i = 0;
    while i < vec_ref.len() {
        inner_str.push_str(&vec_ref[i].to_string());
        if i != vec_ref.len() - 1 {
            inner_str.push_str(",");
        }
        i += 1;
    }
    let s = format!("[{}]", inner_str);
    serializer.serialize_str(&s)
}

fn serde_veced_strs<S>(vec_ref: &Vec<String>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut inner_str = String::new();
    let mut i = 0;
    while i < vec_ref.len() {
        let str_like_word = format!("\"{}\"", vec_ref[i]);
        inner_str.push_str(&str_like_word);
        if i != vec_ref.len() - 1 {
            inner_str.push_str(",");
        }
        i += 1;
    }
    let s = format!("[{}]", inner_str);
    serializer.serialize_str(&s)
}
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
        let mut addr = format!("{}&access_token={}", node_addr, self.access_token);
        for item in arg_vec {
            addr.push_str(&format!("{}{}", '&', item));
        }
        //debug;;; println!("{}", addr);
        addr
    }
    fn get_addr_test(&self, in_node: YunNode, args: &str) -> String {
        let node_addr = get_node_addr(in_node);
        let addr = format!("{}&access_token={}&{}", node_addr, self.access_token, args);
        println!("addr = {}", addr);
        addr
    }
    fn reqest(&self, in_node: YunNode, args: &str, body_data: &str) -> Result<Value, ApiError> {
        if body_data.is_empty() {
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
        } else {
            if let Ok(send_result) = self
                .client
                .post(self.get_addr(in_node, args))
                .body(body_data.to_owned())
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
                return Err(ApiError::new(8989, "send requst error."));
            }
        }
    }
    fn reqest_test(&self, in_node: YunNode, args: &str) -> Result<Value, ApiError> {
        if let Ok(send_result) = self
            .client
            .get(self.get_addr_test(in_node, args))
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
        let value = self.reqest(YunNode::GetUserInfo, "", "").unwrap();
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
        let value = self.reqest(YunNode::GetQuotaInfo, "", "").unwrap();
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
        #[derive(Deserialize, Serialize)]
        struct GetFileInfoParam {
            #[serde(serialize_with = "serde_vec")]
            fsids: Vec<i64>,
            dlink: i64,
            extra: i64,
        }
        let mut param = GetFileInfoParam {
            fsids: vec![],
            dlink: 1,
            extra: 1,
        };
        param.dlink = 1_i64;
        param.extra = 1_i64;
        param.fsids = file_ids.iter().map(|x| x.ret_file_id()).collect();
        let arg_str = serde_qs::to_string(&param).unwrap();
        let value = self.reqest_test(YunNode::GetFileInfo, &arg_str).unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let len = value["list"].as_array().unwrap().len();
            let mut info_vec = Vec::new();
            for index in 0..len {
                // 这里忽略了thumb的解析.
                let file_info: FileInfoEx =
                    serde_json::from_value(value["list"][index].clone()).unwrap();
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
    ///返回信息的具体的字段见[FilePtr]
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
            .reqest(YunNode::GetFileList, &args!(dir, start, limit), "")
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
            .reqest(YunNode::GetFileInfo, &args!(fsids, dlink), "")
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
            .reqest(
                YunNode::Search,
                &args!(key, dir, recursion, page, num, web),
                "",
            )
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

    /// 上传第一步,进行预上传
    ///
    /// - path 为想要上传的位置(/apps/{appName}/filename.filetype), appName为你申请的app名字
    /// - size 为你这次想要上传的文件总大小单位为B
    /// - isdir 表示你这次上传的文件是否为目录,是则True,否则false
    /// - rtype 表示文件命名的策略,参见 [`UploadNameStrategy`](super::UploadNameStrategy)
    /// - block_list 比扫视文件个分片MD5 数组的json串
    pub fn precreate(
        &self,
        in_path: &str,
        in_size: u64,
        in_isdir: bool,
        in_rtype: UploadNameStrategy,
        in_block_list: Vec<String>,
    ) -> Result<PreUploadRet, ApiError> {
        #[derive(Serialize, Deserialize)]
        struct PrecreateParams {
            path: String,
            size: u64,
            isdir: u8,
            autoinit: u8,
            rtype: u8,
            #[serde(serialize_with = "serde_veced_strs")]
            block_list: Vec<String>,
        }

        let params = PrecreateParams {
            path: in_path.to_owned(),
            size: in_size,
            isdir: in_isdir as u8,
            autoinit: 1u8,
            rtype: in_rtype as u8,
            block_list: in_block_list,
        };

        //let params_string = serde_qs::to_string(&params).unwrap();
        let params_string = serde_qs::to_string(&params).unwrap();
        //params_string = params_string.replace("%5B", "[");
        //params_string = params_string.replace("%5D", "]");
        //params_string = params_string.replace("%22", "\"");
        //params_string = params_string.replace("%2C", ",");
        //params_string = params_string.replace("%2F", "/");

        //debug
        //println!("parmas_string: {}", params_string);

        let value = self.reqest(YunNode::PreCreate, "", &params_string).unwrap();

        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let ret_path = value["path"].as_str().unwrap().to_owned();
            let ret_uploadid = value["uploadid"].as_str().unwrap().to_owned();
            let ret_ret_type = value["return_type"].as_u64().unwrap() as u8;
            let ret_block_list = value["block_list"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap() as u32)
                .collect::<Vec<u32>>();

            return Ok(PreUploadRet {
                path: ret_path,
                uploadid: ret_uploadid,
                return_type: ret_ret_type,
                block_list: ret_block_list,
            });
        } else {
            Err(ApiError::new(errno, "PreUpload Failed."))
        }
    }

    /// 上传第二步: 进行Upload操作
    ///
    /// - in_path 云端位置
    /// - in_uploadid 上一步返回的upload_id
    /// - in_partseq 返回的block_list所对应的序号
    /// - in_data 想要向云端写入的数据
    ///
    pub fn upload(
        self,
        in_path: &str,
        in_uploadid: &str,
        in_partseq: u64,
        in_data: String,
    ) -> Result<UploadRet, ApiError> {
        #[derive(Serialize, Deserialize)]
        struct UploadParams {
            method: String,
            #[serde(rename(serialize = "type"))]
            _type: String,
            path: String,
            uploadid: String,
            partseq: u64,
        }

        use urlencoding::encode;

        let params = UploadParams {
            method: String::from("upload"),
            _type: String::from("tmpfile"),
            path: encode(in_path).into_owned(),
            uploadid: in_uploadid.into(),
            partseq: in_partseq,
        };

        let params_string = serde_qs::to_string(&params).unwrap();
        let value = self
            .reqest(YunNode::UpLoad, &params_string, &in_data)
            .unwrap();
        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let md5_string = value["md5"].as_str().unwrap().to_owned();

            return Ok(UploadRet { md5: md5_string });
        } else {
            Err(ApiError::new(errno, "PreUpload Failed."))
        }
    }

    /// 上传第三步:用于将多个分片合并成一个文件，完成文件的上传。 备注：可以使用该接口创建文件夹。
    ///
    /// - in_path:
    /// - in_size:
    /// - is_dir:
    /// - in_rtype:
    /// - in_uploadid:
    /// - block_list:
    pub fn create(
        self,
        in_path: &str,
        in_size: usize,
        is_dir: bool,
        in_rtype: UploadNameStrategy,
        in_uploadid: &str,
        block_list: Vec<String>,
    ) -> Result<CreateRet, ApiError> {
        #[derive(Serialize, Deserialize)]
        struct CreateParams {
            path: String,
            size: usize,
            isdir: u8,
            rtype: u8,
            uploadid: String,
            #[serde(serialize_with = "serde_veced_strs")]
            block_list: Vec<String>,
        }

        let params = CreateParams {
            path: in_path.into(),
            size: in_size,
            isdir: is_dir as u8,
            rtype: in_rtype as u8,
            uploadid: in_uploadid.into(),
            block_list: block_list,
        };

        let params_string = serde_qs::to_string(&params).unwrap();
        let value = self.reqest(YunNode::Create, "", &params_string).unwrap();

        let errno = value["errno"].as_i64().unwrap();
        if errno == 0 {
            let ret_fs_id = value["fs_id"].as_u64().unwrap();
            let md5_string = value["md5"].as_str().unwrap().to_owned();
            let ret_server_filename = value["server_filename"].as_str().unwrap().to_owned();
            let ret_category = value["category"].as_u64().unwrap() as u8;
            let ret_path = value["path"].as_str().unwrap().to_owned();
            let ret_size = value["size"].as_u64().unwrap() as usize;
            let ret_ctime = value["ctime"].as_u64().unwrap();
            let ret_mtime = value["mtime"].as_u64().unwrap();
            let ret_is_dir = value["isdir"].as_u64().unwrap() as u8;

            return Ok(CreateRet {
                fs_id: ret_fs_id,
                md5: md5_string,
                server_filename: ret_server_filename,
                category: ret_category,
                path: ret_path,
                size: ret_size,
                ctime: ret_ctime,
                mtime: ret_mtime,
                isdir: ret_is_dir,
            });
        } else {
            Err(ApiError::new(errno, "PreUpload Failed."))
        }
    }
}

#[cfg(test)]
mod test {
    use super::UploadNameStrategy;
    use super::YunApi;
    use data_encoding::HEXLOWER;
    use md5::compute;
    use std::fs::read_to_string;
    use std::fs::File;
    use std::io::Read;
    #[test]
    fn test_precreate() {
        let key_file_path = "D:\\rust\\baiduyun_space\\baiduyun_api\\key.txt";
        let key = read_to_string(key_file_path).unwrap();
        let api = YunApi::new(&key);
        let mut key_file = File::open(key_file_path).unwrap();
        let file_size = key_file.metadata().unwrap().len();
        let mut data: Vec<u8> = Vec::new();
        key_file.read_to_end(&mut data).unwrap();
        let md5_result: [u8; 16] = compute(data).into();
        let md5_string = HEXLOWER.encode(&md5_result);

        let block_list = vec![md5_string];
        let ret = api
            .precreate(
                "/app/media_shell/key.txt",
                file_size,
                false,
                UploadNameStrategy::NoRename,
                block_list,
            )
            .unwrap();

        println!("path = {}", ret.path);
        assert_eq!(ret.path, "/app/media_shell/key.txt");
    }

    #[test]
    fn test_u8_array_and_string() {
        let a: [u8; 4] = [0xde, 0x12, 0x23, 0x45];
        assert_eq!(HEXLOWER.encode(&a), String::from("de122345"));
    }
}
