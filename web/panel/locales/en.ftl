## Frame

panel-brand = IMPULSE · REMOTE
panel-no-game = No game open
dock-no-run = Not playing
drawer-close = Close
search-clear = Clear the search
link-open = ONLINE
link-connecting = LINKING
link-lost = OFFLINE

## Connection

connection-title = Connection
connection-close = Close the connection panel
connection-pairing = To pair another device, open Settings > REMOTE in Impulse and scan the QR code with it.
connection-follows = The language and the accent follow Impulse.

## Placeholders

placeholder-refused = Pairing failed
placeholder-refused-body = The code was used, expired or tried too often. Scan the QR code in Impulse again.
placeholder-outdated = Reload needed
placeholder-outdated-body = Impulse was updated: this page is older than it.
placeholder-reload = Reload
placeholder-offline = Not connected
placeholder-offline-body = Open Impulse on your PC, turn the remote panel on in Settings > REMOTE and scan its QR code. This device must be on the same network.
placeholder-connecting = Connecting to Impulse
placeholder-pick = Pick a game
placeholder-pick-body = No game is open. Open the library and choose one.
placeholder-browse = Open the library

## Library

library-title = Library
library-close = Close the library
library-count = { $count ->
    [one] { $count } game
   *[other] { $count } games
}
library-search = Search games
library-playing = Now playing
library-favorites = Favorites
library-all = All games
library-open = OPEN
library-no-match = No game matches “{ $query }”
library-empty = The library is empty.

## Game

game-start = Start the game and the trainer
game-stop = Stop the trainer
game-favorite = Add to favorites
game-unfavorite = Remove from favorites
run-idle = Not running
run-starting = Starting
run-live = Trainer on
run-stopping = Stopping
run-failed = Did not start

## Trainer

sheet-none = No trainer yet
sheet-none-body = There is no trainer for this game yet. Impulse flags it in the library as soon as one ships.
sheet-pending = Loading the cheat list
sheet-failed = The cheat list is not downloaded
sheet-search = Search mods
sheet-start-first = Start the game to use the mods.
sheet-no-match = No mod matches “{ $query }”
sheet-matches = { $count ->
    [one] { $count } match
   *[other] { $count } matches
}
sheet-end = END · { $count ->
    [one] { $count } MOD
   *[other] { $count } MODS
}
section-mods = { $count ->
    [one] { $count } mod
   *[other] { $count } mods
}
section-mods-on = { $count ->
    [one] { $count } mod
   *[other] { $count } mods
} · { $on }/{ $toggles } on
cheat-pin = Pin { $name }
cheat-unpin = Unpin { $name }

## Collections

actions-reset = RESET
actions-collect = Collection
actions-forget = Delete the collection { $name }
collect-title = New collection
collect-tip = Keeps the mods that are on now as a collection.
collect-name = Name
collect-cancel = Cancel
collect-save = Save
collect-close = Close

## Controls

control-press = Activate
control-apply = Apply
control-value = { $name }: value
control-decrease = Decrease { $name }
control-increase = Increase { $name }
control-no-options = No options
