## Frame

panel-brand = IMPULSE · ПУЛЬТ
panel-no-game = Гру не відкрито
dock-no-run = Не в грі
drawer-close = Закрити
search-clear = Очистити пошук
link-open = ОНЛАЙН
link-connecting = ПІДКЛЮЧЕННЯ
link-lost = ОФЛАЙН

## Connection

connection-title = Підключення
connection-close = Закрити панель підключення
connection-pairing = Щоб прив'язати ще один пристрій, натисніть значок телефона вгорі вікна Impulse на ПК. Відскануйте QR-код пристроєм, який хочете підключити.
connection-follows = Мова й акцентний колір тут такі самі, як в Impulse.

## Placeholders

placeholder-refused = Не вдалося прив'язати пристрій
placeholder-refused-body = Код уже використано, він більше не діє або було забагато спроб. Ще раз відскануйте QR-код в Impulse.
placeholder-outdated = Оновіть сторінку
placeholder-outdated-body = Impulse оновився. Ця сторінка ще зі старої версії.
placeholder-reload = Оновити
placeholder-offline = Немає підключення
placeholder-offline-body = Відкрийте Impulse на ПК і натисніть значок телефона вгорі вікна. Увімкніть доступ з телефона й відскануйте QR-код. Цей пристрій і ПК мають бути в одній мережі.
placeholder-connecting = Підключаємося до Impulse
placeholder-pick = Виберіть гру
placeholder-pick-body = Гру не відкрито. Виберіть гру в бібліотеці.
placeholder-browse = Відкрити бібліотеку

## Library

library-title = Бібліотека
library-close = Закрити бібліотеку
library-count = { $count ->
    [one] { $count } гра
    [few] { $count } гри
    [many] { $count } ігор
   *[other] { $count } гри
}
library-search = Пошук ігор
library-playing = У грі
library-favorites = Обране
library-all = Усі ігри
library-open = ВІДКРИТА
library-no-match = Немає ігор за запитом «{ $query }»
library-empty = Бібліотека порожня.

## Game

game-start = Запустити гру й трейнер
game-stop = Зупинити трейнер
game-favorite = Додати до обраного
game-unfavorite = Прибрати з обраного
run-idle = Не запущено
run-starting = Запускаємо
run-live = Трейнер активний
run-stopping = Зупиняємо
run-failed = Не вдалося запустити

## Trainer

sheet-none = Трейнера ще немає
sheet-none-body = Для цієї гри ще немає трейнера. Щойно він з'явиться, Impulse позначить гру в бібліотеці.
sheet-pending = Завантажуємо список читів
sheet-failed = Список читів не завантажено
sheet-search = Пошук модів
sheet-start-first = Запустіть гру, щоб користуватися модами.
sheet-no-match = Немає модів за запитом «{ $query }»
sheet-matches = { $count ->
    [one] { $count } збіг
    [few] { $count } збіги
    [many] { $count } збігів
   *[other] { $count } збігу
}
sheet-end = КІНЕЦЬ · { $count ->
    [one] { $count } МОД
    [few] { $count } МОДИ
    [many] { $count } МОДІВ
   *[other] { $count } МОДА
}
section-mods = { $count ->
    [one] { $count } мод
    [few] { $count } моди
    [many] { $count } модів
   *[other] { $count } мода
}
section-mods-on = { $count ->
    [one] { $count } мод
    [few] { $count } моди
    [many] { $count } модів
   *[other] { $count } мода
} · { $on }/{ $toggles } активно
cheat-pin = Закріпити «{ $name }»
cheat-unpin = Відкріпити «{ $name }»

## Collections

actions-reset = СКИНУТИ
actions-collect = Колекція
actions-forget = Видалити колекцію «{ $name }»
collect-title = Нова колекція
collect-tip = Збережіть у колекцію моди, які зараз увімкнено.
collect-name = Назва
collect-cancel = Скасувати
collect-save = Зберегти
collect-close = Закрити

## Controls

control-press = Активувати
control-apply = Застосувати
control-value = { $name }: значення
control-decrease = Зменшити значення «{ $name }»
control-increase = Збільшити значення «{ $name }»
control-no-options = Немає варіантів
