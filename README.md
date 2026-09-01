[![github](https://img.shields.io/badge/baiduyun__api-crate-green)](https://github.com/hfh1999/baiduyun_api/)
# 通告

主要更新有:
- 版本号更新
- 进行了一些内部优化，修复了一些bug
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
- [x] 提供写操作: 创建文件夹(mkdir)、删除(remove)、移动/复制(mv/cp)、重命名(rename)
- [x] 提供单步上传(upload, ≤2GB, 支持冲突策略ondup)
- [x] 提供了较为详细的报错信息.(使用ApiError类)

# 授权工具(快速获取 access_token)

```bash
cargo run --example authorize -- --app-key=你的APP_KEY
```

会自动打开浏览器进入授权页,授权后把地址栏的完整 URL 粘贴回终端,工具自动提取
token 并写入 `.env`(已 gitignore,不会提交)。更多用法见 [examples/authorize.rs](examples/authorize.rs)。

# Todo
- [ ] 提供大文件分片上传(三步上传: 预上传/分片/创建文件)
- [ ] 提供分享服务(创建分享链接/提取码/转存)
- [ ] 完善方便开发的设施(分页迭代器、上传进度回调等)
- [ ] 修复 util::download 的 unwrap 链与超时配置

# 远期计划
- [ ] 上传时不限制于/app文件夹中. --> 暂时没有动工
