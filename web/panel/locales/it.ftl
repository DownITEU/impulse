## Frame

panel-brand = IMPULSE · DA REMOTO
panel-no-game = Nessun gioco aperto
dock-no-run = Nessuna sessione
drawer-close = Chiudi
search-clear = Cancella la ricerca
link-open = ONLINE
link-connecting = CONNESSIONE
link-lost = OFFLINE

## Connection

connection-title = Connessione
connection-close = Chiudi il pannello di connessione
connection-pairing = Per associare un altro dispositivo, clicca sull'icona del telefono nella barra del titolo di Impulse sul PC. Poi inquadra il QR con il nuovo dispositivo.
connection-follows = Il pannello usa la lingua e il colore impostati in Impulse.

## Placeholders

placeholder-refused = Associazione non riuscita
placeholder-refused-body = Il codice è scaduto, è già stato usato o ci sono stati troppi tentativi. Inquadra di nuovo il QR in Impulse.
placeholder-outdated = Ricarica la pagina
placeholder-outdated-body = Impulse è stato aggiornato: questa pagina non è più compatibile.
placeholder-reload = Ricarica
placeholder-offline = Non connesso
placeholder-offline-body = Apri Impulse sul PC e clicca sull'icona del telefono nella barra del titolo. Attiva l'accesso dal telefono e inquadra il QR. Il dispositivo e il PC devono essere sulla stessa rete.
placeholder-connecting = Connessione a Impulse
placeholder-pick = Scegli un gioco
placeholder-pick-body = Non c'è nessun gioco aperto. Scegline uno dalla libreria.
placeholder-browse = Apri la libreria

## Library

library-title = Libreria
library-close = Chiudi la libreria
library-count = { $count ->
    [one] { $count } gioco
   *[other] { $count } giochi
}
library-search = Cerca giochi
library-playing = In gioco
library-favorites = Preferiti
library-all = Tutti i giochi
library-open = APERTO
library-no-match = Nessun gioco corrisponde a “{ $query }”
library-empty = La libreria è vuota.

## Game

game-start = Avvia il gioco e il trainer
game-stop = Ferma il trainer
game-favorite = Aggiungi ai preferiti
game-unfavorite = Rimuovi dai preferiti
run-idle = Trainer fermo
run-starting = Avvio
run-live = Trainer attivo
run-stopping = Chiusura
run-failed = Avvio non riuscito

## Trainer

sheet-none = Nessun trainer per ora
sheet-none-body = Non c'è ancora un trainer per questo gioco. Appena ne esce uno, Impulse te lo segnala in libreria.
sheet-pending = Caricamento elenco mod
sheet-failed = Elenco mod non disponibile
sheet-search = Cerca mod
sheet-start-first = Avvia il gioco per usare le mod.
sheet-no-match = Nessuna mod corrisponde a “{ $query }”
sheet-matches = { $count ->
    [one] { $count } risultato
   *[other] { $count } risultati
}
sheet-end = FINE · { $count ->
    [one] { $count } MOD
   *[other] { $count } MOD
}
section-mods = { $count ->
    [one] { $count } mod
   *[other] { $count } mod
}
section-mods-on = { $count ->
    [one] { $count } mod
   *[other] { $count } mod
} · { $on }/{ $toggles } attive
cheat-pin = Fissa { $name } in alto
cheat-unpin = Non fissare più { $name } in alto

## Collections

actions-reset = RESET
actions-collect = Collezione
actions-forget = Elimina la collezione { $name }
collect-title = Nuova collezione
collect-tip = Salva le mod attive in una collezione.
collect-name = Nome
collect-cancel = Annulla
collect-save = Salva
collect-close = Chiudi

## Controls

control-press = Esegui
control-apply = Usa
control-value = { $name }: valore
control-decrease = Diminuisci { $name }
control-increase = Aumenta { $name }
control-no-options = Nessuna opzione
