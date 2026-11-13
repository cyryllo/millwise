# <img src="assets/icon/millwise.svg" width="48" alt="" align="center"> Millwise

**English:** [README.md](README.md) · **Zmiany:** [CHANGELOG.md](CHANGELOG.md) · **Dla programistów:** [DEVELOPMENT.md](DEVELOPMENT.md)

> **Wczesny etap rozwoju — jeszcze nie do użytku.** Millwise jest na bardzo
> wczesnym etapie (wersja 0.3.0). Zmienia się codziennie, a tworzony G-code
> nie był testowany na prawdziwych maszynach. **Na razie nie zalecamy używania
> go do sterowania frezarką.** Pierwszą wersję testową dla Linuksa można
> pobrać (niżej). Uwagi i pomysły chętnie przyjmujemy w zgłoszeniach (Issues).

Millwise to darmowy program CAM dla 3-osiowych frezarek CNC. Z rysunków DXF
tworzy ścieżki narzędzia i zapisuje G-code dla **GRBL / FluidNC** oraz
**LinuxCNC**. Jest napisany w Rust i działa na Linuksie (wersji dla Windows na
razie nie planujemy).

![Główne okno Millwise](docs/screenshot.png)

## Co potrafi (0.3.0)

- **Import DXF:** linie, łuki, okręgi, polilinie z łukami, splajny, elipsy i
  bloki; luźne elementy są łączone w kontury, cale przeliczane na milimetry.
- **Operacje:** wybierasz narzędzie i operację na pasku po lewej, potem
  klikasz krzywe w widoku 2D. Kolejność obróbki zmieniasz, przeciągając
  operacje na liście.
  - **Profil** zewnętrzny / wewnętrzny, z mostkami, rampą albo wejściem po
    łuku, naddatkiem i przejściem wykańczającym.
  - **Kieszeń** offsetowa albo zygzak, z wyspami i przejściem wykańczającym.
  - **Wiercenie:** pojedyncze, z wycofaniem albo frezowanie otworu po helisie.
  - **Grawer** po linii, z lewej albo z prawej strony linii.
- **Edycja:** przesuń, obróć, odbij, skaluj, wyrównaj, szyk, wytnij / kopiuj /
  wklej; zero programu w rogu półfabrykatu, w jego środku albo we wskazanym
  punkcie. **Cofanie i ponawianie** z paskiem historii: jedna ikona na
  czynność, kliknięcie wraca do niej.
- **Kilka plików w jednym projekcie:** do bieżącego projektu można dodać
  kolejne rysunki DXF i inne projekty (z ich operacjami); lista elementów
  pokazuje, z czego składa się projekt.
- **Biblioteka narzędzi** wspólna dla wszystkich projektów, z graficznym
  wyborem rodzaju freza (płaski, kulowy, z promieniem naroża, V, grawerski,
  wiertło) i parametrami skrawania dla materiałów (drewno, MDF, aluminium,
  plexi).
- **Maszyny** z rysunkiem rodzaju maszyny: pole robocze, narożnik bazowania, sterownik, posuwy maksymalne
  i szybkie, wrzeciono, zmiana narzędzia (pauza M0, M6 albo osobne pliki),
  chłodzenie, własny początek i koniec programu. Operacje są sprawdzane
  względem maszyny.
- **Symulacja 3D** usuwania materiału na prawdziwej bryle (operacje
  boolowskie): odtwarzanie, przewijanie, postęp operacji, wykrywanie ruchów
  szybkich w materiale.
- **Projekty** (`.millwise`) zapisują rysunek, półfabrykat, operacje i kopie
  użytych narzędzi.

![Symulacja](docs/screenshot-simulation.png)

## Plany

Import STL i obróbka 3D (drop-cutter), kieszeń adaptacyjna, V-carve, paczki
dla Linuksa. Co jest gotowe — w CHANGELOG.

## Pobieranie (Linux)

Na stronie [Releases](https://github.com/cyryllo/millwise/releases) jest
**AppImage**: jeden plik, bez instalacji. Działa na Debianie 12 / 13, Ubuntu
22.04 lub nowszym i innych dystrybucjach z tego okresu.

```sh
chmod +x Millwise-0.3.0-x86_64.AppImage
./Millwise-0.3.0-x86_64.AppImage
```

## Budowanie ze źródeł

Potrzebny jest aktualny stabilny Rust (testowane na 1.99):

```sh
git clone https://github.com/cyryllo/millwise.git
cd millwise
cargo run --release -p millwise -- samples/demo.millwise
```

Na Linuksie `packaging/linux/install-local.sh` instaluje zbudowany program,
ikonę i skrót w menu dla bieżącego użytkownika. AppImage można zbudować
skryptem `packaging/linux/build-appimage-docker.sh` (potrzebny Docker, szczegóły
w [DEVELOPMENT.md](DEVELOPMENT.md)).

## Licencja

Copyright © 2026 Cyryl Sochacki.

Millwise jest wolnym oprogramowaniem na licencji **GNU GPL v3 lub
późniejszej** (`GPL-3.0-or-later`, plik [LICENSE](LICENSE)). Możesz go używać
(także komercyjnie), zmieniać i rozpowszechniać, pod warunkiem że zmienione
wersje udostępniasz na tej samej licencji razem z kodem źródłowym.

Program jest dostarczany **bez jakiejkolwiek gwarancji**. G-code zawsze
sprawdzaj przed uruchomieniem na maszynie.

Użyte biblioteki zachowują własne, liberalne licencje — lista w
[THIRD-PARTY.md](THIRD-PARTY.md).
