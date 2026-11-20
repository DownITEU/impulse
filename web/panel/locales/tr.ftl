## Frame

panel-brand = IMPULSE · UZAKTAN KONTROL
panel-no-game = Açık oyun yok
dock-no-run = Oturum yok
drawer-close = Kapat
search-clear = Aramayı temizle
link-open = ÇEVRİMİÇİ
link-connecting = BAĞLANIYOR
link-lost = ÇEVRİMDIŞI

## Connection

connection-title = Bağlantı
connection-close = Bağlantı panelini kapat
connection-pairing = Başka bir cihazı eşleştirmek için Impulse'un üst çubuğundaki telefon simgesine tıkla. QR kodunu eşleştireceğin cihazla tara.
connection-follows = Panel, Impulse'taki dil ve vurgu rengini kullanır.

## Placeholders

placeholder-refused = Cihaz eşleştirilemedi
placeholder-refused-body = Kod kullanılmış, süresi dolmuş ya da çok fazla denenmiş. Impulse'taki QR kodunu yeniden tara.
placeholder-outdated = Sayfayı yenile
placeholder-outdated-body = Impulse güncellendi. Devam etmek için bu sayfayı yenile.
placeholder-reload = Yenile
placeholder-offline = Bağlantı yok
placeholder-offline-body = Bilgisayarında Impulse'u aç ve üst çubuktaki telefon simgesine tıkla. Telefon erişimini açıp QR kodunu tara. Bu cihaz bilgisayarla aynı ağda olmalı.
placeholder-connecting = Impulse'a bağlanıyor
placeholder-pick = Bir oyun seç
placeholder-pick-body = Açık oyun yok. Kütüphaneyi açıp birini seç.
placeholder-browse = Kütüphaneyi aç

## Library

library-title = Kütüphane
library-close = Kütüphaneyi kapat
library-count = { $count ->
    [one] { $count } oyun
   *[other] { $count } oyun
}
library-search = Oyun ara
library-playing = Aktif oyun
library-favorites = Favoriler
library-all = Tüm oyunlar
library-open = AÇIK
library-no-match = “{ $query }” ile eşleşen oyun yok
library-empty = Kütüphane boş.

## Game

game-start = Oyunu ve trainerı başlat
game-stop = Trainerı durdur
game-favorite = Favorilere ekle
game-unfavorite = Favorilerden kaldır
run-idle = Trainer kapalı
run-starting = Başlatılıyor
run-live = Trainer açık
run-stopping = Trainer kapanıyor
run-failed = Başlatılamadı

## Trainer

sheet-none = Henüz trainer yok
sheet-none-body = Bu oyun için henüz trainer yok. Çıktığında kütüphanende görürsün.
sheet-pending = Hile listesi yükleniyor
sheet-failed = Hile listesi indirilemedi
sheet-search = Hile ara
sheet-start-first = Hileleri kullanmak için oyunu başlat.
sheet-no-match = “{ $query }” ile eşleşen hile yok
sheet-matches = { $count ->
    [one] { $count } sonuç
   *[other] { $count } sonuç
}
sheet-end = SON · { $count ->
    [one] { $count } HİLE
   *[other] { $count } HİLE
}
section-mods = { $count ->
    [one] { $count } hile
   *[other] { $count } hile
}
section-mods-on = { $count ->
    [one] { $count } hile
   *[other] { $count } hile
} · { $on }/{ $toggles } açık
cheat-pin = Sabitle: { $name }
cheat-unpin = Sabitlemeyi kaldır: { $name }

## Collections

actions-reset = SIFIRLA
actions-collect = Koleksiyon
actions-forget = { $name } koleksiyonunu sil
collect-title = Yeni koleksiyon
collect-tip = Açık hileleri koleksiyon olarak kaydet.
collect-name = Ad
collect-cancel = İptal
collect-save = Kaydet
collect-close = Kapat

## Controls

control-press = Uygula
control-apply = Uygula
control-value = { $name }: değer
control-decrease = { $name } değerini azalt
control-increase = { $name } değerini artır
control-no-options = Seçenek yok
