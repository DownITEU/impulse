## Frame

panel-brand = IMPULSE · 手机端
panel-no-game = 没有打开的游戏
dock-no-run = 未在游玩
drawer-close = 关闭
search-clear = 清除搜索
link-open = 已连接
link-connecting = 连接中
link-lost = 已断开

## Connection

connection-title = 连接
connection-close = 关闭连接面板
connection-pairing = 要配对另一台设备，点击 Impulse 标题栏中的手机按钮，用它扫描二维码。
connection-follows = 语言和强调色跟随 Impulse。

## Placeholders

placeholder-refused = 配对失败
placeholder-refused-body = 验证码已被使用、已过期或尝试次数过多。请重新扫描 Impulse 中的二维码。
placeholder-outdated = 需要重新加载
placeholder-outdated-body = Impulse 已更新：这个页面比它旧。
placeholder-reload = 重新加载
placeholder-offline = 未连接
placeholder-offline-body = 在电脑上打开 Impulse，点击标题栏中的手机按钮，开启手机访问并扫描二维码。这台设备必须处于同一网络。
placeholder-connecting = 正在连接 Impulse
placeholder-pick = 选择一个游戏
placeholder-pick-body = 没有打开的游戏。打开游戏库并选择一个。
placeholder-browse = 打开游戏库

## Library

library-title = 游戏库
library-close = 关闭游戏库
library-count = { $count ->
    *[other] { $count } 个游戏
}
library-search = 搜索游戏
library-playing = 正在游玩
library-favorites = 收藏
library-all = 所有游戏
library-open = 打开
library-no-match = 没有匹配「{ $query }」的游戏
library-empty = 游戏库是空的。

## Game

game-start = 启动游戏和修改器
game-stop = 停止修改器
game-favorite = 加入收藏
game-unfavorite = 取消收藏
run-idle = 未运行
run-starting = 正在启动
run-live = 修改器已开启
run-stopping = 正在停止
run-failed = 未能启动

## Trainer

sheet-none = 还没有修改器
sheet-none-body = 这个游戏目前还没有修改器。一旦发布，Impulse 会在游戏库中标记出来。
sheet-pending = 正在加载修改项列表
sheet-failed = 修改项列表未能下载
sheet-search = 搜索修改项
sheet-start-first = 启动游戏后才能使用修改项。
sheet-no-match = 没有匹配「{ $query }」的修改项
sheet-matches = { $count ->
    *[other] { $count } 个匹配项
}
sheet-end = 结束 · { $count ->
    *[other] { $count } 个修改项
}
section-mods = { $count ->
    *[other] { $count } 个修改项
}
section-mods-on = { $count ->
    *[other] { $count } 个修改项
} · { $on }/{ $toggles } 已开
cheat-pin = 固定 { $name }
cheat-unpin = 取消固定 { $name }

## Collections

actions-reset = 重置
actions-collect = 组合
actions-forget = 删除组合 { $name }
collect-title = 新建组合
collect-tip = 把当前开启的修改项保存为一个组合。
collect-name = 名称
collect-cancel = 取消
collect-save = 保存
collect-close = 关闭

## Controls

control-press = 激活
control-apply = 应用
control-value = { $name }：值
control-decrease = 减小 { $name }
control-increase = 增大 { $name }
control-no-options = 没有选项
