# default_characters — 预装角色

这个目录下的每个子目录对应一个"预装"角色，会被 MSI 安装包打包到用户机器上的 `install_dir/default_characters/`。

## 添加一个新角色

1. 在本目录下建一个子目录（用英文/拼音/数字，不要带空格和中文），例如 `cat/`。
2. 在子目录里放两张 PNG：
   - `neutral.png` — 中性表情
   - `smile.png` — 微笑表情
3. 写一个 `manifest.json`：
   ```json
   {
     "id": "cat",
     "name_zh": "猫",
     "name_en": "Cat",
     "has_neutral": true,
     "has_smile": true
   }
   ```
   - `id` 必须和目录名一致
   - `name_zh` / `name_en` 是显示名，按用户语言选；都空就回退到 `id`
   - `has_neutral` / `has_smile` 默认 true，没图就设 false

## 约束

- PNG 尺寸和原图一致最好（不一致也能跑，游戏会拉伸）
- 用户可以"复制为我的角色"再编辑预装角色，但**预装本身只读**
- 单个角色的两张图合计建议 < 1 MB

## 当前预装的角色

- `steve/` — Steve（Minecraft 风格方块人，138×276 / 138×275）
- `robot/` — 游戏当前默认的机器人角色（与游戏本体 `asset/dialogue/mentor_*.png` 一致，1828×2947）

新增/替换一个角色后，需要重新打包 MSI 才能让终端用户拿到。
