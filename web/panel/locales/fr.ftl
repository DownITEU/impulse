## Frame

panel-brand = IMPULSE · À DISTANCE
panel-no-game = Aucun jeu ouvert
dock-no-run = Pas de partie
drawer-close = Fermer
search-clear = Effacer la recherche
link-open = EN LIGNE
link-connecting = CONNEXION
link-lost = HORS LIGNE

## Connection

connection-title = Connexion
connection-close = Fermer le panneau de connexion
connection-pairing = Pour associer un autre appareil, ouvrez Paramètres > À DISTANCE dans Impulse et scannez le code QR avec lui.
connection-follows = La langue et la couleur d'accent suivent Impulse.

## Placeholders

placeholder-refused = Association refusée
placeholder-refused-body = Le code a déjà servi, a expiré ou a été trop essayé. Scannez à nouveau le code QR dans Impulse.
placeholder-outdated = Rechargement nécessaire
placeholder-outdated-body = Impulse a été mis à jour : cette page est plus ancienne que lui.
placeholder-reload = Recharger
placeholder-offline = Non connecté
placeholder-offline-body = Ouvrez Impulse sur votre PC, activez le panneau à distance dans Paramètres > À DISTANCE et scannez son code QR. Cet appareil doit être sur le même réseau.
placeholder-connecting = Connexion à Impulse
placeholder-pick = Choisissez un jeu
placeholder-pick-body = Aucun jeu n'est ouvert. Ouvrez la bibliothèque et choisissez-en un.
placeholder-browse = Ouvrir la bibliothèque

## Library

library-title = Bibliothèque
library-close = Fermer la bibliothèque
library-count = { $count ->
    [one] { $count } jeu
   *[other] { $count } jeux
}
library-search = Rechercher un jeu
library-playing = En cours
library-favorites = Favoris
library-all = Tous les jeux
library-open = OUVERT
library-no-match = Aucun jeu ne correspond à « { $query } »
library-empty = La bibliothèque est vide.

## Game

game-start = Lancer le jeu et le trainer
game-stop = Arrêter le trainer
game-favorite = Ajouter aux favoris
game-unfavorite = Retirer des favoris
run-idle = Inactif
run-starting = Lancement
run-live = Trainer actif
run-stopping = Arrêt
run-failed = Échec du lancement

## Trainer

sheet-none = Pas encore de trainer
sheet-none-body = Il n'existe pas encore de trainer pour ce jeu. Impulse le signalera dans la bibliothèque dès qu'il sortira.
sheet-pending = Chargement des cheats
sheet-failed = La liste des cheats n'est pas téléchargée
sheet-search = Rechercher un mod
sheet-start-first = Lancez le jeu pour utiliser les mods.
sheet-no-match = Aucun mod ne correspond à « { $query } »
sheet-matches = { $count ->
    [one] { $count } résultat
   *[other] { $count } résultats
}
sheet-end = FIN · { $count ->
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
} · { $on }/{ $toggles } actifs
cheat-pin = Épingler { $name }
cheat-unpin = Désépingler { $name }

## Collections

actions-reset = RAZ
actions-collect = Collection
actions-forget = Supprimer la collection { $name }
collect-title = Nouvelle collection
collect-tip = Enregistre les mods actifs comme collection.
collect-name = Nom
collect-cancel = Annuler
collect-save = Enregistrer
collect-close = Fermer

## Controls

control-press = Activer
control-apply = Appliquer
control-value = { $name } : valeur
control-decrease = Diminuer { $name }
control-increase = Augmenter { $name }
control-no-options = Aucune option
