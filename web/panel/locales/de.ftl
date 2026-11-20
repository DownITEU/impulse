## Frame

panel-brand = IMPULSE · REMOTE
panel-no-game = Kein Spiel geöffnet
dock-no-run = Keine Sitzung
drawer-close = Schließen
search-clear = Suchfeld leeren
link-open = ONLINE
link-connecting = VERBINDE …
link-lost = OFFLINE

## Connection

connection-title = Verbindung
connection-close = Verbindungsinfo schließen
connection-pairing = Zum Koppeln in der Titelleiste von Impulse auf das Handy-Symbol klicken und den QR-Code mit dem neuen Gerät scannen.
connection-follows = Das Remote-Panel übernimmt Sprache und Akzentfarbe von Impulse.

## Placeholders

placeholder-refused = Kopplung fehlgeschlagen
placeholder-refused-body = Der Code wurde schon benutzt, ist abgelaufen oder es gab zu viele Versuche. Den aktuellen QR-Code in Impulse scannen.
placeholder-outdated = Seite neu laden
placeholder-outdated-body = Impulse wurde aktualisiert. Das Remote-Panel muss neu geladen werden.
placeholder-reload = Neu laden
placeholder-offline = Nicht verbunden
placeholder-offline-body = Impulse auf dem PC öffnen und in der Titelleiste auf das Handy-Symbol klicken. Den Handyzugriff einschalten und den QR-Code scannen. Beide Geräte müssen im selben Netzwerk sein.
placeholder-connecting = Verbindung zu Impulse wird hergestellt
placeholder-pick = Spiel auswählen
placeholder-pick-body = Kein Spiel geöffnet. Ein Spiel aus der Bibliothek auswählen.
placeholder-browse = Bibliothek öffnen

## Library

library-title = Bibliothek
library-close = Bibliothek schließen
library-count = { $count ->
    [one] { $count } Spiel
   *[other] { $count } Spiele
}
library-search = Spiele suchen
library-playing = Wird gespielt
library-favorites = Favoriten
library-all = Alle Spiele
library-open = OFFEN
library-no-match = Keine Treffer für „{ $query }“
library-empty = Die Bibliothek ist leer.

## Game

game-start = Spiel und Trainer starten
game-stop = Trainer stoppen
game-favorite = Zu den Favoriten hinzufügen
game-unfavorite = Aus den Favoriten entfernen
run-idle = Trainer inaktiv
run-starting = Startet
run-live = Trainer aktiv
run-stopping = Stoppt
run-failed = Start fehlgeschlagen

## Trainer

sheet-none = Noch kein Trainer
sheet-none-body = Für dieses Spiel gibt es noch keinen Trainer. Sobald einer verfügbar ist, markiert Impulse das Spiel in der Bibliothek.
sheet-pending = Cheatliste wird geladen
sheet-failed = Cheatliste nicht geladen
sheet-search = Mods suchen
sheet-start-first = Das Spiel starten, um die Mods zu nutzen.
sheet-no-match = Keine Treffer für „{ $query }“
sheet-matches = { $count ->
    [one] { $count } Treffer
   *[other] { $count } Treffer
}
sheet-end = ENDE · { $count ->
    [one] { $count } MOD
   *[other] { $count } MODS
}
section-mods = { $count ->
    [one] { $count } Mod
   *[other] { $count } Mods
}
section-mods-on = { $count ->
    [one] { $count } Mod
   *[other] { $count } Mods
} · { $on }/{ $toggles } aktiv
cheat-pin = { $name } anheften
cheat-unpin = { $name } lösen

## Collections

actions-reset = RESET
actions-collect = Sammlung
actions-forget = Sammlung { $name } löschen
collect-title = Neue Sammlung
collect-tip = Speichert die aktuell aktiven Mods als Sammlung.
collect-name = Name
collect-cancel = Abbrechen
collect-save = Speichern
collect-close = Schließen

## Controls

control-press = Ausführen
control-apply = Übernehmen
control-value = { $name }: Wert
control-decrease = { $name } verringern
control-increase = { $name } erhöhen
control-no-options = Keine Optionen
