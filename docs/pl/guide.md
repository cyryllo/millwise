---
layout: default
lang: pl
other_lang: guide.html
title: Instrukcja
description: "Jak pracować w Millwise krok po kroku, z obrazkami — od otwarcia rysunku DXF albo modelu STL do G-code dla maszyny."
---
{% assign img = '/img/pl/' | relative_url %}

# Instrukcja obsługi

Ta instrukcja pokazuje, jak przejść od rysunku albo modelu 3D do G-code dla
twojej maszyny. Opisuje Millwise 0.7. Program jest we wczesnej fazie rozwoju,
więc w nowszych wersjach niektóre szczegóły mogą wyglądać trochę inaczej.

<div class="notice"><strong>Najpierw bezpieczeństwo.</strong> Millwise tworzy programy, które poruszają prawdziwą maszyną. Zawsze obejrzyj ścieżki w symulacji i przeczytaj G-code przed uruchomieniem, nowe programy puszczaj najpierw w powietrzu albo na odpadzie i nigdy nie zostawiaj pracującej maszyny bez nadzoru.</div>

<ol class="toc">
  <li><a href="#instalacja">Instalacja i pierwsze uruchomienie</a></li>
  <li><a href="#okno">Okno programu</a></li>
  <li><a href="#pliki">Pliki i projekty</a></li>
  <li><a href="#maszyna">Maszyna, półfabrykat i zero</a></li>
  <li><a href="#edycja">Zaznaczanie i edycja krzywych</a></li>
  <li><a href="#operacje2d">Operacje 2D</a></li>
  <li><a href="#obrobka3d">Obróbka 3D</a></li>
  <li><a href="#narzedzia">Narzędzia</a></li>
  <li><a href="#symulacja">Symulacja</a></li>
  <li><a href="#gcode">Eksport G-code</a></li>
  <li><a href="#klawisze">Klawiatura i mysz</a></li>
  <li><a href="#problemy">Gdy coś nie działa</a></li>
</ol>

<h2 id="instalacja">1. Instalacja i pierwsze uruchomienie</h2>

Pobierz `Millwise-…-x86_64.AppImage` z
[najnowszego wydania]({{ site.repo }}/releases/latest). To jeden plik, nic nie
trzeba instalować:

```sh
chmod +x Millwise-*-x86_64.AppImage
./Millwise-*-x86_64.AppImage
```

Można też od razu podać plik do otwarcia: `./Millwise-*.AppImage detal.dxf`.

- **Grafika.** Millwise rysuje kartą graficzną. Na kartach NVIDIA i wtedy,
  gdy domyślna grafika zawiedzie, sam przełącza się na OpenGL. Możesz wybrać
  ręcznie opcją `--gl` (OpenGL) albo `--vulkan`.
- **Język.** Polski albo angielski, według systemu. Zmienisz go w
  **Widok → Język / Language**.
- **Ustawienia** są w `~/.config/millwise/`: biblioteka narzędzi
  (`tools.json`), maszyny (`machines.json`) i ustawienia programu
  (`settings.json`). Skopiuj ten folder, żeby przenieść narzędzia i maszyny
  na inny komputer.

Przy pierwszym uruchomieniu biblioteka narzędzi dostaje zestaw podstawowy
frezów i jedną przykładową maszynę. Zacznij od ustawienia własnej maszyny
(zobacz [Maszyna, półfabrykat i zero](#maszyna)).

<h2 id="okno">2. Okno programu</h2>

[![Główne okno]({{ img }}main.png)]({{ img }}main.png)

- **Zakładki** u góry: **Projekt 2D** (rysunek i operacje), **Model 3D**
  (gdy projekt ma model STL) i **Symulacja**. <kbd>Tab</kbd> przełącza między
  projektem 2D a symulacją.
- **Pasek narzędzi** po lewej: duże przyciski grup (Zaznacz, Profil, Kieszeń,
  Wiercenie, Grawer, 3D, Zero), a obok warianty wybranej grupy.
- **Operacje** obok paska: lista operacji w kolejności obróbki, a pod nią
  ustawienia zaznaczonej operacji.
- **Prawy panel**: maszyna, półfabrykat, obróbka dwustronna, elementy
  projektu (pliki i modele), warstwy i krzywe.
- **Pasek historii** (Widok → Historia zmian): każda zmiana to jeden krok;
  kliknij krok, żeby do niego wrócić.
- **Pasek stanu** na dole: podpowiedzi dla bieżącego trybu, komunikaty i
  pozycja kursora.

<p><a href="{{ img }}left-panels.png"><img src="{{ img }}left-panels.png" alt="Pasek narzędzi i panel operacji" style="max-height:520px"></a>
<a href="{{ img }}right-panel.png"><img src="{{ img }}right-panel.png" alt="Prawy panel" style="max-height:520px"></a></p>

**Mysz:** kółko powiększa, prawy albo środkowy przycisk przesuwa widok,
<kbd>F</kbd> dopasowuje wszystko do ekranu. Kliknięcie w puste miejsce
rysunku zamyka ustawienia operacji.

<h2 id="pliki">3. Pliki i projekty</h2>

| Polecenie | Co robi |
|---|---|
| **Plik → Otwórz DXF / projekt** (<kbd>Ctrl</kbd>+<kbd>O</kbd>) | Otwiera rysunek DXF, model STL albo projekt `.millwise` jako nowy projekt. |
| **Plik → Dodaj** (<kbd>Ctrl</kbd>+<kbd>I</kbd>) | Dokłada pliki do bieżącego projektu. Można wybrać kilka naraz albo upuścić pliki na okno. Nowe elementy stają obok dotychczasowych i są zaznaczone, więc od razu można je przesunąć. |
| **Plik → Zapisz** (<kbd>Ctrl</kbd>+<kbd>S</kbd>) / **Zapisz jako** (<kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd>) | Zapisuje wszystko w jednym pliku `.millwise`: krzywe, modele, operacje, półfabrykat, kopie użytych narzędzi. |
| **Plik → Eksportuj G-code** (<kbd>Ctrl</kbd>+<kbd>E</kbd>) | Zapisuje program dla maszyny, zobacz [Eksport G-code](#gcode). |

**DXF.** Czytane są linie, łuki, okręgi, polilinie (z wybrzuszeniami),
splajny, elipsy i bloki. Jednostki są brane z pliku; jeśli plik ich nie
podaje, przyjmowane są milimetry, a prawy panel pokazuje użyte jednostki.
Warstwy rysunku zostają: można je ukrywać i używać w
[automatyzacji](#automatyzacja).

**STL.** Pliki binarne i tekstowe. Model ma własną zakładkę, zobacz
[Obróbka 3D](#obrobka3d).

<h2 id="maszyna">4. Maszyna, półfabrykat i zero</h2>

### Maszyna

Maszynę wybierasz u góry prawego panelu; **Ustawienia…** (albo
**Maszyny → Maszyny…**) otwiera okno maszyn.

[![Okno maszyn]({{ img }}machines.png)]({{ img }}machines.png)

- **Rodzaj maszyny**: portalowa, z ruchomym stołem, kolumnowa albo do płyt
  (ploter), oraz **sterownik**: GRBL, FluidNC albo LinuxCNC.
- **Pole robocze**: przesuw w X, Y i Z, narożnik bazowania, najdłuższy wysięg
  narzędzia, wysokość bezpieczna i układ współrzędnych (G54…).
- **Ruch**: największe posuwy, ruchy szybkie i przyspieszenie (do szacowania
  czasu i sprawdzania).
- **Wrzeciono**: najmniejsze i największe obroty oraz czas rozpędzania.
- **Zmiana narzędzia**: pauza (M0), M6 albo osobny plik G-code na każdy frez;
  opcjonalnie pozycja zmiany we współrzędnych maszyny (G53).
- **Wyposażenie**: chłodziwo (M7/M8) i sonda Z.
- Zakładka **G-code**: rozszerzenie pliku, miejsca po przecinku, G64, łuki
  G2/G3 albo same odcinki, numery linii N, komentarze, własny początek i
  koniec programu.

Każda operacja jest sprawdzana względem maszyny. Za głęboko, za szybko albo
poza polem roboczym — pojawia się uwaga w operacji i przed eksportem.

### Półfabrykat

- **Grubość** płyty. **Z0 to góra półfabrykatu.**
- **Naddatek XY**: półfabrykat ma wymiar rysunku plus ten margines z każdej
  strony.
- **Materiał** (drewno, MDF, aluminium, plexi…). **Do operacji** wpisuje do
  wszystkich operacji obroty i posuwy dla tego materiału z biblioteki narzędzi.

### Zero programu

Grupa **Zero** na pasku narzędzi ustawia zero XY w **lewym dolnym** rogu,
na **środku** albo w **lewym górnym** rogu półfabrykatu, albo w **punkcie**,
który klikniesz (przyciąga do wierzchołków i środków okręgów). Ten sam punkt
ustaw na maszynie przed uruchomieniem programu.

<h2 id="edycja">5. Zaznaczanie i edycja krzywych</h2>

Grupa **Zaznacz** ma narzędzia do rysunku:

| Narzędzie | Do czego |
|---|---|
| **Zaznacz** | Kliknij krzywą albo przeciągnij ramkę, żeby zaznaczyć wiele. <kbd>Shift</kbd> lub <kbd>Ctrl</kbd> + klik dodaje do zaznaczenia. Zaznaczone krzywe można przeciągać myszą. |
| **Przesuń**, **Obróć**, **Skaluj** | Dokładne wartości w panelu; zmianę widać od razu, a w historii to jeden krok. |
| **Odbij poziomo / pionowo** | Od razu, względem środka zaznaczenia. |
| **Wyrównaj** | Zaznaczone krzywe względem siebie: do lewej, prawej, góry, dołu albo do środka. |
| **Szyk** | Wiersze i kolumny z odstępem między kopiami; kopie trafiają też do operacji oryginału. |
| **Zmierz** | Kliknij dwa punkty (przyciąga do wierzchołków i środków okręgów): ΔX, ΔY, odległość i kąt. |

Menu **Edycja**: cofnij (<kbd>Ctrl</kbd>+<kbd>Z</kbd>), ponów
(<kbd>Ctrl</kbd>+<kbd>Y</kbd>), wytnij, kopiuj, wklej (w miejsce kursora),
usuń krzywe (<kbd>Delete</kbd>), zaznacz wszystko
(<kbd>Ctrl</kbd>+<kbd>A</kbd>), odznacz (<kbd>Esc</kbd>). Lista **Krzywe** w
prawym panelu zaznacza naraz wszystkie zamknięte krzywe albo wszystkie
okręgi (do wiercenia).

**Tekst** (w grupie Grawer): wpisz tekst, wybierz czcionkę i wysokość wielkich
liter, kliknij, gdzie go wstawić. Litery stają się krzywymi do graweru,
kieszeni albo V-carvingu.

<h2 id="operacje2d">6. Operacje 2D</h2>

### Tworzenie operacji

1. Wybierz na pasku grupę i wariant, np. **Profil → Zewnętrzny**.
2. Kliknij krzywą. Operacja powstaje, a kolejnymi kliknięciami dodajesz albo
   usuwasz krzywe. <kbd>Enter</kbd> albo <kbd>Esc</kbd> kończy tryb.
3. Ustaw operację po lewej: **najpierw frez**, potem głębokość, krok w dół i
   resztę. Przycisk **Parametry: (materiał)** wczytuje obroty i posuwy z
   narzędzia dla materiału półfabrykatu.
4. Obejrzyj ścieżkę na rysunku, a potem w [symulacji](#symulacja).

Operacje wykonywane są od góry listy. Kolejność zmieniasz przeciąganiem;
**Ułóż według narzędzi** stawia obok siebie operacje tym samym frezem, a
wycinanie detali na końcu. Pole wyboru wyłącza operację bez usuwania. Panel
pokazuje liczbę ruchów i szacowany czas.

### Rodzaje

| Operacja | Warianty i ustawienia |
|---|---|
| **Profil** | **Zewnętrzny**, **wewnętrzny** albo **po linii**. Współbieżny albo przeciwbieżny, wejście pionowe, po **rampie** albo **po łuku** (zejście w odpadzie, bez śladu), **naddatek**, **przejście wykańczające** (ostatnie przejście na pełnej głębokości zbiera resztę, z własnym posuwem), **mostki** (liczba, szerokość, wysokość). |
| **Narożniki** | Kliknij profil wewnętrzny albo kieszeń: frez dojeżdża do każdego rogu (dog-bone), więc kwadratowy element wchodzi do końca. |
| **Kieszeń** | **Offsetowa**, **zygzak** albo **adaptacyjna** (HSM: mały, równy krok i zaokrąglone pętle, łagodne dla frezu). Krzywe w środku stają się wyspami. Naddatek ścian i przejście wykańczające. |
| **Wiercenie** | **Pojedyncze**, **z wycofaniem** (co zadaną głębokość) albo **frezowanie otworu** (po helisie, dla otworów większych niż narzędzie). |
| **Grawer** | **Po ścieżce**, **z lewej** albo **z prawej** strony linii. |
| **V-carving** | Głębokość wynika z szerokości kształtu; ustawiona głębokość to limit. Potrzebny frez V albo grawerski. |

<h3 id="automatyzacja">Automatyzacja</h3>

**Narzędzia → Automatyzacja…** robi kilka rzeczy naraz (w historii to też
jeden krok — <kbd>Ctrl</kbd>+<kbd>Z</kbd> cofa wszystko):

[![Okno automatyzacji]({{ img }}automation.png)]({{ img }}automation.png)

- **Operacje dla wolnych krzywych**, warstwa po warstwie. „Z kształtu”
  decyduje sam: okrąg w rozmiarze wiertła jest wiercony (większy niż frez —
  frezowanie otworu), kontur zewnętrzny dostaje profil zewnętrzny, kontur w
  środku detalu profil wewnętrzny, a linia otwarta grawer. Wybory dla warstw
  są zapamiętywane.
- **Uszka w narożnikach** we wszystkich profilach wewnętrznych i kieszeniach.
- **Mostki według obwodu** w profilach zewnętrznych: około jeden na 150 mm,
  od 2 do 12.
- **Kolejność według narzędzi**, z wycinaniem detali na końcu.

### Obróbka dwustronna

Zaznacz **Obróbka dwustronna** w prawym panelu i wybierz **oś odwracania**
(wzdłuż X albo Y, przez środek półfabrykatu). **Dodaj otwory na kołki**
tworzy dwa otwory na osi, poza detalem, przez płytę i w stół ofiarny — jako
pierwszą operację strony A. W każdej operacji wybierz **stronę płyty**:
operacje strony B są odbijane i trafiają do osobnego pliku G-code. Po
odwróceniu płyty na kołki ustaw Z0 na nowej górze.

<h2 id="obrobka3d">7. Obróbka 3D</h2>

### Model

Otwórz albo dodaj plik STL. Widok przełącza się na zakładkę **Model 3D**:
model w półfabrykacie, ścieżki operacji 3D i pasek narzędzi tylko z
operacjami, które działają na modelu.

[![Zakładka Model 3D]({{ img }}model3d.png)]({{ img }}model3d.png)

<p><a href="{{ img }}model-card.png"><img src="{{ img }}model-card.png" alt="Ustawienia modelu w prawym panelu" style="max-height:560px;float:right;margin:0 0 12px 20px"></a></p>

Karta modelu w prawym panelu:

- **Jednostki pliku** (mm albo cale) i **skala**.
- **Obróć o 90°** wokół X, Y albo Z — żeby obrabiana strona była na górze.
- **Naddatek nad modelem**: góra modelu jest na Z = −ta wartość, zero w
  lewym dolnym rogu półfabrykatu. Grubość półfabrykatu to wysokość modelu
  plus ta wartość.
- **Krzywe z modelu**: **sylwetka** (model widziany z góry) i **obrys powyżej
  Z** (tam, gdzie model jest wyżej niż podane Z) stają się krzywymi w
  projekcie 2D — np. do wycięcia profilem albo do kieszeni 2,5D wokół modelu.

Jeśli model ma **podcięcia** (miejsca schowane pod innymi częściami, gdy
patrzeć z góry), pojawia się ostrzeżenie z ich wielkością: tych miejsc nie
obrobisz od góry. Obróć model albo obrabiaj z dwóch stron.

<div style="clear:both"></div>

### Płaskorzeźba ze zdjęcia

Zamiast pliku STL otwórz albo dodaj obraz **JPG, PNG lub BMP**, a Millwise
zamieni go na model 3D: jasne miejsca są wyżej, ciemne niżej. Karta modelu
pokazuje wtedy zdjęcie i jego ustawienia:

- **Szerokość (X)** — wysokość (Y) wynika z proporcji zdjęcia.
- **Głębokość** — różnica wysokości między najciemniejszym a najjaśniejszym
  miejscem.
- **Podstawa** — materiał zostawiony pod najniższym miejscem.
- **Wygładzanie** — rozmywa zdjęcie, usuwa szum i ziarno; większe wartości
  dają łagodniejsze kształty.
- **Dokładność siatki** — mniejsza daje więcej szczegółów, ale liczy się
  dłużej (najwyżej 1000 punktów na dłuższym boku).
- **Ciemne wyżej** — dla obrazów, w których wypukłe miejsca są ciemne.

W sekcjach **Obraz**, **Kształt** i **Kadrowanie** są filtry, a obrazek w
karcie pokazuje wysokości po nich (jaśniej = wyżej) od razu przy zmianie:

- **Czarny / biały punkt** i **Auto poziomy** — pełna głębokość; **gamma**
  podnosi albo obniża ciemne miejsca.
- **Odszumianie** usuwa plamki i zostawia krawędzie, **wygładzanie**
  łagodzi, **wyostrzanie** wydobywa drobne szczegóły.
- **Odbicie lustrzane** — lewo i prawo zamienione, np. do stempla.
- **Płaskie tło** — wszystko poniżej progu leży płasko na podstawie.
- **Schodki** — tylko kilka poziomów wysokości, jak warstwice.
- **Owal** — tylko owal pośrodku, np. do portretu; **ramka** — płaskie,
  wyższe obramowanie o zadanej szerokości i wysokości.
- **Kadrowanie** — obcina brzegi zdjęcia.

Zmiany wchodzą po puszczeniu myszy albo po Enter. Dalej płaskorzeźbę obrabia
się jak każdy model: zgrubna, wykańczająca frezem kulowym albo kulowym
stożkowym, wycięcie. Najlepiej sprawdzają się zdjęcia z miękkim światłem i
jednolitym tłem; portret zwykle potrzebuje trochę wygładzania.

### Operacje

| Operacja | Co robi |
|---|---|
| **Zgrubna** | Zdejmuje większość materiału warstwami Z wokół modelu, z dodatkowymi warstwami tuż nad płaskimi miejscami. Zostawia **naddatek na modelu** do wykończenia. Bez krzywych obejmuje cały półfabrykat; zamknięte krzywe zaznaczone w projekcie 2D ograniczają ją do środka. |
| **Wykańczająca — równoległa** | Frez jest opuszczany na model w gęstych punktach: dotyka powierzchni, nie wcinając się. Najlepsza na łagodne zbocza i płaskie miejsca. **Odstęp przejść** pokazuje spodziewaną wysokość grzbietów; **kąt przejść** i **drugi raz w poprzek** (pod kątem 90°) wygładzają strome boki. Najlepiej frezem kulowym albo kulowym stożkowym, po obróbce zgrubnej. |
| **Wykańczająca — warstwicowa** | Wokół modelu na stałych wysokościach — na strome ściany. Płaskie miejsca zostają dla równoległej. |
| **Wykańczająca — łączona** | Obie naraz, w jednej operacji: warstwice tam, gdzie powierzchnia jest bardziej stroma niż **kąt graniczny** (domyślnie 45°), przejścia równoległe tam, gdzie jest łagodniejsza — z małą zakładką, żeby nie został pasek między nimi. |
| **Tylko resztki** | Wykańczanie tylko tam, gdzie większe narzędzie (wybrane w operacji) nie sięgnęło: wąskie doliny i narożniki. **Resztki grubsze niż** pomija cienkie pozostałości. |
| **Wycięcie** | Profil zewnętrzny po sylwetce modelu, z mostkami — wycina detal z półfabrykatu. |

Każdą wykańczającą można ograniczyć do zakresu **nachylenia**: np. 0–2°
(**Tylko płaskie**) obrabia same poziome miejsca — przydatne z frezem płaskim
po dnie wokół płaskorzeźby — a 50–90° tylko strome ściany.

**Głębokość** operacji 3D to najniższe miejsce, do którego zjedzie frez
(poza modelem). Ścieżki 3D **liczą się w tle**: operacja pokazuje „liczy
się…”, a okno działa dalej. G-code można wyeksportować, gdy liczenie się
skończy.

<div class="notice">Obróbka 3D nie była jeszcze sprawdzana na prawdziwej maszynie. Obejrzyj cały program w symulacji i zacznij od miękkiego materiału.</div>

<h2 id="narzedzia">8. Narzędzia</h2>

**Narzędzia → Biblioteka narzędzi…** otwiera bibliotekę wspólną dla
wszystkich projektów. Frezy są pogrupowane według rodzaju, od najmniejszej
średnicy.

[![Biblioteka narzędzi]({{ img }}tools.png)]({{ img }}tools.png)

| Rodzaj | Do czego |
|---|---|
| **Płaski** | Profile, kieszenie, obróbka zgrubna. |
| **Kulowy** | Wykańczanie 3D. |
| **Kulowy stożkowy** | Boki jak w V-bicie, na końcu półkula: mocny i cienki, idealny do dokładnego wykańczania 3D. Podaje się kąt stożka i średnicę kulki. |
| **Z promieniem** | Płaski z zaokrąglonymi narożami: zgrubna i wykańczanie płaskich miejsc. |
| **V** | Fazowanie, grawer, V-carving. |
| **Grawerski** | Stożek z małym płaskim czubkiem: grawer i V-carving. |
| **Wiertło** | Wiercenie. |

Każde narzędzie ma numer (T w G-code), średnicę, chwyt, długość ostrzy, liczbę
ostrzy oraz **parametry skrawania** dla materiałów: obroty, posuw, posuw
wgłębny, krok w dół i odstęp przejść. Rysunek w oknie pokazuje prawdziwy
kształt frezu.

Przy pierwszym uruchomieniu biblioteka dostaje **zestaw podstawowy**: frezy
płaskie, kulowe, V i wiertła 3, 4 i 6 mm, frez grawerski i kulowy stożkowy.
Projekty zapisują kopie używanych narzędzi; gdy otworzysz projekt z frezami,
których nie masz w bibliotece, zostaną do niej dodane.

**Frez dla nowych operacji** nad listą operacji ustawia narzędzie, które
dostają nowe operacje.

<h2 id="symulacja">9. Symulacja</h2>

Zakładka **Symulacja** (<kbd>Tab</kbd>) pokazuje cały program na
półfabrykacie.

[![Symulacja projektu 2D]({{ img }}sim2d.png)]({{ img }}sim2d.png)

- **Odtwórz** (<kbd>Spacja</kbd>), pauza, prędkość, **od nowa**, **koniec** i
  suwak do przewijania. Kliknij operację na liście, żeby przeskoczyć do jej
  początku.
- **Pokaż**: materiał, model 3D, ścieżki, ruchy szybkie (G0) i narzędzie.
  Pod spodem pozycja narzędzia i głębokość.
- **Strona**: w projektach dwustronnych strona A albo B (B na odwróconej
  płycie).
- **Ruchy szybkie w materiale** są sprawdzane dla całego programu i
  wypisywane jako kolizje.

Są dwa sposoby pokazania:

- **Bryła (GPU)** albo **Bryła (CPU)**: materiał jest zdejmowany ruch po
  ruchu. Gdy **Widok → Symulacja materiału na karcie graficznej (GPU)** jest
  włączona (domyślnie), robi to karta graficzna i działa płynnie przy każdej
  wielkości programu. Bez niej procesor liczy dokładną bryłę — dobrze dla
  obróbki 2D, wolno dla gęstego wykańczania 3D.
- **Ścieżki**: model docelowy, wykonane ruchy w kolorze swojej operacji,
  obrabiana warstwa na żółto, a przyszłe ruchy przyciemnione. Natychmiast przy
  każdej wielkości; pokazuje tylko bieżącą operację.

[![Symulacja 3D na karcie graficznej]({{ img }}sim3d.png)]({{ img }}sim3d.png)

[![Widok Ścieżki]({{ img }}paths.png)]({{ img }}paths.png)

Na słabej (np. zintegrowanej) karcie graficznej, która sobie nie radzi,
wyłącz opcję GPU.

<h2 id="gcode">10. Eksport G-code</h2>

**Plik → Eksportuj G-code…** (<kbd>Ctrl</kbd>+<kbd>E</kbd>) zapisuje program
dla wybranej maszyny. **Podgląd G-code** w panelu operacji pokazuje kod przed
zapisaniem.

- Program powstaje z postprocesora maszyny (GRBL / FluidNC albo LinuxCNC), z
  jej początkiem i końcem, jednostkami, łukami i numerami linii.
- **Zmiana narzędzia** tak, jak ustawiono w maszynie: pauza, M6 albo osobny
  plik na każdy frez.
- **Projekty dwustronne** dają osobny plik dla każdej strony.
- Przed zapisaniem Millwise wypisuje uwagi: operacje poza polem roboczym, za
  głębokie dla narzędzia, bez ścieżki albo jeszcze liczone.

Pierwsze uruchomienie zawsze sprawdź w powietrzu albo na odpadzie.

<h2 id="klawisze">11. Klawiatura i mysz</h2>

| Klawisz | Działanie |
|---|---|
| <kbd>Ctrl</kbd>+<kbd>O</kbd> | Otwórz DXF, STL albo projekt |
| <kbd>Ctrl</kbd>+<kbd>I</kbd> | Dodaj pliki do projektu |
| <kbd>Ctrl</kbd>+<kbd>S</kbd> / <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>S</kbd> | Zapisz / zapisz jako |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Eksportuj G-code |
| <kbd>Ctrl</kbd>+<kbd>Z</kbd> | Cofnij |
| <kbd>Ctrl</kbd>+<kbd>Y</kbd>, <kbd>Ctrl</kbd>+<kbd>Shift</kbd>+<kbd>Z</kbd> | Ponów |
| <kbd>Ctrl</kbd>+<kbd>X</kbd> / <kbd>C</kbd> / <kbd>V</kbd> | Wytnij / kopiuj / wklej krzywe |
| <kbd>Delete</kbd> | Usuń zaznaczone krzywe, a bez nich zaznaczoną operację |
| <kbd>Ctrl</kbd>+<kbd>A</kbd> | Zaznacz wszystkie krzywe |
| <kbd>Esc</kbd> / <kbd>Enter</kbd> | Zakończ bieżący tryb, odznacz |
| <kbd>Tab</kbd> | Projekt 2D ↔ symulacja |
| <kbd>F</kbd> | Dopasuj widok |
| <kbd>Spacja</kbd> | Odtwórz / zatrzymaj symulację |
| Kółko | Powiększenie |
| Prawy albo środkowy przycisk | Przesuwanie widoku (w 3D lewy przycisk obraca) |
| <kbd>Shift</kbd> / <kbd>Ctrl</kbd> + klik | Dodaj do zaznaczenia |

Ta sama lista jest w **Pomoc → Skróty klawiszowe**.

<h2 id="problemy">12. Gdy coś nie działa</h2>

- **Okno się nie otwiera** albo jest czarne: uruchom z opcją `--gl`. Jeśli
  dalej nie działa, uruchom program z terminala i wyślij komunikaty w
  zgłoszeniu.
- **Symulacja działa wolno**: sprawdź, czy **Widok → Symulacja materiału na
  karcie graficznej (GPU)** jest włączona, albo użyj widoku **Ścieżki**. Na
  słabej karcie, na której z GPU jest wolniej, wyłącz tę opcję.
- **Operacja 3D długo się liczy**: mniejszy odstęp przejść i większy model to
  więcej pracy; okno działa w tym czasie. Pomaga trochę większy odstęp albo
  krzywe ograniczające obszar.
- **„Brak ścieżki”**: operacja nie ma krzywych, frez jest za duży na kształt
  albo głębokość jest zerowa.
- **Znalazłeś błąd albo masz pomysł?** Załóż [zgłoszenie]({{ site.repo }}/issues)
  — po polsku albo po angielsku, najlepiej z plikiem projektu.
