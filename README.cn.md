# Cargo

Cargo可以管理Rust项目的依赖，并编译它们

阅读 https://doc.rust-lang.org/cargo/ 了解更多

## 项目状态

[![Build Status](https://dev.azure.com/rust-lang/cargo/_apis/build/status/rust-lang.cargo?branchName=auto-cargo)](https://dev.azure.com/rust-lang/cargo/_build?definitionId=18)

文档: https://docs.rs/cargo/

## 安装

Cargo默认与Rust一起安装，如果你在本地安装了`rustc`，那么`cargo`一般已经安装在本地了。

## 从源代码编译

构建Cargo需要这些工具和包:

* `git`
* `curl` (在 Unix 系统上)
* `pkg-config` (在 Unix 系统上, 用于获取 `libssl` 的headers/libraries)
* OpenSSL headers (仅针对 Unix 系统, 在 Ubuntu 上是 `libssl-dev`)
* `cargo` 和 `rustc`

首先，你需要下载这个存储库

```
git clone https://github.com/rust-lang/cargo
cd cargo
```

如果你已经安装了 `cargo`，那么你可以直接运行这行指令：

```
cargo build --release
```

## 为 Cargo 创作新的子命令

Cargo被设计为可以轻松拓展新的子命令，而无需修改Cargo本身。
在 [这一页][third-party-subcommands] 可以查看更多信息和一些社区开发的子命令。

[第三方子命令]: https://github.com/rust-lang/cargo/wiki/Third-party-cargo-subcommands


## 发行版

Cargo 与 Rust 版本同时发布。
高版本的信息可以在[Rust's release notes][rel]中查看。
发行版的更多信息可以在这个存储库或者[CHANGELOG.md]中查看。

[rel]: https://github.com/rust-lang/rust/blob/master/RELEASES.md
[CHANGELOG.md]: CHANGELOG.md

## 报告问题

我们很乐意了解新发现的bug！

请将所有的问题回报到 GitHub 的 [issue tracker][issues] 上。

[issues]: https://github.com/rust-lang/cargo/issues

## 做出贡献

可以在 **[Cargo Contributor Guide]** 上查看为Cargo做出贡献的完整介绍。

[Cargo Contributor Guide]: https://rust-lang.github.io/cargo/contrib/

## 许可证

Cargo 主要根据 MIT 和 Apache 许可证（2.0 版）分发。

详见 [LICENSE-APACHE](LICENSE-APACHE) 和 [LICENSE-MIT](LICENSE-MIT)。

### 第三方软件

这个产品包含OpenSSL开发的OpenSSL Tookit（https://www.openssl.org/） 。


在二进制版本，这个产品包含以GNU General Public License, 版本 2发行的软件。
它们可以在这里看到：[上游存储库][1]。

详见 [LICENSE-THIRD-PARTY](LICENSE-THIRD-PARTY) 。

[1]: https://github.com/libgit2/libgit2

