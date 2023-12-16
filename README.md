[![github](https://img.shields.io/badge/baiduyun__api-crate-green)](https://github.com/hfh1999/baiduyun_api/)
# 通告

主要更新有:
- 版本变为0.2.5
- 增加了搜索的接口 search_with_key()
- 增加了接口 get_files_info()
- 更改部分接口的返回值使其返回迭代器
- 更新了文档.

需要注意的是，目前的api并**不稳定**,可能会发生不少变化,我计划到0.3.0达到稳定的api,那之后只会增加api而不会变动api

# 方便的使用官方Api

提供了方便的Rust接口,及相关的实用设施

我的github仓库在 [这里](https://github.com/hfh1999/baiduyun_api) ，欢迎提出你的意见.

# 文档
请看[这个文档](https://docs.rs/baiduyun_api/)，目前正在完善中

# 已经支持
- [x] 提供基本的文件信息访问(容量，大小，时间，md5等)
- [x] 提供搜索接口(使用字符串来搜索,支持递归,翻页,支持中文字符搜索)
- [x] 提供下载链接提取(可以从get_files_info,或者是get_file_dlink_vec来获取)
- [x] 提供云盘存储，用户基本信息访问(获取当前云盘的总容量,已用值;可以查看用户的昵称,等信息) 
- [x] 提供了较为详细的报错信息.(使用ApiError类)

# Todo
- [ ] 提供方便的单线程乃至多线程下载接口 --> 目前只是提供了一个简单的下载测试例程
- [ ] 完善方便开发的设施 --> 目前初步编写了一个YunFs的设施以及单位换算设施
- [ ] 提供上传接口 --> 正要开始

# 远期计划
- [ ] 上传时不限制于/app文件夹中. --> 暂时没有动工
