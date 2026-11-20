## Frame

panel-brand = IMPULSE · REMOTO
panel-no-game = Nenhum jogo aberto
dock-no-run = Sem sessão
drawer-close = Fechar
search-clear = Limpar a busca
link-open = ONLINE
link-connecting = CONECTANDO
link-lost = OFFLINE

## Connection

connection-title = Conexão
connection-close = Fechar o painel de conexão
connection-pairing = Para parear outro dispositivo, clique no ícone de celular no topo do Impulse e escaneie o QR Code com o dispositivo que quer conectar.
connection-follows = O painel usa o mesmo idioma e a mesma cor de destaque do Impulse.

## Placeholders

placeholder-refused = Falha no pareamento
placeholder-refused-body = O código já foi usado, expirou ou atingiu o limite de tentativas. Escaneie o QR Code do Impulse de novo.
placeholder-outdated = Recarregue a página
placeholder-outdated-body = O Impulse foi atualizado. Recarregue a página para continuar.
placeholder-reload = Recarregar
placeholder-offline = Sem conexão
placeholder-offline-body = No PC, abra o Impulse e clique no ícone de celular no topo. Ative o acesso pelo celular e escaneie o QR Code. O PC e este dispositivo precisam estar na mesma rede.
placeholder-connecting = Conectando ao Impulse
placeholder-pick = Escolha um jogo
placeholder-pick-body = Abra a biblioteca e escolha um jogo.
placeholder-browse = Abrir a biblioteca

## Library

library-title = Biblioteca
library-close = Fechar a biblioteca
library-count = { $count ->
    [0] { $count } jogos
    [one] { $count } jogo
   *[other] { $count } jogos
}
library-search = Buscar jogos
library-playing = Em jogo
library-favorites = Favoritos
library-all = Todos os jogos
library-open = ABERTO
library-no-match = Nenhum jogo encontrado para “{ $query }”
library-empty = A biblioteca está vazia.

## Game

game-start = Iniciar o jogo e o trainer
game-stop = Parar o trainer
game-favorite = Adicionar aos favoritos
game-unfavorite = Remover dos favoritos
run-idle = Trainer parado
run-starting = Iniciando
run-live = Trainer ativo
run-stopping = Parando
run-failed = Falha ao iniciar

## Trainer

sheet-none = Ainda sem trainer
sheet-none-body = Este jogo ainda não tem trainer. Quando sair um, o Impulse avisa na biblioteca.
sheet-pending = Carregando a lista de mods
sheet-failed = Lista de mods não baixada
sheet-search = Buscar mods
sheet-start-first = Inicie o jogo para usar os mods.
sheet-no-match = Nenhum mod encontrado para “{ $query }”
sheet-matches = { $count ->
    [0] { $count } resultados
    [one] { $count } resultado
   *[other] { $count } resultados
}
sheet-end = FIM · { $count ->
    [0] { $count } MODS
    [one] { $count } MOD
   *[other] { $count } MODS
}
section-mods = { $count ->
    [0] { $count } mods
    [one] { $count } mod
   *[other] { $count } mods
}
section-mods-on = { $count ->
    [0] { $count } mods
    [one] { $count } mod
   *[other] { $count } mods
} · { $on }/{ $toggles } ativos
cheat-pin = Fixar { $name }
cheat-unpin = Tirar { $name } dos fixados

## Collections

actions-reset = RESETAR
actions-collect = Coleção
actions-forget = Excluir a coleção { $name }
collect-title = Nova coleção
collect-tip = Salva os mods ativos em uma coleção.
collect-name = Nome
collect-cancel = Cancelar
collect-save = Salvar
collect-close = Fechar

## Controls

control-press = Ativar
control-apply = Aplicar
control-value = { $name }: valor
control-decrease = Diminuir { $name }
control-increase = Aumentar { $name }
control-no-options = Sem opções
