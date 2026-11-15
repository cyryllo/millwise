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

## 2. Później: STL i obróbka 3D (wstrzymane)

Plan ustalony, start dopiero po dopracowaniu 2D. Szczegóły w
[ROADMAP.md](ROADMAP.md).

- [ ] Import STL i ustawienie modelu w półfabrykacie (0.10.0, nowy moduł).
- [ ] Kontur z góry (sylwetka) i przekrój na wysokości Z jako krzywe dla
      operacji 2.5D.
- [ ] Zgrubna 3D warstwami Z (z silnika kieszeni).
- [ ] Wykańczająca 3D: równoległa (drop-cutter) i waterline; upraszczanie
      ścieżek.
- [ ] Obróbka resztek i kontrola podcięć modelu.

## 3. Pomysły na dalej

**Frezowanie 2D**

- [ ] Kieszeń o stałym kącie opasania (pełne HSM / trochoidalna) — teraz jest
      uproszczona odmiana: mały krok i zaokrąglone pętle.

**Paczki i program**

- [ ] Budowanie AppImage w GitHub Actions.
- [ ] Paczki dla Linuksa z CI (deb).
- [ ] Sprawdzanie aktualizacji.
