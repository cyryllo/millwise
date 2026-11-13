# TODO

Lista tego, co jest w planie, w kolejności. Szczegóły techniczne większych
etapów są w [ROADMAP.md](ROADMAP.md). Odhaczone punkty przenosimy na dół do
„Zrobione” (z wersją, w której weszły).

## 1. Teraz: dopracowanie frezowania 2D / 2.5D

Najpierw poprawki istniejących operacji, potem nowe funkcje.

**Usterki zgłoszone przez użytkowników** (do opisania — co, w której operacji, jak
powinno być):

- [ ] …

**Brakujące funkcje 2D** (propozycje, do potwierdzenia):

- [ ] Przejście wykańczające w profilu i kieszeni (naddatek zbierany na końcu
      na pełnej głębokości).
- [ ] Wejście i wyjście po łuku / stycznie (lead-in, lead-out) w profilu.
- [ ] Kolejność konturów w operacji tak, żeby jałowe przejazdy były krótkie;
      ręczna zmiana kolejności operacji.
- [ ] Profil po konturze otwartym (linia, łuk) — sprawdzić zachowanie.

## 2. Pierwsze wydanie

- [ ] Wysłać na GitHub zaległe commity (tylko na prośbę).
- [ ] Wersja z poprawkami 2D (0.2.0 lub 0.1.x) + sekcja w CHANGELOG.
- [ ] Tag `vX.Y.Z` i wydanie na GitHubie z AppImage.
- [ ] Sprawdzić G-code na prawdziwej maszynie (GRBL/FluidNC, LinuxCNC), potem
      złagodzić ostrzeżenie w README.

## 3. Później: STL i obróbka 3D (wstrzymane)

Plan ustalony, start dopiero po dopracowaniu 2D. Szczegóły w
[ROADMAP.md](ROADMAP.md).

- [ ] Import STL i ustawienie modelu w półfabrykacie (0.10.0, nowy moduł).
- [ ] Kontur z góry (sylwetka) i przekrój na wysokości Z jako krzywe dla
      operacji 2.5D.
- [ ] Zgrubna 3D warstwami Z (z silnika kieszeni).
- [ ] Wykańczająca 3D: równoległa (drop-cutter) i waterline; upraszczanie
      ścieżek.
- [ ] Obróbka resztek i kontrola podcięć modelu.

## 4. Pomysły na dalej

- [ ] Kieszeń adaptacyjna (HSM).
- [ ] V-carving.
- [ ] Budowanie AppImage w GitHub Actions.
- [ ] Paczki dla Linuksa z CI (deb).
- [ ] Sprawdzanie aktualizacji.
- [ ] Obróbka dwustronna.

## Zrobione

**0.1.0** (2026-11-13)

- [x] Import DXF (linie, łuki, okręgi, polilinie z łukami, splajny, elipsy,
      bloki), łączenie w kontury, cale → mm.
- [x] Operacje: profil (zewn./wewn./po linii, mostki, rampa, naddatek), kieszeń
      (offset, zygzak, wyspy), wiercenie (proste, z wycofaniem, helisa), grawer
      (po linii / z lewej / z prawej).
- [x] Edycja rysunku: przesuń, obróć, odbij, skaluj, wyrównaj, szyk; zero
      programu.
- [x] Biblioteka narzędzi z graficznym wyborem typu freza i ustawieniami dla
      materiałów.
- [x] Maszyny: pole robocze, sterownik, posuwy, wrzeciono, zmiana narzędzia,
      chłodzenie, własny początek/koniec programu; sprawdzanie operacji.
- [x] G-code dla GRBL/FluidNC i LinuxCNC (G54–G59, osobne pliki na narzędzie).
- [x] Symulacja 3D na bryle + szybki silnik mapy wysokości, wykrywanie ruchów
      szybkich w materiale.
- [x] Projekty `.millwise`.

**Po 0.1.0 (jeszcze bez wersji)**

- [x] Znacznik zera programu w widoku 2D i 3D.
- [x] AppImage budowany lokalnie w Dockerze; `--version` / `--help`.
- [x] Plan STL / 3D zapisany w ROADMAP.md.
- [x] Historia zmian (panel na dole, Ctrl+Z / Ctrl+Y), schowek krzywych
      (Ctrl+X / C / V), usuwanie krzywych (Delete), okno skrótów klawiszowych.
