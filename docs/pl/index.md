---
layout: default
lang: pl
other_lang: ""
wide: true
description: "Millwise to darmowy program CAM do 3-osiowych frezarek CNC. Z rysunków DXF i modeli STL do G-code dla GRBL, FluidNC i LinuxCNC — obróbka 2D i 3D, symulacja na karcie graficznej. Dla Linuksa."
---
{% assign img = '/img/pl/' | relative_url %}

<div class="hero">
  <img src="{{ '/assets/logo.svg' | relative_url }}" alt="Logo Millwise">
  <div>
    <h1>Millwise</h1>
    <p>Darmowy CAM do 3-osiowych frezarek CNC. Od rysunku albo modelu 3D do G-code w jednym oknie.</p>
    <p class="badges"><span>Linux</span><span>Darmowy i otwarty</span><span>GRBL · FluidNC · LinuxCNC</span><span>Obróbka 2D i 3D</span><span>Polski · English</span></p>
  </div>
</div>

<div class="buttons">
  <a class="button primary" href="#pobierz">Pobierz</a>
  <a class="button" href="{{ '/pl/guide.html' | relative_url }}">Instrukcja</a>
  <a class="button" href="{{ site.repo }}">Kod źródłowy</a>
</div>

<div class="notice"><strong>Wczesna faza rozwoju.</strong> Millwise zmienia się z dnia na dzień. Pierwsze próby G-code na prawdziwej frezarce (FluidNC) przebiegły dobrze, ale dokładność wymiarów nie była jeszcze mierzona, a obróbka 3D nie była sprawdzana na maszynie. Zawsze sprawdzaj G-code przed uruchomieniem — używasz programu na własną odpowiedzialność.</div>

<div class="shot"><a href="{{ img }}main.png"><img src="{{ img }}main.png" alt="Główne okno Millwise: operacje po lewej, rysunek ze ścieżkami pośrodku, maszyna i półfabrykat po prawej"></a></div>

<p class="lead">Millwise zamienia <strong>rysunki DXF</strong> i <strong>modele STL</strong> na ścieżki narzędzia i zapisuje G-code dla <strong>GRBL / FluidNC</strong> i <strong>LinuxCNC</strong>. Wybierasz frez i operację, klikasz krzywe, oglądasz całą obróbkę w symulacji i eksportujesz program: profile, kieszenie, otwory, grawer i V-carving w 2D oraz zgrubną i wykańczającą obróbkę modeli 3D.</p>

<div class="stats">
  <div><strong>5</strong><span>operacji 2D: profil, kieszeń, wiercenie, grawer, V-carving</span></div>
  <div><strong>4</strong><span>operacje 3D: zgrubna, wykańczająca, resztki, wycięcie</span></div>
  <div><strong>7</strong><span>rodzajów frezów, 16 narzędzi w zestawie podstawowym</span></div>
  <div><strong>GPU</strong><span>symulacja materiału na karcie graficznej</span></div>
  <div><strong>3</strong><span>sterowniki: GRBL, FluidNC, LinuxCNC</span></div>
</div>

## Co potrafi Millwise

<div class="showcase">
  <div class="shot-col small"><a href="{{ img }}left-panels.png"><img loading="lazy" src="{{ img }}left-panels.png" alt="Pasek narzędzi i panel operacji z ustawieniami profilu"></a></div>
  <div class="text-col">
    <h3>Obróbka 2D z rysunków DXF</h3>
    <ul>
      <li><strong>Profil</strong> zewnętrzny, wewnętrzny albo po linii — z mostkami, wejściem po rampie albo łuku, naddatkiem i przejściem wykańczającym z własnym posuwem.</li>
      <li><strong>Kieszeń</strong> offsetowa, zygzak albo adaptacyjna (HSM: mały, równy krok i zaokrąglone pętle) — z wyspami i przejściem wykańczającym.</li>
      <li><strong>Wiercenie</strong> pojedyncze, z wycofaniem albo frezowanie otworu po helisie; <strong>grawer</strong> po ścieżce, z lewej lub z prawej; <strong>V-carving</strong> z głębokością zależną od szerokości kształtu.</li>
      <li><strong>Uszka w narożnikach (dog-bone)</strong>, żeby kwadratowe elementy wchodziły do końca, i <strong>napisy</strong> dowolną czcionką zamieniane na krzywe.</li>
      <li>Kolejność operacji zmienia się przeciąganiem; jednym kliknięciem operacje tym samym frezem trafiają obok siebie.</li>
    </ul>
  </div>
</div>

<div class="showcase reverse">
  <div class="shot-col"><a href="{{ img }}model3d.png"><img loading="lazy" src="{{ img }}model3d.png" alt="Zakładka Model 3D: model w półfabrykacie ze ścieżkami zgrubnymi i wykańczającymi"></a></div>
  <div class="text-col">
    <h3>Obróbka 3D modeli STL</h3>
    <ul>
      <li>Otwórz plik STL albo <strong>zdjęcie</strong>, z którego powstanie płaskorzeźba: model trafia do półfabrykatu (jednostki, skala, obroty o 90°, naddatek nad modelem) i ma własną zakładkę <strong>Model 3D</strong>.</li>
      <li><strong>Zgrubna</strong> warstwami Z wokół modelu, z dodatkowymi warstwami tuż nad płaskimi miejscami.</li>
      <li><strong>Wykańczająca</strong> przejściami równoległymi albo <strong>warstwicowa</strong> na strome ściany; <strong>tylko resztki</strong> — tam, gdzie większy frez nie sięgnął.</li>
      <li><strong>Wycięcie</strong> detalu po sylwetce modelu z mostkami; sylwetka i obrys na dowolnej wysokości także jako krzywe do operacji 2D.</li>
      <li>Ostrzeżenie o podcięciach; ścieżki 3D liczą się w tle, a okno cały czas działa.</li>
    </ul>
  </div>
</div>

<div class="showcase">
  <div class="shot-col"><a href="{{ img }}sim3d.png"><img loading="lazy" src="{{ img }}sim3d.png" alt="Symulacja na karcie graficznej: wykańczanie w połowie, na modelu po obróbce zgrubnej"></a></div>
  <div class="text-col">
    <h3>Zobacz obróbkę, zanim zobaczy ją maszyna</h3>
    <ul>
      <li><strong>Bryła (GPU)</strong>: karta graficzna zdejmuje materiał ruch po ruchu — płynnie przy każdej wielkości programu.</li>
      <li><strong>Bryła (CPU)</strong> przy słabej grafice: dokładna bryła liczona przez procesor.</li>
      <li><strong>Ścieżki</strong>: model docelowy, a na nim obrabiana warstwa na żółto — natychmiast przy każdej wielkości.</li>
      <li>Odtwarzanie, pauza, przewijanie, skok do operacji; <strong>ruchy szybkie w materiale</strong> są sprawdzane dla całego programu.</li>
    </ul>
  </div>
</div>

<div class="showcase reverse">
  <div class="shot-col"><a href="{{ img }}tools.png"><img loading="lazy" src="{{ img }}tools.png" alt="Biblioteka narzędzi z frezami pogrupowanymi według rodzaju, zaznaczony frez kulowy stożkowy"></a></div>
  <div class="text-col">
    <h3>Jedna biblioteka narzędzi dla wszystkich projektów</h3>
    <ul>
      <li>Frezy płaskie, kulowe, kulowe stożkowe, z promieniem, V, grawerskie i wiertła — każdy z rysunkiem i wymiarami.</li>
      <li>Parametry skrawania dla materiałów (drewno, MDF, aluminium, plexi): wybierasz materiał półfabrykatu, a posuwy idą za nim.</li>
      <li>Na start zestaw podstawowy: frezy płaskie, kulowe, V i wiertła 3, 4 i 6 mm.</li>
      <li>Projekt zapisuje kopie używanych narzędzi, więc na innym komputerze otworzy się tak samo.</li>
    </ul>
  </div>
</div>

<div class="showcase">
  <div class="shot-col"><a href="{{ img }}machines.png"><img loading="lazy" src="{{ img }}machines.png" alt="Okno maszyn: pole robocze, narożnik bazowania, sterownik i limity"></a></div>
  <div class="text-col">
    <h3>Twoje maszyny, twój G-code</h3>
    <ul>
      <li>Pole robocze, narożnik bazowania, limity posuwów i ruchów szybkich, zakres obrotów i czas rozpędzania wrzeciona, zmiana narzędzia (pauza, M6 albo osobny plik na każdy frez), chłodziwo, sonda.</li>
      <li>Własny początek i koniec programu, numery linii, liczba miejsc po przecinku, łuki G2/G3 albo odcinki.</li>
      <li>Każda operacja jest sprawdzana względem maszyny: pole robocze, głębokość, posuwy.</li>
    </ul>
  </div>
</div>

<div class="showcase reverse">
  <div class="shot-col small"><a href="{{ img }}automation.png"><img loading="lazy" src="{{ img }}automation.png" alt="Okno automatyzacji: operacje dla każdej warstwy, uszka, mostki i kolejność według narzędzi"></a></div>
  <div class="text-col">
    <h3>Mniej klikania</h3>
    <ul>
      <li><strong>Automatyzacja</strong>: operacje dla wszystkich wolnych krzywych według warstw, uszka, mostki według obwodu i kolejność według narzędzi — w jednym kroku.</li>
      <li><strong>Obróbka dwustronna</strong> z otworami na kołki ustalające i odbitą stroną B w osobnym pliku G-code.</li>
      <li>Przesuwanie, obracanie, odbijanie, skalowanie, wyrównywanie, szyk, wycinanie, kopiowanie i wklejanie; <strong>cofanie i ponawianie</strong> z paskiem historii.</li>
      <li>Kilka plików DXF, modeli i projektów w jednym projekcie; wszystko w jednym pliku <code>.millwise</code>.</li>
    </ul>
  </div>
</div>

<h2 id="pobierz">Pobierz</h2>

<p>Za darmo, bez konta i bez instalacji. W <a href="{{ site.repo }}/releases/latest">najnowszym wydaniu</a> jest AppImage dla Linuksa:</p>

<div class="platforms">
  <div class="platform">
    <h3>Linux <span class="tag">x86_64</span></h3>
    <p><code>Millwise-…-x86_64.AppImage</code></p>
    <p>Działa na Debianie 12 / 13, Ubuntu 22.04 lub nowszym i innych dystrybucjach z tego okresu. Nadaj prawo wykonywania i uruchom.</p>
  </div>
  <div class="platform">
    <h3>Windows, macOS</h3>
    <p>Na razie nie planujemy — Millwise jest celowo programem dla Linuksa.</p>
  </div>
</div>

```sh
chmod +x Millwise-*-x86_64.AppImage
./Millwise-*-x86_64.AppImage
```

<div class="buttons">
  <a class="button primary" href="{{ site.repo }}/releases/latest">Przejdź do najnowszego wydania</a>
  <a class="button" href="{{ '/pl/guide.html' | relative_url }}">Przeczytaj instrukcję</a>
</div>

## Pytania

<div class="faq">
<details><summary>Z jakimi maszynami działa Millwise?</summary><p>Z 3-osiowymi frezarkami CNC ze sterownikiem GRBL, FluidNC (G-code zgodny z GRBL) albo LinuxCNC. G-code był najpierw próbowany na frezarce ze sterownikiem FluidNC.</p></details>
<details><summary>Jakie pliki otwiera?</summary><p>Rysunki DXF (linie, łuki, okręgi, polilinie, splajny, elipsy, bloki), modele STL (binarne i tekstowe) i własne projekty <code>.millwise</code>. Do jednego projektu można dodać kilka plików.</p></details>
<details><summary>Czy naprawdę jest darmowy?</summary><p>Tak: to wolne oprogramowanie na licencji GPL-3.0 — bez konta, reklam i ograniczeń. Kod źródłowy jest na <a href="{{ site.repo }}">GitHubie</a>.</p></details>
<details><summary>Czy wysyła coś przez internet?</summary><p>Nie. Millwise działa bez sieci i niczego nigdzie nie wysyła.</p></details>
<details><summary>Okno się nie otwiera albo wszystko działa bardzo wolno.</summary><p>Na kartach NVIDIA i wtedy, gdy domyślna grafika zawiedzie, Millwise sam wybiera OpenGL. Jeśli okno dalej się nie otwiera, uruchom program z opcją <code>--gl</code>. Na słabej karcie graficznej wyłącz Widok → Symulacja materiału na karcie graficznej (GPU).</p></details>
<details><summary>W jakim jest języku?</summary><p>Po polsku i po angielsku. Język wybiera się sam według systemu; zmienisz go w Widok → Język / Language.</p></details>
</div>

## Stan projektu

**Millwise 0.7 to wczesna wersja testowa.** Robi już obróbkę 2D i 3D, ale
zmienia się z dnia na dzień i nie była jeszcze dość testowana na prawdziwych
maszynach. W planach: wykańczanie łączone (strome i
płaskie), wykańczanie spiralne, grawer na powierzchniach 3D. Zobacz
[listę zmian]({{ site.repo }}/blob/main/CHANGELOG.md) i
[plany]({{ site.repo }}/blob/main/TODO.md).

## Pomoc i uwagi

Znalazłeś problem albo masz pomysł? Załóż [zgłoszenie]({{ site.repo }}/issues)
— po polsku albo po angielsku. Na tym etapie najbardziej pomaga testowanie
na własnej maszynie i opisanie, jak poszło.
