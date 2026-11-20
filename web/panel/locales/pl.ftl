## Frame

panel-brand = IMPULSE · PANEL
panel-no-game = Brak otwartej gry
dock-no-run = Brak sesji
drawer-close = Zamknij
search-clear = Wyczyść wyszukiwanie
link-open = ONLINE
link-connecting = ŁĄCZENIE
link-lost = OFFLINE

## Connection

connection-title = Połączenie
connection-close = Zamknij panel połączenia
connection-pairing = Aby sparować kolejne urządzenie, kliknij ikonę telefonu na pasku tytułu Impulse i zeskanuj kod QR tym urządzeniem.
connection-follows = Panel używa tego samego języka i koloru akcentu co Impulse.

## Placeholders

placeholder-refused = Nie udało się sparować urządzenia
placeholder-refused-body = Kod został już użyty, stracił ważność albo przekroczono limit prób. Zeskanuj ponownie kod QR z Impulse.
placeholder-outdated = Odśwież stronę
placeholder-outdated-body = Impulse jest już w nowszej wersji. Odśwież stronę, by wczytać aktualny panel.
placeholder-reload = Odśwież
placeholder-offline = Brak połączenia
placeholder-offline-body = Otwórz Impulse na komputerze, kliknij ikonę telefonu na pasku tytułu, włącz dostęp z telefonu i zeskanuj kod QR. Oba urządzenia muszą być w tej samej sieci.
placeholder-connecting = Łączenie z Impulse
placeholder-pick = Wybierz grę
placeholder-pick-body = Nie otwarto żadnej gry. Grę można wybrać z biblioteki.
placeholder-browse = Otwórz bibliotekę

## Library

library-title = Biblioteka
library-close = Zamknij bibliotekę
library-count = { $count ->
    [one] { $count } gra
    [few] { $count } gry
    [many] { $count } gier
   *[other] { $count } gry
}
library-search = Szukaj gier
library-playing = W grze
library-favorites = Ulubione
library-all = Wszystkie gry
library-open = OTWARTA
library-no-match = Brak gier dla „{ $query }”
library-empty = Biblioteka jest pusta.

## Game

game-start = Uruchom grę i trainer
game-stop = Zatrzymaj trainer
game-favorite = Dodaj do ulubionych
game-unfavorite = Usuń z ulubionych
run-idle = Trainer wył.
run-starting = Uruchamianie
run-live = Trainer wł.
run-stopping = Zatrzymywanie
run-failed = Błąd startu

## Trainer

sheet-none = Brak trainera
sheet-none-body = Do tej gry nie ma jeszcze trainera. Gdy się pojawi, Impulse oznaczy grę w bibliotece.
sheet-pending = Ładowanie listy modów
sheet-failed = Nie pobrano listy modów
sheet-search = Szukaj modów
sheet-start-first = Mody będą dostępne po uruchomieniu gry.
sheet-no-match = Brak modów dla „{ $query }”
sheet-matches = { $count ->
    [one] { $count } wynik
    [few] { $count } wyniki
    [many] { $count } wyników
   *[other] { $count } wyniku
}
sheet-end = KONIEC · { $count ->
    [one] { $count } MOD
    [few] { $count } MODY
    [many] { $count } MODÓW
   *[other] { $count } MODA
}
section-mods = { $count ->
    [one] { $count } mod
    [few] { $count } mody
    [many] { $count } modów
   *[other] { $count } moda
}
section-mods-on = { $count ->
    [one] { $count } mod
    [few] { $count } mody
    [many] { $count } modów
   *[other] { $count } moda
} · { $on }/{ $toggles } wł.
cheat-pin = Przypnij { $name }
cheat-unpin = Odepnij { $name }

## Collections

actions-reset = RESET
actions-collect = Kolekcja
actions-forget = Usuń kolekcję { $name }
collect-title = Nowa kolekcja
collect-tip = Do kolekcji trafią tylko włączone mody.
collect-name = Nazwa
collect-cancel = Anuluj
collect-save = Zapisz
collect-close = Zamknij

## Controls

control-press = Wykonaj
control-apply = Ustaw
control-value = { $name }: wartość
control-decrease = Zmniejsz { $name }
control-increase = Zwiększ { $name }
control-no-options = Brak opcji
