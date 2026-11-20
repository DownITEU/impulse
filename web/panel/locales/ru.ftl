## Frame

panel-brand = IMPULSE · ПУЛЬТ
panel-no-game = Игра не открыта
dock-no-run = Не в игре
drawer-close = Закрыть
search-clear = Очистить поиск
link-open = В СЕТИ
link-connecting = ПОДКЛЮЧЕНИЕ
link-lost = НЕ В СЕТИ

## Connection

connection-title = Подключение
connection-close = Закрыть панель подключения
connection-pairing = Чтобы подключить другое устройство, нажмите значок телефона вверху окна Impulse. Отсканируйте QR-код устройством, которое хотите подключить.
connection-follows = Язык и цвет акцента — как в Impulse.

## Placeholders

placeholder-refused = Не удалось подключить устройство
placeholder-refused-body = Код уже использовали, срок его действия истёк или было слишком много попыток. Отсканируйте новый QR-код в Impulse.
placeholder-outdated = Обновите страницу
placeholder-outdated-body = Impulse обновился, а эта страница устарела.
placeholder-reload = Обновить
placeholder-offline = Нет подключения
placeholder-offline-body = Откройте Impulse на ПК и нажмите значок телефона вверху окна. Включите «Доступ с телефона» и отсканируйте QR-код. Подключитесь к той же сети, что и ПК.
placeholder-connecting = Подключение к Impulse
placeholder-pick = Выберите игру
placeholder-pick-body = Откройте библиотеку и выберите игру.
placeholder-browse = Открыть библиотеку

## Library

library-title = Библиотека
library-close = Закрыть библиотеку
library-count = { $count ->
    [one] { $count } игра
    [few] { $count } игры
    [many] { $count } игр
   *[other] { $count } игры
}
library-search = Поиск игр
library-playing = Сейчас в игре
library-favorites = Избранное
library-all = Все игры
library-open = ОТКРЫТА
library-no-match = Нет игр по запросу «{ $query }»
library-empty = Библиотека пуста.

## Game

game-start = Запустить игру и трейнер
game-stop = Остановить трейнер
game-favorite = Добавить в избранное
game-unfavorite = Убрать из избранного
run-idle = Трейнер выключен
run-starting = Запуск
run-live = Трейнер работает
run-stopping = Остановка
run-failed = Не удалось запустить

## Trainer

sheet-none = Трейнера пока нет
sheet-none-body = Для этой игры пока нет трейнера. Когда он появится, Impulse отметит игру в библиотеке.
sheet-pending = Загрузка списка читов
sheet-failed = Не удалось загрузить список читов
sheet-search = Поиск модов
sheet-start-first = Моды станут доступны после запуска игры.
sheet-no-match = Нет модов по запросу «{ $query }»
sheet-matches = { $count ->
    [one] { $count } совпадение
    [few] { $count } совпадения
    [many] { $count } совпадений
   *[other] { $count } совпадения
}
sheet-end = ВСЕГО · { $count ->
    [one] { $count } МОД
    [few] { $count } МОДА
    [many] { $count } МОДОВ
   *[other] { $count } МОДА
}
section-mods = { $count ->
    [one] { $count } мод
    [few] { $count } мода
    [many] { $count } модов
   *[other] { $count } мода
}
section-mods-on = { $count ->
    [one] { $count } мод
    [few] { $count } мода
    [many] { $count } модов
   *[other] { $count } мода
} · { $on }/{ $toggles } вкл.
cheat-pin = Закрепить { $name }
cheat-unpin = Открепить { $name }

## Collections

actions-reset = СБРОС
actions-collect = Коллекция
actions-forget = Удалить коллекцию { $name }
collect-title = Новая коллекция
collect-tip = В коллекцию попадут все включённые моды.
collect-name = Название
collect-cancel = Отмена
collect-save = Сохранить
collect-close = Закрыть

## Controls

control-press = Применить
control-apply = Применить
control-value = { $name }: значение
control-decrease = Уменьшить { $name }
control-increase = Увеличить { $name }
control-no-options = Нет вариантов
