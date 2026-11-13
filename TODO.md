# TODO

Lista tego, co jest jeszcze w planie, w kolejności. Szczegóły techniczne
większych etapów są w [ROADMAP.md](ROADMAP.md). Zrobione punkty usuwamy — co
weszło do programu, jest w [CHANGELOG.md](CHANGELOG.md) i w historii gita.

## 1. Wydania

- [ ] Kolejna wersja z poprawkami frezowania 2D.
- [ ] Sprawdzić G-code na prawdziwej maszynie (GRBL/FluidNC, LinuxCNC), potem
      złagodzić ostrzeżenie w README.

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

- [ ] Kieszeń adaptacyjna (HSM).
- [ ] V-carving.
- [ ] Budowanie AppImage w GitHub Actions.
- [ ] Paczki dla Linuksa z CI (deb).
- [ ] Sprawdzanie aktualizacji.
- [ ] Obróbka dwustronna.
