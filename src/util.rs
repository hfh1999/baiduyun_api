//!提供实用工具
//!
//!一些方便开发的实用设施
//!包括:
//!- 单线程及多线程下载设施
//!- 单位转换之设施
//!- 目录结构之设施: [YunFs](YunFs) ,提供云端文件系统的抽象.

use super::ApiError;
use super::FilePtr;
use super::YunApi;
use std::path::PathBuf;
/// 提供方便的容量大小转换
///
///返回是一个元组,从左往右依次是转换为KB,MB,GB的值,用浮点数表示
pub fn human_quota(in_quta: i64) -> (f64, f64, f64) {
    let tmp_quota = in_quta as f64;
    let k = 1024 as f64;
    let m = (1024 * 1024) as f64;
    let g = (1024 * 1024 * 1024) as f64;
    (tmp_quota / k, tmp_quota / m, tmp_quota / g)
}

///提供方便的云目录结构,方便进行各种目录切换操作
///
///
///YunFs是Api的高级抽象,需要创建一个YunFs结构体才能使用
///
///这个云目录模型模拟进行目录浏览，让我们浏览云端文件系统如同浏览本地文件系统一样
///
///暂时提供以下几种操作:
///- 返回当前路径[pwd()](YunFs::pwd())
///- 切换路径[chdir()](YunFs::chdir())
///- 列出指定路径的所有文件list
pub struct YunFs<'a> {
    api: &'a YunApi,
    current_path: PathBuf,
}
impl<'a> YunFs<'a> {
    ///创建一个YunFs结构体
    pub fn new(api_ref: &YunApi) -> YunFs {
        YunFs {
            api: api_ref,
            current_path: PathBuf::from("/"), // 总是以绝对路径的形式
        }
    }

    ///返回当前的目录
    ///
    ///注意:每次都检查当前目录是否存在
    ///
    pub fn pwd(&self) -> Result<String, ApiError> {
        let path_string: String = self.current_path.to_str().unwrap().into();
        if let Ok(_) = self.api.get_file_list(&path_string, 0, 0) {
            Ok(path_string.clone())
        } else {
            Err(ApiError::new(
                "Error when pwd():the directory may not exist.",
            ))
        }
    }
    fn check_dir_fmt(dir_str:&str)->Result<(),ApiError>{
        // 以下这几种才是正确的目录形式
        // .[/]
        // ..[/]
        // ./dir1/dir2[/]
        // ../dir1/dir2[/]
        // /dir1/dir2/dir3[/]
        // dir1/dir2/dir3[/]

        /*下面进行路径检查,使用状态机*/
        let states = (0, 1, 2, 3, 4); //0-开始,1-开头遇到`.`,2-开头遇到`..`,3-遇到`/`,4-遇到其他字符
        let mut c_state = states.0;
        for item in dir_str.chars() {
            if item == '.' {
                if c_state == states.0 {
                    //开头是.
                    c_state = states.1;
                    continue;
                } else if c_state == states.1 {
                    //开头是..
                    c_state = states.2;
                    continue;
                } else {
                    //error.
                    return Err(ApiError::new(
                        "path resolve Error: `.` not the correct position.",
                    ));
                }
            }
            if item == '/' {
                if c_state != states.3 {
                    c_state = states.3;
                    continue;
                } else {
                    //error.
                    return Err(ApiError::new(
                        "path resolve Error: `/` not the correct position.",
                    ));
                }
            } else {
                //剩下的应该都是普通字符,特殊字符则报错
                match item {
                    '\\' => {
                        return Err(ApiError::new(
                            "path resolve Error: `\\` not the accepted char.",
                        ))
                    }
                    _ => {}
                }
                if c_state == 1 || c_state == 2{
                    return Err(ApiError::new("`.` or `..`can not be here."));
                }
                c_state = states.4;
                continue;
            }
        }
        return Ok(());
    }
    fn resolve_path(&self, dir_str: &str) -> Result<String, ApiError> {

        //先检查是否符合路径规范
        match Self::check_dir_fmt(dir_str) {
            Ok(_)=>{},
            Err(error)=>{return Err(error);}
        }

        let mut tmp_dir = String::from(dir_str);
        if dir_str == "." || dir_str == "./" {
            return Ok(self.current_path.to_str().unwrap().into());
        }

        if dir_str == ".." || dir_str == "../"{
            if self.current_path.to_str().unwrap() == "/"{
                //特殊情况,已经是系统的根了就不应该再往上找了.
                return  Ok("/".into());
            }
            else {
                let mut tmp_path = PathBuf::from(&self.current_path);
                tmp_path.pop();
                return  Ok(tmp_path.to_str().unwrap().into());
            }

        }

        //去除可能有的最后的`/`
        if tmp_dir.len()!= 1 && tmp_dir.ends_with("/") {
            tmp_dir.pop();
        }

        // ../ 开头,父目录查找
        if tmp_dir.starts_with("../") {
            tmp_dir.remove(0);
            tmp_dir.remove(0); 
            tmp_dir.remove(0); 

            if self.current_path.to_str().unwrap() == "/"{
                //特殊情况,已经是系统的根了就不应该再往上找了.
                let ret_string = format!("/{}",tmp_dir);
                return  Ok(ret_string);
            }
            else {
                let mut tmp_path = PathBuf::from(&self.current_path);
                tmp_path.pop();
                return  Ok(format!("{}/{}",tmp_path.to_str().unwrap(),tmp_dir));
            }
        }
        // 绝对目录查找
        if tmp_dir.starts_with("/") {
            return Ok(tmp_dir);
        }

        /*剩下的都是相对目录查找*/
        // ./开头,相对目录查找
        if tmp_dir.starts_with("./") {
            tmp_dir.remove(0);
            tmp_dir.remove(0);
        } 
            if self.current_path.to_str().unwrap() == "/"{//要是不处理这种特殊情况会出现解析出来为 //dir1 的情况
                return Ok(format!("/{}",tmp_dir));
            }
            let ret_string = format!("{}/{}", self.current_path.to_str().unwrap(), tmp_dir);
            return Ok(ret_string);
    }
    ///切换当前目录
    ///
    ///该函数自动定为在线,即总是在切换目录时确认路径是否存在.
    ///
    ///输入的格式有许多种如:
    ///- ".[/]"
    ///- "..[/]"
    ///- "./dir1/dir2[/]"
    ///- "../dir1/dir2[/]"
    ///- "/dir1/dir2/dir3[/]"
    ///- "dir1/dir2/dir3[/]"
    pub fn chdir(&mut self, dir_str: &str) -> Result<(), ApiError> {
        //每一次目录变动都需要进行一次在线检查,检查失败则操作失败
        let resolved_result = self.resolve_path(dir_str);
        let dir_resolved = match resolved_result {
            Ok(dir_path) => dir_path,
            Err(error) => return Err(error),
        };
        //debug;;; println!("resolved:path {}",dir_resolved);
        if let Ok(_) = self.api.get_file_list(&dir_resolved, 0, 0) {
            //将本地表示也改变为目录切换后的版本
            self.current_path = PathBuf::from(dir_resolved);
            Ok(())
        } else {
            Err(ApiError::new("Error:chdir():the directory may not exist."))
        }
    }

    ///列出当前目录的所有文件
    ///
    ///这个函数一次网络请求最多得到1000个文件,如果超过1000则需要发起多次网络请求，速度就会变慢.
    pub fn ls(&self)->Result<Vec<FilePtr>,ApiError>{
        //将所有的文件都列出来
        let list_len = 1000;
        let mut ret_vec:Vec<FilePtr> = vec![];
        loop{
            let tmp_list = match self.api.get_file_list(self.current_path.to_str().unwrap(), 0,list_len)  {
                Ok(list)  =>  list,
                Err(error) => {return Err(error);}
            }; 
            ret_vec.extend_from_slice(&tmp_list);
            if tmp_list.len() < 1000{
                break;
            }
        }
        Ok(ret_vec)
    }
}
