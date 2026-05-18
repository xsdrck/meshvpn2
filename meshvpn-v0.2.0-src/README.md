# MeshVPN - 现代去中心化 VPN

一个现代化的、类似 Tailscale/ZeroTier/WireGuard 架构的去中心化 VPN 项目。

## 项目特点

- ✅ 完全使用 Rust 编写
- ✅ X25519 密钥交换 (类似 WireGuard)
- ✅ ChaCha20Poly1305 AEAD 加密
- ✅ Mesh 网络架构
- ✅ 支持 NAT 穿透 (STUN/ICE)
- ✅ 模块化架构设计

## 快速开始

### 前置要求

- Rust 1.75+ 
- `cargo` 包管理器

### 编译运行

```bash
# 开发编译
cargo build

# 生产编译 (优化版本)
cargo build --release

# 查看帮助
./target/release/meshvpn --help
```

### 使用方法

#### 1. 生成配置文件

```bash
./target/release/meshvpn generate
# 将生成:
# - meshvpn.toml (配置文件)
# - meshvpn.key (私钥)
```

#### 2. 查看公钥

```bash
./target/release/meshvpn pubkey
```

#### 3. 启动节点

```bash
./target/release/meshvpn start
```

## 项目架构

```
src/
├── lib.rs          # 主库入口
├── main.rs         # CLI 入口
├── config.rs       # 配置管理
├── crypto.rs       # 加密模块
├── errors.rs       # 错误处理
├── tun.rs          # TUN 设备
├── node.rs         # Mesh 节点
└── tunnel/
    ├── mod.rs
    └── wireguard.rs # WireGuard 协议
```

## 技术栈

- **异步运行时**: Tokio
- **加密**: X25519, ChaCha20Poly1305, SHA2
- **序列化**: Serde, TOML, Bincode
- **CLI**: Clap
- **日志**: Tracing

## 许可证

MIT/Apache-2.0
