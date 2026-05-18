# 🎯 GitHub 上传 - 傻瓜式完整教程

## 让 GitHub 免费帮你编译好 Windows EXE！

---

## 📋 第一步：创建 GitHub 账号（如果你没有的话）

1. 访问 https://github.com/
2. 点击 "Sign up"
3. 填写：用户名、邮箱、密码
4. 完成验证
5. **记住你的用户名！**

---

## 📁 第二步：下载并解压源代码

1. 下载 `meshvpn-v0.2.0-src.tar.gz`
2. 解压到一个文件夹，例如：`C:\Users\你的用户名\meshvpn`

---

## 🌐 第三步：在 GitHub 创建仓库

### 3.1 点击新建仓库

1. 登录 GitHub
2. 点击右上角 **"+"** 按钮
3. 选择 **"New repository"**

### 3.2 填写仓库信息

```
Repository name: meshvpn
Description: MeshVPN - Modern decentralized VPN
（随便填，或者留空）

Owner: 你的用户名（默认）
Repository type: Public（公共）或者 Private（私有）都可以

⚠️ 重要：不要勾选以下任何选项！
❌ 不要勾选 "Add a README file"
❌ 不要勾选 "Add .gitignore"
❌ 不要勾选 "Choose a license"

然后点击 "Create repository"
```

### 3.3 你会看到这样的页面

```
…or create a new repository on the command line
echo "# meshvpn" >> README.md
git init
git add README.md
git commit -m "first commit"
git branch -M main
git remote add origin https://github.com/你的用户名/meshvpn.git
git push -u origin main

…or push an existing repository from the command line
git remote add origin https://github.com/你的用户名/meshvpn.git
git branch -M main
git push -u origin main
```

---

## 💻 第四步：上传你的代码

### 方法 A：最简单 - 使用网页上传（推荐！）

#### 4.1 在仓库页面，点击 "uploading an existing file"

在新建的仓库页面，往下滚动，找到链接 "uploading an existing file"

#### 4.2 上传文件

1. **拖拽** 你解压的 `meshvpn` 文件夹里的所有内容到这个页面
   - 或者点击 "choose your files" 选择文件

2. **注意**：只需要上传这些文件/文件夹：
   - ✅ `Cargo.toml`
   - ✅ `Cargo.lock`
   - ✅ `README.md`
   - ✅ `src/` 文件夹
   - ✅ `.github/` 文件夹
   - ✅ 其他 .md 文件

3. **不要上传** `target/` 文件夹（太大了！）

#### 4.3 提交

1. 在 "Commit changes" 部分填写：
   ```
   Commit message: Initial commit
   ```
2. 点击绿色按钮 **"Commit changes"**

#### 4.4 完成！

你会看到你的文件已经出现在仓库里了！

---

## ⚙️ 第五步：触发自动构建

### 5.1 进入 Actions 页面

1. 在仓库页面，点击顶部的 **"Actions"** 标签
2. 你会看到 "Build and Release" 工作流

### 5.2 运行构建

**方法 1：手动触发（最简单！）**

1. 点击左侧的 **"Build and Release"**
2. 点击右侧的 **"Run workflow"** 按钮（是灰色的）
3. 点击绿色 **"Run workflow"** 按钮
4. **等待！** 一般需要 5-10 分钟

**方法 2：推送代码触发**

在本地执行：
```bash
git add .
git commit -m "Trigger build"
git push
```

### 5.3 监控构建进度

1. 点击构建任务
2. 看到进度条在走
3. 等待所有任务变成绿色勾 ✓

---

## 📥 第六步：下载编译好的 EXE

### 6.1 找到 Release

1. 在仓库页面，点击 **"Releases"** （右侧边栏）
2. 或者访问：`https://github.com/你的用户名/meshvpn/releases`

### 6.2 下载 EXE

你会看到类似这样的文件：

```
✅ meshvpn-windows-amd64.zip     ← 这个是你要的！Windows EXE
  meshvpn-ubuntu-latest-amd64.tar.gz  ← Linux 版本
  meshvpn-macos-latest-amd64.tar.gz    ← Mac 版本
```

1. 点击 **meshvpn-windows-amd64.zip** 旁边的 **"Download"**
2. 解压下载的 zip 文件
3. **双击 `meshvpn.exe` 运行！**

---

## 🎉 完成！

恭喜！你已经成功获得了 Windows EXE！

```
meshvpn.exe
```

---

## 💡 快速参考

| 步骤 | 操作 | 需要时间 |
|------|------|----------|
| 1 | 创建 GitHub 账号 | 2分钟 |
| 2 | 下载解压源代码 | 1分钟 |
| 3 | 创建 GitHub 仓库 | 1分钟 |
| 4 | 上传代码 | 2分钟 |
| 5 | 触发构建 | 10分钟 |
| 6 | 下载 EXE | 1分钟 |

**总计：约 17 分钟！**

---

## ⚠️ 常见问题

**Q: 没看到 Actions 标签？**
> A: 确保仓库已经创建完成，刷新页面

**Q: 构建失败了？**
> A: 点击失败的构建任务，查看错误信息，告诉我具体错误

**Q: 没看到 Releases？**
> A: 用方法 1（手动触发）会创建 Release，方法 2 可能需要打标签

**Q: 下载的是 zip 但打不开？**
> A: Windows 10/11 可以直接解压，或者用 7-Zip, WinRAR 等工具

---

## 🎊 恭喜！

现在你可以：
- 运行 `meshvpn.exe --help`
- 生成配置：`meshvpn.exe generate`
- 查看公钥：`meshvpn.exe pubkey`

有任何问题随时问我！
