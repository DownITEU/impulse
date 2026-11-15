## Shared

off = OFF
on = ON

## Tray: Windows menu words

tray-open = Open Impulse
tray-stop = Stop trainer
tray-overlay = Overlay
tray-quit = Quit

## Failures: a kind's words fit after "It could not be loaded:"

failure-offline = no connection
failure-signed-out = signed out
failure-not-found = not found
failure-unsupported = not supported yet
failure-setup-needed = a setting is missing: see Settings, APP
failure-game-not-found = the game was not found
failure-refused = refused
failure-internal = something went wrong: the log has the details
failure-place = { $why }: see Settings, APP

## Page

catalog-failed-tag = [ NO CATALOG ]
catalog-failed = THE GAME CATALOG IS UNAVAILABLE
catalog-failed-body = It could not be downloaded: { $why }. Impulse tries again on the next start.
library-filter = filter
picker-filter = game name
page-search = search { $trainers } trainers
page-search-none = search trainers
page-search-key = CTRL K
page-signed-out = NOT SIGNED IN
page-overlay = OVERLAY
page-star = Star
page-folder = Open the game's folder
page-launcher = Open in { $launcher }
page-source = Game source
page-custom = CUSTOM
page-live = LIVE
page-active = { $active }/{ $total } ACTIVE
page-idle = IDLE
page-log = LOG
page-binding = PRESS KEYS · ESC CANCEL · BKSP DEFAULT
page-binding-unbind = PRESS KEYS · ESC CANCEL · BKSP DEFAULT · DEL UNBIND
page-hotkeys-on = HOTKEYS ON
page-hotkeys-off = HOTKEYS OFF

## Library

library-favorites = FAVORITES
library-games = GAMES
library-no-match = NO GAMES MATCH
library-add = + ADD
library-live = LIVE
library-scanning-tag = [ SCANNING ]
library-scanning = LOOKING FOR INSTALLED GAMES
library-scanning-body = Reading the game catalog and your launchers.
library-empty-tag = [ NO GAMES ]
library-empty = NO INSTALLED GAMES FOUND
library-empty-body = Impulse looks in Steam, Epic, GOG, Ubisoft, Xbox, Rockstar and EA for games it has trainers for.
library-source-exe = EXE  { $file }
library-source-add = + ADD .EXE
library-source-change = CHANGE .EXE
library-source-remove = REMOVE .EXE

## Picker

picker-title = ADD A GAME
picker-owned = MORE THAN ONE GAME MATCHES: WHICH ONE IS IT?
picker-unowned = NO CATALOG GAME MATCHES: FIND IT BY NAME
picker-in-library = { $store }  IN LIBRARY
picker-prompt = Select the game's .exe

## Cheats

cheats-failed-tag = [ NO DATA ]
cheats-failed = THE CHEAT LIST IS NOT DOWNLOADED
cheats-failed-body = It could not be downloaded: { $why }. Impulse tries again when you open the game.
cheats-filter = filter mods
cheats-pinned = PINNED
cheats-kept = It is kept on disk after the first download.
cheats-no-trainer-tag = [ NO TRAINER ]
cheats-no-trainer = NO TRAINER FOR { $game } YET
cheats-no-trainer-body = There is no trainer for this game yet. Impulse will flag it in your library as soon as one ships.
cheats-loading-tag = [ LOADING ]
cheats-loading = LOADING THE CHEAT LIST
cheats-shown = { $shown } MODS
cheats-no-match = NO MODS MATCH THE FILTER
cheats-updated = GAME UPDATED
cheats-updated-body = This build is newer than the trainer supports. Mods may not work until the trainer is updated.
cheats-set = SET
cheats-set-done = OK
cheats-execute = EXECUTE
cheats-execute-done = APPLIED
cheats-save = SAVE
cheats-saved = SAVED { $saved }
cheats-save-tip = The mods a run leaves on come back on at the next one.
cheats-reset = RESET
cheats-no-collections = NO COLLECTIONS
cheats-collect-tip = Keeps the mods that are on now as a collection.
cheats-edit = EDIT
cheats-delete = DELETE
cheats-name-field = name
cheats-cancel = CANCEL
cheats-name = NAME
cheats-icon = ICON
cheats-collection-mods = { $mods } MODS
cheats-edit-collection = EDIT COLLECTION
cheats-new-collection = NEW COLLECTION

## Hotkeys

hotkeys-press-keys = PRESS KEYS
hotkeys-off = OFF
hotkeys-in-game = IN GAME
hotkeys-global = GLOBAL
hotkeys-enabled = HOTKEYS
hotkeys-enabled-hint = trainer hotkeys during a run
hotkeys-works-in = WORKS IN
hotkeys-works-in-hint = in game: only while the game is in front

## Trainer

trainer-none = NO TRAINER
trainer-mods = { $count } MODS
trainer-version = V{ $version }
trainer-version-unknown = VERSION UNKNOWN
trainer-update-pending = UPDATE PENDING
trainer-sessions = { $count ->
    [one] { $count } SESSION
   *[other] { $count } SESSIONS
}
trainer-played-min = { $count } MIN PLAYED
trainer-played-hours = { $hours } H PLAYED

## Run

run-no-cheats = the cheat list is not loaded
run-preparing = PREPARING THE TRAINER
run-launching = STARTING THE GAME
run-waiting = WAITING FOR THE GAME
run-attaching = LOADING THE TRAINER
run-stopping = STOPPING THE TRAINER
run-stop = STOP
run-launch = LAUNCH + ATTACH
run-enter = ENTER
run-session = SESSION
run-last-session = LAST SESSION
run-active = ACTIVE
run-hotkeys = HOTKEYS
run-idle = TRAINER IDLE · CONTROLS UNLOCK AFTER LAUNCH

## Map

teleport-failed = TELEPORT FAILED: { $why }
map-failed-tag = [ NO MAP ]
map-failed = THE MAP IS UNAVAILABLE
map-failed-body = It could not be loaded: { $why }.
map-title = MAP
map-none = NO INTERACTIVE MAP FOR THIS GAME
map-loading-tag = [ LOADING ]
map-loading = LOADING THE MAP
map-loading-body = Pins and tiles are kept on disk after the first time.
map-search = find a place, a group, a word
map-unfinished = Still being mapped: pins may be missing or move.
map-found = FOUND
map-found-count = FOUND
map-mark-found = MARK FOUND
map-unfound = UNFOUND
map-show-all = SHOW ALL
map-hide-all = HIDE ALL
map-teleport = TELEPORT
map-filter-teleport = TELEPORT
map-teleport-here = TELEPORT HERE
map-confirm = CONFIRM
map-delete = DELETE
map-add-pin = ADD MY PIN
map-my-pin = MY PIN
map-my-pins = MY PINS
map-pin-name = My pin { $number }
map-name-field = name this pin
map-note-field = a note
map-no-pins = Right click the map to add one.
map-regions = REGIONS
map-layer-regions = REGIONS
map-places = { $places } PLACES
map-no-matches = Nothing matches. Try a group or a shorter word.
map-nearby = NEARBY
map-copy = COPY
map-copied = COPIED
map-hide-group = HIDE { $group } ON THE MAP
map-video = VIDEO
map-photo = PHOTO
map-layer-labels = PLACE NAMES
map-layer-clusters = GROUP CLOSE PINS
map-axis-x = X
map-axis-y = Y
map-axis-z = Z

## Overlay

overlay-dock-mods = MODS
overlay-dock-map = MAP
overlay-dock-session = SESSION
overlay-dock-minimap = MINIMAP
overlay-minimap-tag = [ MINIMAP ]
overlay-no-live-map = NO LIVE MAP
overlay-no-live-map-hint = It follows the player while a trainer that can place them runs in a game with a map.
overlay-mode = MODE
overlay-mode-hint = in-game, drawn inside the game: later
overlay-mode-over = OVER THE GAME
overlay-mode-in-game = IN-GAME
overlay-hotkey = HOTKEY
overlay-hotkey-hint = modifiers and one key, free in other apps and mods
overlay-minimap-hotkey = MINIMAP HOTKEY
overlay-minimap-hotkey-hint = shows or hides the minimap; held only while the game is in front

## Settings

settings = SETTINGS
settings-appearance = APPEARANCE
settings-overlay = OVERLAY
settings-hotkeys = HOTKEYS
settings-app = APP
settings-resources = RESOURCES
settings-about = ABOUT
accent = ACCENT
accent-hint = marks live state only
accent-lime = LIME
accent-amber = AMBER
accent-cyan = CYAN
accent-white = WHITE
sidebar = SIDEBAR
sidebar-hint = leading item of a game row
sidebar-number = NUMBER
sidebar-thumbnail = THUMBNAIL
panels = PANELS
panels-hint = map and session over the mods, or beside them
panels-top = TOP
panels-side = SIDE
reduce-motion = REDUCE MOTION
reduce-motion-hint = no blinking LIVE, no transitions
language = LANGUAGE
language-hint = the language of Impulse's own texts
language-system = SYSTEM
close-to-tray = CLOSE TO TRAY
close-to-tray-hint = the close button hides Impulse; quit from the tray icon
hide-from-capture = HIDE FROM CAPTURE
hide-from-capture-hint = screenshots, recordings and streams show no Impulse window
folder-path = path to the folder
folder-select = Select the folder

## Resources

media-pictures = PICTURES
media-pictures-hint = game art and pin pictures
media-tiles = MAP TILES
media-tiles-hint = the ground of the maps you opened
media-videos = VIDEOS
media-videos-hint = pin clips
resources-reading = READING SIZES...
resources-in-use = IN USE
resources-in-use-hint = everything Impulse keeps on this PC
resources-clear = CLEAR
resources-on-restart = ON RESTART
resources-clears-on-restart = CLEARS ON RESTART
resources-clear-caches = CLEAR ALL CACHES
resources-clear-caches-hint = your data below stays
resources-clear-all = CLEAR ALL
resources-limit = MEDIA LIMIT
resources-limit-hint = past it, the media unused longest goes
resources-limit-500m = 500 MB
resources-limit-1g = 1 GB
resources-limit-2g = 2 GB
resources-limit-5g = 5 GB
resources-limit-none = NONE
resources-kept = KEPT
resources-progress = MAP PROGRESS
resources-progress-hint = found pins and your own pins
resources-hotkeys = HOTKEYS
resources-hotkeys-hint = the keys you bound
resources-logs = LOGS
resources-logs-hint = two files at most, read by LOG
resources-bytes = { $size } B
resources-kilobytes = { $size } KB
resources-megabytes = { $size } MB
resources-gigabytes = { $size } GB

## Log

log-title = LOG
log-error = ERROR
log-warn = WARN
log-info = INFO
log-debug = DEBUG
log-filter = filter log
log-follow = FOLLOW
log-copy-all = COPY ALL
log-copy = COPY { $count }
log-copied = COPIED
log-clear = CLEAR
log-folder = FOLDER
log-detail-empty = CLICK A ROW TO READ AND SELECT THE FULL RECORD
