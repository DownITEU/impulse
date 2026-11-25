## Frame

panel-brand = IMPULSE · 遠端面板
panel-no-game = 未開啟任何遊戲
dock-no-run = 尚未遊玩
drawer-close = 關閉
search-clear = 清除搜尋
link-open = 已連線
link-connecting = 連線中
link-lost = 已斷線

## Connection

connection-title = 連線狀態
connection-close = 關閉連線面板
connection-pairing = 若要配對其他裝置，請點選 Impulse 標題列中的手機按鈕，並以該裝置掃描 QR Code。
connection-follows = 語言與強調色會跟隨 Impulse 設定。

## Placeholders

placeholder-refused = 配對失敗
placeholder-refused-body = 配對碼已使用、已過期或嘗試次數過多，請重新掃描 Impulse 上的 QR Code。
placeholder-outdated = 需要重新載入
placeholder-outdated-body = Impulse 已更新：此頁面為舊版。
placeholder-reload = 重新載入
placeholder-offline = 未連線
placeholder-offline-body = 請在電腦上開啟 Impulse，點選標題列中的手機按鈕，開啟「手機連線」並掃描 QR Code。此裝置必須位於同一個網路。
placeholder-connecting = 正在連線至 Impulse
placeholder-pick = 選擇遊戲
placeholder-pick-body = 目前沒有開啟任何遊戲，請開啟收藏庫並選擇一款遊戲。
placeholder-browse = 開啟收藏庫

## Library

library-title = 收藏庫
library-close = 關閉收藏庫
library-count = { $count ->
    *[other] { $count } 款遊戲
}
library-search = 搜尋遊戲
library-playing = 正在遊玩
library-favorites = 最愛
library-all = 所有遊戲
library-open = 開啟
library-no-match = 找不到符合「{ $query }」的遊戲
library-empty = 收藏庫目前是空的。

## Game

game-start = 啟動遊戲與修改器
game-stop = 停止修改器
game-favorite = 加入最愛
game-unfavorite = 移出最愛
run-idle = 未執行
run-starting = 啟動中
run-live = 修改器已開啟
run-stopping = 停止中
run-failed = 無法啟動

## Trainer

sheet-none = 尚無修改器
sheet-none-body = 這款遊戲目前還沒有修改器。一旦推出，Impulse 會在收藏庫中標示出來。
sheet-pending = 正在載入修改項目清單
sheet-failed = 修改項目清單尚未下載
sheet-search = 搜尋修改項目
sheet-start-first = 請先啟動遊戲再使用修改項目。
sheet-no-match = 找不到符合「{ $query }」的修改項目
sheet-matches = { $count ->
    *[other] { $count } 筆相符項目
}
sheet-end = 結束 · { $count ->
    *[other] { $count } 個修改項目
}
section-mods = { $count ->
    *[other] { $count } 個修改項目
}
section-mods-on = { $count ->
    *[other] { $count } 個修改項目
} · { $on }/{ $toggles } 已開啟
cheat-pin = 釘選 { $name }
cheat-unpin = 取消釘選 { $name }

## Collections

actions-reset = 重設
actions-collect = 組合
actions-forget = 刪除組合 { $name }
collect-title = 新增組合
collect-tip = 將目前開啟的修改項目儲存為一個組合。
collect-name = 名稱
collect-cancel = 取消
collect-save = 儲存
collect-close = 關閉

## Controls

control-press = 啟用
control-apply = 套用
control-value = { $name }：數值
control-decrease = 減少 { $name }
control-increase = 增加 { $name }
control-no-options = 無選項
