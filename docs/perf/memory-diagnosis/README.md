# WGPU 内存根因诊断基线（GUI-100）

目标：为 `envr-gui` 的 `wgpu` 相关内存增长建立**可复现测量基线**，并给出“增长发生在哪个场景阶段 + 最可能原因（按优先级）”的归因依据。

本目录产出：

- `results/<date>-baseline.md`：同一天的一次 baseline 结果（含结论模板）
- （可选）脚本会额外生成 `.csv/.json` 用于复核计算过程

## 你需要准备

1. 确保 GUI 在“同一窗口大小/同一数据规模/同一流程顺序”下操作
2. 尽量使用与 `refactor docs/03-gui-设计.md` 相同的预期：冷/热启动、导航来回、长列表滚动、下载面板展开/折叠
3. 记下你最终对照的 `GPU` 口径（Task Manager 里看 `Dedicated/Shared`）

## 运行测量脚本（推荐）

1. 先在 PowerShell 窗口准备好（不要最小化，否则有些机器上窗口渲染/采样会不稳定）
2. 运行：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/measure-gui-memory-baseline.ps1 -GuiExe "target\release\envr-gui.exe" -Repeat 1
```

默认开启“进程树口径”（父进程 + 子进程 Private Bytes），用于更公平比较 WebView/多进程架构。  
若你只想看单进程，可显式关闭：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/measure-gui-memory-baseline.ps1 -GuiExe "target\release\envr-gui.exe" -Repeat 1 -IncludeChildProcesses:$false
```

脚本默认是“计时模式”：你不需要在脚本窗口按 `Enter`。你只要在每个阶段的计时窗口内完成对应操作（这样场景边界足够一致）。

## 场景操作顺序（必须保持顺序）

- S1 冷启动：启动 -> 停在仪表盘空态/或运行时页空列表（不滚动）
- S2 导航切换：左侧导航来回切 10 次（仪表盘 <-> 运行时 <-> 设置）
- S3 长列表：切到某运行时版本列表（大量版本）-> 向下滚动到尽头 -> 再向上滚动到顶部
- S4 下载面板：点击某语言安装触发下载队列 -> 下载面板展示 -> 展开/折叠若干次 -> 结束下载或取消
- S5 资源释放观察：退出页面（切回仪表盘）后等待 30 秒，观察内存是否回落

## GPU 指标采集（手工补全）

脚本自动采集的是进程 `Private Bytes`（Windows 口径）。

`Dedicated/Shared` 建议你在同一轮 baseline 中手工记录（Task Manager）：

1. 任务管理器 -> 进程 -> `envr-gui.exe`（或标题为 Envr 的窗口对应进程）
2. 性能 -> GPU -> 记录 Dedicated/Shared（在每个场景开始/结束时各记一次）

脚本生成的 `results/<date>-baseline.md` 会预留字段给你粘贴。

## 归因规则（脚本会按数据模式给“可能原因排序”）

根据 `tasks_gui.md`（GUI-100 验收/归因方法模板）：

- 如果增长在 S1 就很高：优先怀疑字体/初始化资源/纹理图集
- 如果增长在 S2：优先怀疑页面切换导致反复构建 widget 树/资源缓存未复用
- 如果增长在 S3：优先怀疑列表虚拟化/字体渲染/纹理 atlas
- 如果增长在 S4：优先怀疑下载面板动态渲染、图标/进度组件、纹理创建
- 如果 S5 不回落：优先怀疑资源释放/缓存生命周期问题

注：最终“至少得到一次反证/证据”的要求仍需要你运行脚本拿到真实数据后确认；脚本只做数据模式推断与证据字段模板化。

