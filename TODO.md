# TODO

Lista tego, co jest jeszcze w planie, w kolejności. Szczegóły techniczne
większych etapów są w [ROADMAP.md](ROADMAP.md). Zrobione punkty usuwamy — co
weszło do programu, jest w [CHANGELOG.md](CHANGELOG.md) i w historii gita.

## 1. Wydania

- [ ] Kolejna wersja z poprawkami frezowania 2D.
- [ ] G-code na prawdziwej maszynie: pierwsze próby (2026-11-14) na frezarce
      użytkownika ze sterownikiem FluidNC (G-code zgodny z GRBL) — przygotowane
      rysunki i wygenerowane G-code działają dobrze. Zostaje:
  - [ ] zmierzyć wymiary wyciętych detali (profil zewn./wewn., kieszeń,
        otwory, naddatek, przejście wykańczające),
  - [ ] LinuxCNC (inny wariant G-code: nagłówek %, G64, M6 z G43),
  - [ ] klasyczny GRBL (ten sam G-code co FluidNC — wystarczy jedna próba),
  - [ ] potem złagodzić ostrzeżenie w README i w opisach wydań.

## 2. STL i obróbka 3D

Jest: model STL w półfabrykacie (zakładka „Model 3D”), krzywe z modelu,
zgrubna warstwami Z, wykańczająca równoległa i warstwicowa, resztki, wycięcie
po sylwetce, frezy kulowe stożkowe, płaskorzeźba ze zdjęcia, symulacja na GPU
i „Ścieżki”. Szczegóły w
[ROADMAP.md](ROADMAP.md).

Dalej w paczkach, w kolejności od największego pożytku przy frezowaniu w
drewnie (jedna paczka = jedno wydanie):

**Paczka 2 — Wykańczanie łączone**

- [ ] Strome i płaskie w jednej operacji: warstwice tylko na stromych
      miejscach, równoległa tylko na łagodnych, kąt graniczny do ustawienia —
      jedna, równa powierzchnia bez nakładania się dwóch operacji.
- [ ] Ograniczenie stromizny dla każdej wykańczającej (obrabiaj tylko
      nachylenie od–do).
- [ ] Płaskie miejsca: frez płaski tylko po poziomych powierzchniach (np. dno
      wokół płaskorzeźby).

**Paczka 3 — Kształty okrągłe**

- [ ] Wykańczająca spiralna: jedna ciągła spirala od środka na zewnątrz, bez
      zawracania — kopuły, misy, rozety.
- [ ] Wykańczająca promieniowa: przejścia od środka jak szprychy.

**Paczka 4 — Grawer na powierzchni 3D**

- [ ] Rzutowanie krzywych na model: napis albo wzór z widoku 2D frezowany po
      powierzchni na stałej głębokości pod nią (napis na łukowatej desce, wzór
      na kopule).

**Paczka 5 — Bezpieczeństwo**

- [ ] Kolizja chwytu i nakrętki z modelem: ostrzeżenie, gdy w głębokim
      miejscu ostrza są za krótkie i o materiał ociera chwyt (ważne przy
      długich, wąskich frezach, np. kulowych stożkowych).

**Paczka 6 — Resztki i zgrubna**

- [ ] Ołówkowa (pencil): jedno przejście małym frezem wzdłuż wklęsłych
      narożników i dolin.
- [ ] Ponowna zgrubna mniejszym frezem — tylko to, czego nie dosięgnął duży
      (wąskie kieszenie między elementami).
- [ ] Zgrubna HSM 3D: stałe obciążenie frezu w każdej warstwie (szybciej i
      bezpieczniej w twardym drewnie).

**Paczka 7 — Dwie strony i najwyższa jakość**

- [ ] Obróbka 3D na stronie B (odwrócona płyta) — detale obrabiane z dwóch
      stron.
- [ ] Wykańczanie ze stałym odstępem na powierzchni (scallop): jednakowe
      grzbiety na stromym i na płaskim.

## 3. Pomysły na dalej

**Frezowanie 2D**

- [ ] **Nesting**: automatyczne układanie detali z rysunków 2D na płycie,
      jak najciaśniej, żeby zostało jak najmniej odpadu. Odstęp między
      detalami (co najmniej średnica freza), obroty (o 90° albo dowolne),
      kilka sztuk tego samego detalu, otwory i wnęki wewnątrz większych
      detali. Operacje idą razem z detalami; gdy nie mieszczą się na jednej
      płycie — kolejne płyty. Na bibliotece
      [jagua-rs](https://github.com/JeroenGar/jagua-rs) (Rust, MPL-2.0 —
      zgodna z GPL; układanie nieregularnych kształtów, też na arkuszach o
      stałym rozmiarze); przed startem sprawdzić licencje i użycie jako
      biblioteki.
- [ ] Kieszeń o stałym kącie opasania (pełne HSM / trochoidalna) — teraz jest
      uproszczona odmiana: mały krok i zaokrąglone pętle.

**Paczki i program**

- [ ] Budowanie AppImage w GitHub Actions.
- [ ] Paczki dla Linuksa z CI (deb).
- [ ] Sprawdzanie aktualizacji.
