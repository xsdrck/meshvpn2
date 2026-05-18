# 🚀 GitHub Actions 自动构建指南

## 这是最简单的方案！让 GitHub 免费帮你编译好所有平台版本！

---

## 📋 使用步骤（超简单！）

### 第 1 步：创建 GitHub 仓库

1. 访问 https://github.com/new 创建一个新仓库
2. 填写仓库名，例如 `meshvpn`
3. 选择 Public（公共仓库）或 Private（私有仓库）
4. **跳过** 添加 README 等选项
5. 点击 "Create repository"

### 第 2 步：上传代码

在你的电脑上：

```bash
# 进入我们的项目目录
cd meshvpn-v0.2.0-src

# 初始化 Git
git init
git add .
git commit -m "Initial commit"

# 关联你的 GitHub 仓库
git remote add origin https://github.com/你的用户名/你的仓库名.git

# 推送到 GitHub
git branch -M main
git push -u origin main
```

### 第 3 步：触发自动构建！

**方式 A：打标签触发（推荐）**
```bash
git tag v0.2.0
git push origin v0.2.0
```

**方式 B：手动触发（更简单！）**
1. 在 GitHub 仓库页面，点击上方 "Actions" 标签
2. 点击左侧 "Build and Release"
3. 点击右侧的 "Run workflow"
4. 点击绿色 "Run workflow" 按钮

### 第 4 步：下载编译好的文件！

几分钟后（一般 5-10 分钟）：
1. 点击仓库的 "Releases" 页面
2. 找到最新的 Release
3. 下载你需要的：
   - `meshvpn-windows-amd64.zip` - Windows EXE ⭐
   - `meshvpn-ubuntu-latest-amd64.tar.gz` - Linux
   - `meshvpn-macos-latest-amd64.tar.gz` - macOS

---

## 🎯 更简单的方法？直接看这个！

### 或者，你只是想要 Windows EXE？

**我们也可以简化到只构建 Windows 版本！**

我可以帮你把工作流简化成只构建 Windows，这样更快！

---

## 💡 这个方案的好处

✅ **完全免费！** GitHub 免费给你构建
✅ **不用安装任何东西！** 不用 Rust，不用 C++ 工具链
✅ **所有平台都能编译！** Windows/Linux/macOS 一起搞定
✅ **几分钟搞定！** 你只需要上传代码，喝杯咖啡，回来就能下载了！

---

## 📝 快速参考

| 步骤 | 说明 |
|------|------|
| 1 | 创建 GitHub 仓库 |
| 2 | 上传代码 |
| 3 | 在 Actions 点击 "Run workflow" |
| 4 | 去 Releases 下载！ |

就这么简单！

---

## ⚠️ 注意事项

- GitHub Actions 对公共仓库完全免费
- 私有仓库也有免费额度，足够用了
- 第一次运行需要授权 GitHub Actions
