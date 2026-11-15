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

Jest: model STL w półfabrykacie, krzywe z modelu, zgrubna warstwami Z,
wykańczająca równoległa. Szczegóły w [ROADMAP.md](ROADMAP.md).

- [ ] Warstwice tylko na stromych miejscach + równoległa tylko na łagodnych (jedna operacja).
- [ ] Resztki po obróbce zgrubnej (ponowna zgrubna mniejszym frezem).
- [ ] Obróbka 3D na stronie B (dwustronna).

## 3. Pomysły na dalej

**Frezowanie 2D**

- [ ] Kieszeń o stałym kącie opasania (pełne HSM / trochoidalna) — teraz jest
      uproszczona odmiana: mały krok i zaokrąglone pętle.

**Paczki i program**

- [ ] Budowanie AppImage w GitHub Actions.
- [ ] Paczki dla Linuksa z CI (deb).
- [ ] Sprawdzanie aktualizacji.
