[![github](https://img.shields.io/badge/baiduyun__api-crate-green)](https://github.com/hfh1999/baiduyun_api/)
# 通告
由于我的大失误,竟然在v0.1.0及v0.1.1中包含了调试信息,因此直接将这两个版本yank掉.

主要更新有:
- 版本直接变为0.2.0，更新了基础设施，提供了更加方便地YunFs功能;
- 更新了文档.
需要注意的是，目前的api并**不稳定**,计划到0.3.0达到稳定的api.

# 方便的使用官方Api

提供了方便的Rust接口,及相关的实用设施

我的github仓库在 [这里](https://github.com/hfh1999/baiduyun_api) ，欢迎提出你的意见.

# 文档
请看[这个文档](https://docs.rs/baiduyun_api/)，目前正在完善中

# 已经支持
- [x] 提供基本的文件信息访问
- [x] 提供下载链接提取
- [x] 提供云盘存储，用户基本信息访问  

# Todo
- [ ] 提供方便的单线程乃至多线程下载接口
- [ ] 完善方便开发的设施
- [ ] 提供上传接口

# 远期计划
- [ ] 上传时不限制于/app文件夹中.
