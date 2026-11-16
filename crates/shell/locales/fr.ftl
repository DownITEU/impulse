## Shared

off = OFF
on = ON
number-group = { "\u00A0" }
number-decimal = ,

## Tray: Windows menu words

tray-open = Ouvrir Impulse
tray-stop = Arrêter le trainer
tray-overlay = Overlay
tray-quit = Quitter

## Failures: a kind's words fit after "It could not be loaded:"

failure-offline = pas de connexion
failure-signed-out = déconnecté
failure-not-found = introuvable
failure-unsupported = pas encore géré
failure-setup-needed = un réglage manque : voir Paramètres, APP
failure-game-not-found = le jeu est introuvable
failure-refused = refusé
failure-internal = un problème est survenu : détails dans le log
failure-place = { $why } : voir Paramètres, APP

## Page

catalog-failed-tag = [ SANS CATALOGUE ]
catalog-failed = LE CATALOGUE DES JEUX EST INDISPONIBLE
catalog-failed-body = Téléchargement impossible : { $why }. Impulse réessaie au prochain démarrage.
library-filter = filtrer
picker-filter = nom du jeu
page-search = { $count ->
    [one] chercher { $trainers } trainer
   *[other] chercher { $trainers } trainers
}
page-search-none = chercher un trainer
page-search-key = CTRL K
page-signed-out = NON CONNECTÉ
page-overlay = OVERLAY
page-star = Favori
page-folder = Ouvrir le dossier du jeu
page-launcher = Ouvrir dans { $launcher }
page-source = Source du jeu
page-custom = PERSO
page-live = LIVE
page-active = { $active ->
    [one] { $active }/{ $total } ACTIF
   *[other] { $active }/{ $total } ACTIFS
}
page-idle = INACTIF
page-log = LOG
page-binding = APPUYEZ · ESC ANNULER · BKSP DÉFAUT
page-binding-unbind = APPUYEZ · ESC ANNULER · BKSP DÉFAUT · DEL EFFACER
page-hotkeys-on = RACCOURCIS ON
page-hotkeys-off = RACCOURCIS OFF

## Library

library-favorites = FAVORIS
library-games = JEUX
library-no-match = AUCUN RÉSULTAT
library-add = + AJOUTER
library-live = LIVE
library-scanning-tag = [ ANALYSE ]
library-scanning = RECHERCHE DES JEUX INSTALLÉS
library-scanning-body = Lecture du catalogue des jeux et de vos lanceurs.
library-empty-tag = [ AUCUN JEU ]
library-empty = AUCUN JEU INSTALLÉ TROUVÉ
library-empty-body = Impulse cherche dans Steam, Epic, GOG, Ubisoft, Xbox, Rockstar et EA les jeux pour lesquels il a des trainers.
library-source-exe = EXE  { $file }
library-source-add = + AJOUTER .EXE
library-source-change = CHANGER .EXE
library-source-remove = RETIRER .EXE

## Picker

picker-title = AJOUTER UN JEU
picker-owned = PLUSIEURS JEUX CORRESPONDENT : LEQUEL ?
picker-unowned = AUCUNE CORRESPONDANCE : CHERCHEZ-LE PAR NOM
picker-in-library = { $store }  EN BIBLIOTHÈQUE
picker-prompt = Choisir le .exe du jeu

## Cheats

cheats-failed-tag = [ SANS DONNÉES ]
cheats-failed = LA LISTE DES CHEATS N'EST PAS TÉLÉCHARGÉE
cheats-failed-body = Téléchargement impossible : { $why }. Impulse réessaie à l'ouverture du jeu.
cheats-filter = filtrer les mods
cheats-pinned = ÉPINGLÉS
cheats-kept = Conservée sur le disque après le premier téléchargement.
cheats-no-trainer-tag = [ AUCUN TRAINER ]
cheats-no-trainer = PAS DE TRAINER POUR { $game }
cheats-no-trainer-body = Il n'existe pas encore de trainer pour ce jeu. Impulse le signalera dans votre bibliothèque dès qu'il sortira.
cheats-loading-tag = [ CHARGEMENT ]
cheats-loading = CHARGEMENT DES CHEATS
cheats-shown = { $count ->
    [one] { $shown } MOD
   *[other] { $shown } MODS
}
cheats-no-match = AUCUN MOD NE CORRESPOND
cheats-updated = JEU MIS À JOUR
cheats-updated-body = Cette version dépasse celle gérée par le trainer. Des mods risquent de ne pas fonctionner avant sa mise à jour.
cheats-set = FIXER
cheats-set-done = OK
cheats-execute = EXÉCUTER
cheats-execute-done = APPLIQUÉ
cheats-save = SAUVER
cheats-saved = { $count ->
    [one] SAUVÉ { $saved }
   *[other] SAUVÉS { $saved }
}
cheats-save-tip = Les mods laissés actifs reviennent à la session suivante.
cheats-reset = RAZ
cheats-no-collections = AUCUNE COLLECTION
cheats-collect-tip = Enregistre les mods actifs comme collection.
cheats-edit = ÉDITER
cheats-delete = SUPPRIMER
cheats-name-field = nom
cheats-cancel = ANNULER
cheats-name = NOM
cheats-icon = ICÔNE
cheats-collection-mods = { $count ->
    [one] { $mods } MOD
   *[other] { $mods } MODS
}
cheats-edit-collection = MODIFIER COLLECTION
cheats-new-collection = NOUVELLE COLLECTION

## Hotkeys

hotkeys-press-keys = APPUYEZ
hotkeys-off = OFF
hotkeys-in-game = EN JEU
hotkeys-global = GLOBAL
hotkeys-enabled = RACCOURCIS
hotkeys-enabled-hint = raccourcis du trainer pendant le jeu
hotkeys-works-in = PORTÉE
hotkeys-works-in-hint = en jeu : seulement quand le jeu est au premier plan

## Trainer

trainer-none = AUCUN TRAINER
trainer-mods = { $count ->
    [one] { $count } MOD
   *[other] { $count } MODS
}
trainer-version = V{ $version }
trainer-version-unknown = VERSION INCONNUE
trainer-update-pending = MÀJ EN ATTENTE
trainer-sessions = { $count ->
    [one] { $count } SESSION
   *[other] { $count } SESSIONS
}
trainer-played-min = { $count } MIN DE JEU
trainer-played-hours = { $hours } H DE JEU

## Run

run-no-cheats = liste des cheats non chargée
run-preparing = PRÉPARATION DU TRAINER
run-launching = LANCEMENT DU JEU
run-waiting = EN ATTENTE DU JEU
run-attaching = CHARGEMENT DU TRAINER
run-stopping = ARRÊT DU TRAINER
run-stop = STOP
run-launch = LANCER + ATTACHER
run-enter = ENTRÉE
run-session = SESSION
run-last-session = DERNIÈRE SESSION
run-active = ACTIFS
run-hotkeys = RACCOURCIS
run-idle = TRAINER INACTIF · COMMANDES DÉBLOQUÉES APRÈS LANCEMENT

## Map

teleport-failed = ÉCHEC TÉLÉPORTATION : { $why }
map-failed-tag = [ SANS CARTE ]
map-failed = LA CARTE EST INDISPONIBLE
map-failed-body = Chargement impossible : { $why }.
map-title = CARTE
map-none = PAS DE CARTE INTERACTIVE POUR CE JEU
map-loading-tag = [ CHARGEMENT ]
map-loading = CHARGEMENT DE LA CARTE
map-loading-body = Repères et tuiles restent sur disque après la première fois.
map-search = trouver un lieu, un groupe, un mot
map-unfinished = Cartographie en cours : des repères peuvent manquer ou bouger.
map-found = TROUVÉ
map-found-count = TROUVÉS
map-mark-found = MARQUER TROUVÉ
map-unfound = À TROUVER
map-show-all = TOUT VOIR
map-hide-all = TOUT MASQUER
map-teleport = TÉLÉPORTER
map-filter-teleport = TÉLÉPORT
map-teleport-here = TÉLÉPORTER ICI
map-confirm = CONFIRMER
map-delete = SUPPRIMER
map-add-pin = NOUVEAU REPÈRE
map-my-pin = MON REPÈRE
map-my-pins = MES REPÈRES
map-pin-name = Mon repère { $number }
map-name-field = nommer ce repère
map-note-field = note
map-no-pins = Clic droit sur la carte pour en ajouter.
map-regions = RÉGIONS
map-layer-regions = RÉGIONS
map-places = { $count ->
    [one] { $places } LIEU
   *[other] { $places } LIEUX
}
map-no-matches = Aucun résultat. Essayez un groupe ou un mot plus court.
map-nearby = PROCHES
map-copy = COPIER
map-copied = COPIÉ
map-hide-group = MASQUER { $group } SUR LA CARTE
map-video = VIDÉO
map-photo = PHOTO
map-layer-labels = NOMS DES LIEUX
map-layer-clusters = GROUPER SI PROCHES
map-axis-x = X
map-axis-y = Y
map-axis-z = Z

## Overlay

overlay-dock-mods = MODS
overlay-dock-map = CARTE
overlay-dock-session = SESSION
overlay-dock-minimap = MINICARTE
overlay-minimap-tag = [ MINICARTE ]
overlay-no-live-map = SANS CARTE LIVE
overlay-no-live-map-hint = Elle suit le joueur quand un trainer capable de le situer tourne dans un jeu avec une carte.
overlay-mode = MODE
overlay-mode-hint = en jeu, dessiné dans le jeu : plus tard
overlay-mode-over = SUR LE JEU
overlay-mode-in-game = EN JEU
overlay-hotkey = RACCOURCI
overlay-hotkey-hint = modificateurs et une touche, libres dans d'autres applis et mods
overlay-minimap-hotkey = RACCOURCI MINICARTE
overlay-minimap-hotkey-hint = affiche ou masque la minicarte ; actif seulement quand le jeu est au premier plan

## Settings

settings = PARAMÈTRES
settings-appearance = APPARENCE
settings-overlay = OVERLAY
settings-hotkeys = RACCOURCIS
settings-app = APP
settings-resources = RESSOURCES
settings-about = À PROPOS
accent = ACCENT
accent-hint = marque seulement le LIVE
accent-lime = LIME
accent-amber = AMBRE
accent-cyan = CYAN
accent-white = BLANC
sidebar = BARRE LATÉRALE
sidebar-hint = en tête de chaque ligne de jeu
sidebar-number = NUMÉRO
sidebar-thumbnail = VIGNETTE
panels = PANNEAUX
panels-hint = carte et session au-dessus des mods, ou à côté
panels-top = HAUT
panels-side = CÔTÉ
reduce-motion = ANIMATIONS RÉDUITES
reduce-motion-hint = ni LIVE clignotant ni transitions
language = LANGUE
language-hint = la langue des textes d'Impulse
language-system = SYSTÈME
close-to-tray = RÉDUIRE EN FERMANT
close-to-tray-hint = Fermer masque Impulse ; quitter par l'icône de notification
hide-from-capture = MASQUER À LA CAPTURE
hide-from-capture-hint = captures d'écran, enregistrements et streams n'affichent pas Impulse
folder-path = chemin du dossier
folder-select = Choisir le dossier

## Remote

remote = PANNEAU À DISTANCE
remote-open = Panneau à distance : piloter Impulse depuis un téléphone
remote-switch = ACCÈS TÉLÉPHONE
remote-hint = piloter Impulse depuis un téléphone sur ce réseau
remote-address = ADRESSE
remote-address-hint = à scanner avec l'appareil photo : le code associe une fois et change toutes les cinq minutes
remote-failed = aucune adresse réseau où écouter
remote-devices = APPAREILS
remote-devices-hint = téléphones et navigateurs associés ; un appareil révoqué est déconnecté aussitôt
remote-no-devices = aucun appareil pour l'instant
remote-device = appareil { $number }
remote-unknown-device = NAVIGATEUR
remote-revoke = RÉVOQUER
remote-trust = HTTP simple : tout ce réseau peut le voir. Réseaux de confiance seulement.
remote-tips = SI LE TÉLÉPHONE NE SE CONNECTE PAS
remote-tip-network = Le téléphone doit être sur le réseau de ce PC : pas en données mobiles, sur un Wi-Fi invité ou un VPN.
remote-tip-firewall = Le pare-feu Windows peut bloquer Impulse sans rien demander, surtout sur un réseau public : rendez ce réseau privé, ou autorisez Impulse dans Sécurité Windows > Pare-feu et protection du réseau > Autoriser une application via le pare-feu.
remote-tip-isolation = Certains routeurs isolent les appareils Wi-Fi entre eux (isolation AP ou client) : désactivez-la, ou essayez un autre réseau.
remote-note-changed = La liste a changé : réessayez.
remote-note-not-live = Lancez d'abord le jeu.
remote-note-busy = Une autre partie tourne, ou le trainer n'est pas encore là.
remote-note-nothing-on = Activez d'abord des mods.
remote-note-name = Nommez d'abord la collection.

## Resources

media-pictures = IMAGES
media-pictures-hint = visuels et photos de repères
media-tiles = FONDS DE CARTE
media-tiles-hint = le fond des cartes ouvertes
media-videos = VIDÉOS
media-videos-hint = clips de repères
resources-reading = CALCUL EN COURS...
resources-in-use = UTILISÉ
resources-in-use-hint = tout ce qu'Impulse garde sur ce PC
resources-clear = VIDER
resources-on-restart = AU REDÉMARRAGE
resources-clears-on-restart = VIDÉ AU REDÉMARRAGE
resources-clear-caches = VIDER LES CACHES
resources-clear-caches-hint = sauf vos données ci-dessous
resources-clear-all = TOUT VIDER
resources-limit = LIMITE MÉDIAS
resources-limit-hint = au-delà, le moins récemment utilisé est retiré
resources-limit-500m = 500 Mo
resources-limit-1g = 1 Go
resources-limit-2g = 2 Go
resources-limit-5g = 5 Go
resources-limit-none = AUCUNE
resources-kept = GARDÉ
resources-progress = PROGRESSION CARTES
resources-progress-hint = repères trouvés et vos repères
resources-hotkeys = RACCOURCIS
resources-hotkeys-hint = les touches assignées
resources-logs = LOGS
resources-logs-hint = deux fichiers au plus, lus par LOG
resources-bytes = { $size } o
resources-kilobytes = { $size } Ko
resources-megabytes = { $size } Mo
resources-gigabytes = { $size } Go

## Log

log-title = LOG
log-error = ERROR
log-warn = WARN
log-info = INFO
log-debug = DEBUG
log-filter = filtrer le log
log-follow = SUIVRE
log-copy-all = TOUT COPIER
log-copy = COPIER { $count }
log-copied = COPIÉ
log-clear = VIDER
log-folder = DOSSIER
log-detail-empty = CLIC SUR UNE LIGNE POUR LIRE ET SÉLECTIONNER TOUTE L'ENTRÉE
