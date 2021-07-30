//!一些方便开发的实用设施
//!包括:
//!- 单线程及多线程下载设施
//!- 单位转换之设施
//!- 目录结构之设施
/// 提供方便的容量大小转换
///
///返回是一个元组,从左往右依次是转换为KB,MB,GB的值,用浮点数表示
pub fn human_quota(in_quta:i64)->(f64,f64,f64) {
    let tmp_quota = in_quta as f64;
    let k = 1024 as f64;
    let m = (1024 * 1024) as f64;
    let g = (1024 * 1024 * 1024) as f64;
    (tmp_quota/k,tmp_quota/m,tmp_quota/g)
}
