## Frame

panel-brand = IMPULSE · TÉLÉCOMMANDE
panel-no-game = Aucun jeu ouvert
dock-no-run = Aucune session
drawer-close = Fermer
search-clear = Effacer la recherche
link-open = EN LIGNE
link-connecting = CONNEXION
link-lost = HORS LIGNE

## Connection

connection-title = Connexion
connection-close = Fermer le panneau de connexion
connection-pairing = Pour associer un autre appareil, cliquez sur l'icône du téléphone dans la barre de titre d'Impulse, puis scannez le code QR avec cet appareil.
connection-follows = La langue et la couleur sont les mêmes que dans Impulse.

## Placeholders

placeholder-refused = Échec de l'association
placeholder-refused-body = Le code a déjà servi, a expiré ou a été bloqué après trop de tentatives. Scannez à nouveau le code QR dans Impulse.
placeholder-outdated = Page à recharger
placeholder-outdated-body = Impulse a été mis à jour. Rechargez cette page pour continuer.
placeholder-reload = Recharger
placeholder-offline = Hors ligne
placeholder-offline-body = Ouvrez Impulse sur votre PC et cliquez sur l'icône du téléphone dans la barre de titre. Activez l'accès mobile, puis scannez le code QR. Les deux appareils doivent être sur le même réseau.
placeholder-connecting = Connexion à Impulse
placeholder-pick = Choisissez un jeu
placeholder-pick-body = Choisissez un jeu dans la bibliothèque pour l'ouvrir ici.
placeholder-browse = Ouvrir la bibliothèque

## Library

library-title = Bibliothèque
library-close = Fermer la bibliothèque
library-count = { $count ->
    [one] { $count } jeu
   *[other] { $count } jeux
}
library-search = Rechercher un jeu
library-playing = Jeu en cours
library-favorites = Favoris
library-all = Tous les jeux
library-open = OUVERT
library-no-match = Aucun jeu ne correspond à « { $query } »
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
sheet-none-body = Dès qu'un trainer sortira pour ce jeu, Impulse l'indiquera dans votre bibliothèque.
sheet-pending = Chargement des mods
sheet-failed = Liste des mods indisponible
sheet-search = Rechercher un mod
sheet-start-first = Lancez le jeu pour utiliser les mods.
sheet-no-match = Aucun mod ne correspond à « { $query } »
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

actions-reset = RESET
actions-collect = Collection
actions-forget = Supprimer la collection { $name }
collect-title = Nouvelle collection
collect-tip = Enregistre les mods actifs dans une collection.
collect-name = Nom
collect-cancel = Annuler
collect-save = Enregistrer
collect-close = Fermer

## Controls

control-press = Lancer
control-apply = Appliquer
control-value = { $name } : valeur
control-decrease = Diminuer { $name }
control-increase = Augmenter { $name }
control-no-options = Aucune option
