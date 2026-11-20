## Frame

panel-brand = IMPULSE · REMOTO
panel-no-game = Ningún juego abierto
dock-no-run = Sin sesión
drawer-close = Cerrar
search-clear = Borrar la búsqueda
link-open = EN LÍNEA
link-connecting = CONECTANDO
link-lost = SIN CONEXIÓN

## Connection

connection-title = Conexión
connection-close = Cerrar el panel de conexión
connection-pairing = Para vincular otro dispositivo, abre el panel remoto con el botón del teléfono en la barra de título de Impulse y escanea el QR desde ese dispositivo.
connection-follows = Este panel usa el mismo idioma y color que Impulse.

## Placeholders

placeholder-refused = No se pudo vincular el dispositivo
placeholder-refused-body = El código ya se usó, caducó o hubo demasiados intentos. Vuelve a escanear el QR en Impulse.
placeholder-outdated = Recarga la página
placeholder-outdated-body = Impulse se actualizó y el panel aún tiene la versión anterior.
placeholder-reload = Recargar
placeholder-offline = Sin conexión
placeholder-offline-body = Abre Impulse en tu PC y pulsa el botón del teléfono en la barra de título. Activa el acceso móvil y escanea el QR. Este dispositivo y el PC deben estar en la misma red.
placeholder-connecting = Conectando con Impulse
placeholder-pick = Elige un juego
placeholder-pick-body = No hay ningún juego abierto. Abre la biblioteca y elige uno.
placeholder-browse = Abrir la biblioteca

## Library

library-title = Biblioteca
library-close = Cerrar la biblioteca
library-count = { $count ->
    [one] { $count } juego
   *[other] { $count } juegos
}
library-search = Buscar juegos
library-playing = En juego
library-favorites = Favoritos
library-all = Todos los juegos
library-open = ABIERTO
library-no-match = Ningún juego coincide con «{ $query }»
library-empty = La biblioteca está vacía.

## Game

game-start = Iniciar el juego y el trainer
game-stop = Parar el trainer
game-favorite = Añadir a favoritos
game-unfavorite = Quitar de favoritos
run-idle = Inactivo
run-starting = Iniciando
run-live = Trainer activo
run-stopping = Parando
run-failed = Error al iniciar

## Trainer

sheet-none = Aún no hay trainer
sheet-none-body = Aún no hay trainer para este juego. Impulse te lo indicará en la biblioteca en cuanto salga uno.
sheet-pending = Cargando mods
sheet-failed = Lista de mods no disponible
sheet-search = Buscar mods
sheet-start-first = Inicia el juego para usar los mods.
sheet-no-match = Ningún mod coincide con «{ $query }»
sheet-matches = { $count ->
    [one] { $count } resultado
   *[other] { $count } resultados
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
} · { $on }/{ $toggles } activos
cheat-pin = Fijar { $name }
cheat-unpin = Desfijar { $name }

## Collections

actions-reset = RESET
actions-collect = Colección
actions-forget = Eliminar la colección { $name }
collect-title = Nueva colección
collect-tip = Guarda los mods activos en una colección.
collect-name = Nombre
collect-cancel = Cancelar
collect-save = Guardar
collect-close = Cerrar

## Controls

control-press = Activar
control-apply = Aplicar
control-value = { $name }: valor
control-decrease = Reducir { $name }
control-increase = Aumentar { $name }
control-no-options = Sin opciones
